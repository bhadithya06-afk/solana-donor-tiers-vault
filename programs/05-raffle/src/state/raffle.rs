use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Raffle {
    pub mint: Pubkey,
    pub draw_time: i64,
    pub total_deposited: u64,
    pub entry_count: u64,
    pub randomness_requested: bool,
    pub claimed: bool,
    pub bump: u8,
}
