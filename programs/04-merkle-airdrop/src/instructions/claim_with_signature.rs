use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    ed25519_program,
    sysvar::instructions::{
        load_current_index_checked, load_instruction_at_checked, ID as INSTRUCTIONS_ID,
    },
};
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked};

use crate::constants::{CLAIM_SEED, CONFIG_SEED, VAULT_SEED};
use crate::error::MerkleAirdropError;
use crate::state::{Claim, Config};

#[derive(Accounts)]
pub struct ClaimWithSignature<'info> {
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = mint
    )]
    pub config: Account<'info, Config>,

    #[account(
        mut,
        seeds = [VAULT_SEED],
        bump,
        token::mint = mint,
        token::authority = config,
        token::token_program = token_program
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,

    #[account(
        init,
        payer = claimant,
        space = 8 + Claim::INIT_SPACE,
        seeds = [CLAIM_SEED, claimant.key().as_ref()],
        bump
    )]
    pub claim: Account<'info, Claim>,

    #[account(
        init_if_needed,
        payer = claimant,
        associated_token::mint = mint,
        associated_token::authority = claimant,
        associated_token::token_program = token_program
    )]
    pub claimant_ata: InterfaceAccount<'info, TokenAccount>,

    pub mint: InterfaceAccount<'info, Mint>,

    #[account(mut)]
    pub claimant: Signer<'info>,

    /// CHECK: Verified by address constraint against the Solana Instructions sysvar.
    #[account(address = INSTRUCTIONS_ID)]
    pub instructions_sysvar: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

fn read_u16_le(data: &[u8], offset: usize) -> Result<u16> {
    let bytes: [u8; 2] = data
        .get(offset..offset + 2)
        .ok_or(error!(MerkleAirdropError::InvalidSignatureInstruction))?
        .try_into()
        .map_err(|_| error!(MerkleAirdropError::InvalidSignatureInstruction))?;
    Ok(u16::from_le_bytes(bytes))
}

pub fn claim_with_signature(ctx: Context<ClaimWithSignature>, amount: u64) -> Result<()> {
    require!(amount > 0, MerkleAirdropError::ZeroAmount);

    let current_idx = load_current_index_checked(&ctx.accounts.instructions_sysvar)
        .map_err(|_| error!(MerkleAirdropError::InvalidSignatureInstruction))?;
    require!(
        current_idx > 0,
        MerkleAirdropError::InvalidSignatureInstruction
    );

    let ed25519_ix = load_instruction_at_checked(
        (current_idx - 1) as usize,
        &ctx.accounts.instructions_sysvar,
    )
    .map_err(|_| error!(MerkleAirdropError::InvalidSignatureInstruction))?;

    require!(
        ed25519_ix.program_id == ed25519_program::ID,
        MerkleAirdropError::InvalidSignatureInstruction
    );

    let data = &ed25519_ix.data;
    require!(
        data.len() >= 16,
        MerkleAirdropError::InvalidSignatureInstruction
    );
    let num_signatures = data[0];
    require!(
        num_signatures == 1,
        MerkleAirdropError::InvalidSignatureInstruction
    );

    let sig_ix_idx = read_u16_le(data, 4)?;
    let pubkey_offset = read_u16_le(data, 6)? as usize;
    let pubkey_ix_idx = read_u16_le(data, 8)?;
    let msg_offset = read_u16_le(data, 10)? as usize;
    let msg_size = read_u16_le(data, 12)? as usize;
    let msg_ix_idx = read_u16_le(data, 14)?;

    require!(
        sig_ix_idx == u16::MAX && pubkey_ix_idx == u16::MAX && msg_ix_idx == u16::MAX,
        MerkleAirdropError::InvalidSignatureInstruction
    );

    let pubkey_bytes = data
        .get(pubkey_offset..pubkey_offset + 32)
        .ok_or(error!(MerkleAirdropError::InvalidSignatureInstruction))?;
    require!(
        pubkey_bytes == ctx.accounts.config.signer.as_ref(),
        MerkleAirdropError::InvalidSigner
    );

    let mut expected_msg = Vec::with_capacity(72);
    expected_msg.extend_from_slice(ctx.program_id.as_ref());
    expected_msg.extend_from_slice(ctx.accounts.claimant.key().as_ref());
    expected_msg.extend_from_slice(&amount.to_le_bytes());

    let signed_msg = data
        .get(msg_offset..msg_offset + msg_size)
        .ok_or(error!(MerkleAirdropError::InvalidSignatureInstruction))?;
    require!(
        signed_msg == expected_msg.as_slice(),
        MerkleAirdropError::InvalidSignedMessage
    );

    let claim_account = &mut ctx.accounts.claim;
    claim_account.claimant = ctx.accounts.claimant.key();
    claim_account.amount = amount;

    let signer_seeds: &[&[&[u8]]] = &[&[CONFIG_SEED, &[ctx.accounts.config.bump]]];
    transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.vault.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.claimant_ata.to_account_info(),
                authority: ctx.accounts.config.to_account_info(),
            },
            signer_seeds,
        ),
        amount,
        ctx.accounts.mint.decimals,
    )?;

    Ok(())
}
