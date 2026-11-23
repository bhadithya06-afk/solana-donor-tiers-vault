use anchor_lang::prelude::*;
use anchor_spl::token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked};

use crate::constants::{ENTRY_SEED, RAFFLE_SEED, RANDOMNESS_SEED, VAULT_SEED};
use crate::error::RaffleError;
use crate::state::{Entry, Raffle, Randomness};

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(
        mut,
        seeds = [RAFFLE_SEED],
        bump = raffle.bump,
        has_one = mint
    )]
    pub raffle: Account<'info, Raffle>,

    #[account(
        seeds = [RANDOMNESS_SEED],
        bump
    )]
    pub randomness: Account<'info, Randomness>,

    #[account(
        mut,
        seeds = [VAULT_SEED],
        bump,
        token::mint = mint,
        token::authority = raffle,
        token::token_program = token_program
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,

    #[account(
        init,
        payer = entrant,
        space = 8 + Entry::INIT_SPACE,
        seeds = [ENTRY_SEED, &raffle.entry_count.to_le_bytes()],
        bump
    )]
    pub entry: Account<'info, Entry>,

    #[account(
        mut,
        token::mint = mint,
        token::authority = entrant,
        token::token_program = token_program
    )]
    pub entrant_ata: InterfaceAccount<'info, TokenAccount>,

    pub mint: InterfaceAccount<'info, Mint>,

    #[account(mut)]
    pub entrant: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
    require!(amount > 0, RaffleError::ZeroAmount);

    let raffle = &mut ctx.accounts.raffle;
    let now = Clock::get()?.unix_timestamp;
    require!(
        now < raffle.draw_time && !raffle.randomness_requested && !ctx.accounts.randomness.fulfilled,
        RaffleError::RaffleClosed
    );

    transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.entrant_ata.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.vault.to_account_info(),
                authority: ctx.accounts.entrant.to_account_info(),
            },
        ),
        amount,
        ctx.accounts.mint.decimals,
    )?;

    let range_start = raffle.total_deposited;
    let range_end = range_start
        .checked_add(amount)
        .ok_or(RaffleError::Overflow)?;
    let index = raffle.entry_count;

    raffle.total_deposited = range_end;
    raffle.entry_count = index.checked_add(1).ok_or(RaffleError::Overflow)?;

    let entry = &mut ctx.accounts.entry;
    entry.entrant = ctx.accounts.entrant.key();
    entry.index = index;
    entry.amount = amount;
    entry.range_start = range_start;
    entry.range_end = range_end;

    Ok(())
}
