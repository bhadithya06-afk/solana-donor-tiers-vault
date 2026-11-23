use anchor_lang::prelude::*;
use anchor_spl::token_2022::spl_token_2022::{
    extension::{non_transferable::NonTransferable, BaseStateWithExtensions, StateWithExtensions},
    state::Mint as Token2022Mint,
};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::constants::{CONFIG_SEED, VAULT_SEED};
use crate::error::StakingError;
use crate::state::Config;

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + Config::INIT_SPACE,
        seeds = [CONFIG_SEED],
        bump
    )]
    pub config: Account<'info, Config>,

    #[account(
        init,
        payer = payer,
        seeds = [VAULT_SEED],
        bump,
        token::mint = rddk_mint,
        token::authority = config,
        token::token_program = token_program
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,

    pub rddk_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mint::token_program = token_2022_program
    )]
    pub vrddk_mint: InterfaceAccount<'info, Mint>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub token_2022_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
    // Verify vRDDK is a Token-2022 mint with the NonTransferable extension
    // and that its mint authority is the config PDA.
    let vrddk_info = ctx.accounts.vrddk_mint.to_account_info();
    let vrddk_data = vrddk_info.try_borrow_data()?;
    let mint_state = StateWithExtensions::<Token2022Mint>::unpack(&vrddk_data)
        .map_err(|_| error!(StakingError::InvalidVrddkMint))?;

    mint_state
        .get_extension::<NonTransferable>()
        .map_err(|_| error!(StakingError::NotNonTransferable))?;

    let expected_authority = ctx.accounts.config.key();
    require!(
        mint_state.base.mint_authority == Some(expected_authority).into(),
        StakingError::InvalidVrddkMint
    );
    require!(
        mint_state.base.decimals == ctx.accounts.rddk_mint.decimals,
        StakingError::InvalidVrddkMint
    );

    let config = &mut ctx.accounts.config;
    config.rddk_mint = ctx.accounts.rddk_mint.key();
    config.vrddk_mint = ctx.accounts.vrddk_mint.key();
    config.bump = ctx.bumps.config;

    Ok(())
}
