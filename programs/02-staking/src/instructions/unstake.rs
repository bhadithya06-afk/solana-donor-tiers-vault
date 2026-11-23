use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    burn, transfer_checked, Burn, Mint, TokenAccount, TokenInterface, TransferChecked,
};

use crate::constants::{CONFIG_SEED, VAULT_SEED};
use crate::error::StakingError;
use crate::state::Config;

#[derive(Accounts)]
pub struct Unstake<'info> {
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = rddk_mint,
        has_one = vrddk_mint
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
        mut,
        mint::token_program = token_2022_program
    )]
    pub vrddk_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        token::mint = vrddk_mint,
        token::authority = user,
        token::token_program = token_2022_program
    )]
    pub user_vrddk_ata: InterfaceAccount<'info, TokenAccount>,

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
    pub token_2022_program: Interface<'info, TokenInterface>,
}

pub fn unstake(ctx: Context<Unstake>, amount: u64) -> Result<()> {
    require!(amount > 0, StakingError::ZeroAmount);

    burn(
        CpiContext::new(
            ctx.accounts.token_2022_program.to_account_info(),
            Burn {
                mint: ctx.accounts.vrddk_mint.to_account_info(),
                from: ctx.accounts.user_vrddk_ata.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        amount,
    )?;

    let signer_seeds: &[&[&[u8]]] = &[&[CONFIG_SEED, &[ctx.accounts.config.bump]]];
    transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.vault.to_account_info(),
                mint: ctx.accounts.rddk_mint.to_account_info(),
                to: ctx.accounts.user_rddk_ata.to_account_info(),
                authority: ctx.accounts.config.to_account_info(),
            },
            signer_seeds,
        ),
        amount,
        ctx.accounts.rddk_mint.decimals,
    )?;

    Ok(())
}
