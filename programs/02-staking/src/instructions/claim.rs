use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_interface::{mint_to, Mint, MintTo, TokenAccount, TokenInterface};

use crate::constants::{CONFIG_SEED, LOCK_DURATION_SECONDS, STAKE_SEED};
use crate::error::StakingError;
use crate::state::{Config, StakePosition};

#[derive(Accounts)]
#[instruction(index: u64)]
pub struct Claim<'info> {
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = vrddk_mint
    )]
    pub config: Account<'info, Config>,

    #[account(
        mut,
        seeds = [STAKE_SEED, user.key().as_ref(), &index.to_le_bytes()],
        bump,
        has_one = user
    )]
    pub stake_position: Account<'info, StakePosition>,

    #[account(
        mut,
        mint::authority = config,
        mint::token_program = token_2022_program
    )]
    pub vrddk_mint: InterfaceAccount<'info, Mint>,

    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = vrddk_mint,
        associated_token::authority = user,
        associated_token::token_program = token_2022_program
    )]
    pub user_vrddk_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub token_2022_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn claim(ctx: Context<Claim>, _index: u64) -> Result<()> {
    let position = &mut ctx.accounts.stake_position;
    require!(!position.claimed, StakingError::AlreadyClaimed);

    let now = Clock::get()?.unix_timestamp;
    let elapsed = now
        .checked_sub(position.staked_at)
        .ok_or(StakingError::StillLocked)?;
    require!(elapsed >= LOCK_DURATION_SECONDS, StakingError::StillLocked);

    position.claimed = true;

    let signer_seeds: &[&[&[u8]]] = &[&[CONFIG_SEED, &[ctx.accounts.config.bump]]];
    mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_2022_program.to_account_info(),
            MintTo {
                mint: ctx.accounts.vrddk_mint.to_account_info(),
                to: ctx.accounts.user_vrddk_ata.to_account_info(),
                authority: ctx.accounts.config.to_account_info(),
            },
            signer_seeds,
        ),
        position.amount,
    )?;

    Ok(())
}
