use anchor_lang::prelude::*;
use anchor_spl::token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked};

use crate::constants::{CONFIG_SEED, STAKE_SEED, USER_SEED, VAULT_SEED};
use crate::error::StakingError;
use crate::state::{Config, StakePosition, UserState};

#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = rddk_mint
    )]
    pub config: Account<'info, Config>,

    #[account(
        mut,
        seeds = [VAULT_SEED],
        bump,
        token::mint = rddk_mint,
        token::authority = config,
        token::token_program = token_program
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = user,
        space = 8 + UserState::INIT_SPACE,
        seeds = [USER_SEED, user.key().as_ref()],
        bump
    )]
    pub user_state: Account<'info, UserState>,

    #[account(
        init,
        payer = user,
        space = 8 + StakePosition::INIT_SPACE,
        seeds = [
            STAKE_SEED,
            user.key().as_ref(),
            &user_state.stake_count.to_le_bytes()
        ],
        bump
    )]
    pub stake_position: Account<'info, StakePosition>,

    #[account(
        mut,
        token::mint = rddk_mint,
        token::authority = user,
        token::token_program = token_program
    )]
    pub user_rddk_ata: InterfaceAccount<'info, TokenAccount>,

    pub rddk_mint: InterfaceAccount<'info, Mint>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn stake(ctx: Context<Stake>, amount: u64) -> Result<()> {
    require!(amount > 0, StakingError::ZeroAmount);

    transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.user_rddk_ata.to_account_info(),
                mint: ctx.accounts.rddk_mint.to_account_info(),
                to: ctx.accounts.vault.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        amount,
        ctx.accounts.rddk_mint.decimals,
    )?;

    let user_state = &mut ctx.accounts.user_state;
    let current_index = user_state.stake_count;
    if current_index == 0 {
        user_state.user = ctx.accounts.user.key();
    }
    user_state.stake_count = current_index
        .checked_add(1)
        .ok_or(StakingError::Overflow)?;

    let position = &mut ctx.accounts.stake_position;
    position.user = ctx.accounts.user.key();
    position.index = current_index;
    position.amount = amount;
    position.staked_at = Clock::get()?.unix_timestamp;
    position.claimed = false;

    Ok(())
}
