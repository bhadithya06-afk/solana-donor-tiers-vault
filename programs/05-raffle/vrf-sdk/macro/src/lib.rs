use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn vrf(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let src = item.to_string();
    let struct_pos = src.find("struct ").expect("expected struct");
    let after_struct = &src[struct_pos + "struct ".len()..];
    let ident_end = after_struct
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .unwrap_or(after_struct.len());
    let struct_ident = after_struct[..ident_end].trim();

    let open_brace = src.find('{').expect("expected {");
    let close_brace = src.rfind('}').expect("expected }");
    let header = &src[..=open_brace];
    let body = &src[open_brace + 1..close_brace];

    let injected_fields = r#"
        /// CHECK: program identity PDA
        #[account(seeds = [b"identity"], bump)]
        pub program_identity: AccountInfo<'info>,
        /// CHECK: MagicBlock VRF program
        #[account(address = ephemeral_vrf_sdk::consts::VRF_PROGRAM_ID)]
        pub vrf_program: AccountInfo<'info>,
        /// CHECK: SlotHashes sysvar
        #[account(address = anchor_lang::solana_program::sysvar::slot_hashes::ID)]
        pub slot_hashes: AccountInfo<'info>,
        pub system_program: Program<'info, System>,
    "#;

    let impl_block = format!(
        r#"
        impl<'info> {struct_ident}<'info> {{
            pub fn invoke_signed_vrf(
                &self,
                payer: &AccountInfo<'info>,
                ix: &anchor_lang::solana_program::instruction::Instruction,
            ) -> anchor_lang::Result<()> {{
                let (_, bump) = anchor_lang::prelude::Pubkey::find_program_address(
                    &[b"identity"],
                    &crate::ID,
                );
                let signer_seeds: &[&[&[u8]]] = &[&[b"identity", &[bump]]];
                anchor_lang::solana_program::program::invoke_signed(
                    ix,
                    &[
                        payer.clone(),
                        self.program_identity.clone(),
                        self.oracle_queue.clone(),
                        self.system_program.to_account_info(),
                        self.slot_hashes.clone(),
                        self.vrf_program.clone(),
                    ],
                    signer_seeds,
                )?;
                Ok(())
            }}
        }}
        "#
    );

    let out = format!("{header}\n{body}\n{injected_fields}\n}}\n{impl_block}");
    out.parse().expect("valid generated rust")
}
