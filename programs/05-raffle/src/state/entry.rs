use anchor_lang::prelude::*;

/// Each deposit occupies a contiguous half-open interval `[range_start, range_end)`
/// inside `[0, total_deposited)`, enabling O(1) weighted winner verification.
#[account]
#[derive(InitSpace)]
pub struct Entry {
    pub entrant: Pubkey,
    pub index: u64,
    pub amount: u64,
    pub range_start: u64,
    pub range_end: u64,
}
