use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::hashv;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked};

use crate::constants::{CLAIM_SEED, CONFIG_SEED, VAULT_SEED};
use crate::error::MerkleAirdropError;
use crate::state::{Claim, Config};

#[derive(Accounts)]
pub struct ClaimMerkle<'info> {
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

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn claim(ctx: Context<ClaimMerkle>, amount: u64, proof: Vec<[u8; 32]>) -> Result<()> {
    require!(amount > 0, MerkleAirdropError::ZeroAmount);

    // Reconstruct leaf hash: SHA256(0x00 || claimant || amount_le)
    let mut computed_hash = hashv(&[
        &[0x00],
        ctx.accounts.claimant.key().as_ref(),
        &amount.to_le_bytes(),
    ])
    .to_bytes();

    // Walk proof with domain-separated sorted pair hashing: SHA256(0x01 || min || max)
    for sibling in proof.iter() {
        computed_hash = if computed_hash <= *sibling {
            hashv(&[&[0x01], &computed_hash, sibling]).to_bytes()
        } else {
            hashv(&[&[0x01], sibling, &computed_hash]).to_bytes()
        };
    }

    require!(
        computed_hash == ctx.accounts.config.merkle_root,
        MerkleAirdropError::InvalidProof
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
