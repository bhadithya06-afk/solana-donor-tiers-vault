use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};

pub mod anchor {
    pub use ephemeral_vrf_sdk_vrf_macro::vrf;
}

pub mod consts {
    use anchor_lang::prelude::*;

    pub const VRF_PROGRAM_ID: Pubkey =
        pubkey!("Vrf1RNUjXmQGjmQrQLvJHs9SNkvDJEsRVFPkfSQUwGz");
    pub const DEFAULT_QUEUE: Pubkey =
        pubkey!("Cuj97ggrhhidhbu39TijNVqE74xvKJ69gDervRUXAxGh");
}

pub mod types {
    use anchor_lang::prelude::*;

    #[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
    pub struct SerializableAccountMeta {
        pub pubkey: Pubkey,
        pub is_signer: bool,
        pub is_writable: bool,
    }
}

pub mod instructions {
    use super::consts::VRF_PROGRAM_ID;
    use super::types::SerializableAccountMeta;
    use anchor_lang::prelude::*;
    use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};

    #[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
    pub struct RequestRandomnessParams {
        pub payer: Pubkey,
        pub oracle_queue: Pubkey,
        pub callback_program_id: Pubkey,
        pub callback_discriminator: Vec<u8>,
        pub caller_seed: [u8; 32],
        pub accounts_metas: Option<Vec<SerializableAccountMeta>>,
        pub callback_args: Option<Vec<u8>>,
    }

    pub fn create_request_randomness_ix(params: RequestRandomnessParams) -> Instruction {
        let (program_identity, _) =
            Pubkey::find_program_address(&[b"identity"], &params.callback_program_id);
        let mut data = vec![145, 73, 198, 134, 152, 70, 120, 48];
        params.serialize(&mut data).unwrap();

        Instruction {
            program_id: VRF_PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(params.payer, true),
                AccountMeta::new_readonly(program_identity, true),
                AccountMeta::new(params.oracle_queue, false),
                AccountMeta::new_readonly(anchor_lang::system_program::ID, false),
                AccountMeta::new_readonly(
                    anchor_lang::solana_program::sysvar::slot_hashes::ID,
                    false,
                ),
            ],
            data,
        }
    }
}
