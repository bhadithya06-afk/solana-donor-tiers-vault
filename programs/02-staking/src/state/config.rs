use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Config {
    pub rddk_mint: Pubkey,
    pub vrddk_mint: Pubkey,
    pub bump: u8,
}
