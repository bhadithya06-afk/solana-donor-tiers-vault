use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked};

use crate::constants::{ENTRY_SEED, RAFFLE_SEED, RANDOMNESS_SEED, VAULT_SEED};
use crate::error::RaffleError;
use crate::state::{Entry, Raffle, Randomness};

#[derive(Accounts)]
#[instruction(entry_index: u64)]
pub struct ClaimPrize<'info> {
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
        seeds = [ENTRY_SEED, &entry_index.to_le_bytes()],
        bump,
        has_one = entrant
    )]
    pub entry: Account<'info, Entry>,

    #[account(
        init_if_needed,
        payer = entrant,
        associated_token::mint = mint,
        associated_token::authority = entrant,
        associated_token::token_program = token_program
    )]
    pub winner_ata: InterfaceAccount<'info, TokenAccount>,

    pub mint: InterfaceAccount<'info, Mint>,

    #[account(mut)]
    pub entrant: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn claim(ctx: Context<ClaimPrize>, _entry_index: u64) -> Result<()> {
    let raffle = &mut ctx.accounts.raffle;
    let randomness = &ctx.accounts.randomness;

    require!(!raffle.claimed, RaffleError::AlreadyClaimed);
    require!(randomness.fulfilled, RaffleError::RandomnessNotFulfilled);
    require!(raffle.total_deposited > 0, RaffleError::NoEntries);

    // Map the 32-byte VRF value uniformly into `[0, total_deposited)`.
    let rand_u128 = u128::from_le_bytes(
        randomness.value[..16]
            .try_into()
            .map_err(|_| error!(RaffleError::NotWinner))?,
    );
    let winning_ticket = (rand_u128 % (raffle.total_deposited as u128)) as u64;

    let entry = &ctx.accounts.entry;
    require!(
        winning_ticket >= entry.range_start && winning_ticket < entry.range_end,
        RaffleError::NotWinner
    );

    raffle.claimed = true;

    let payout = ctx.accounts.vault.amount;
    let signer_seeds: &[&[&[u8]]] = &[&[RAFFLE_SEED, &[raffle.bump]]];
    transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.vault.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.winner_ata.to_account_info(),
                authority: raffle.to_account_info(),
            },
            signer_seeds,
        ),
        payout,
        ctx.accounts.mint.decimals,
    )?;

    Ok(())
}
