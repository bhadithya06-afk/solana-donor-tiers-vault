use anchor_lang::prelude::*;
use anchor_lang::solana_program::native_token::LAMPORTS_PER_SOL;

/// Loyalty tier of a donor. The variant order is the on-chain/return encoding —
/// don't change it.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tier {
    None,
    Bronze,
    Silver,
    Gold,
    Platinum,
}

impl Tier {
    pub fn from_total(total: u64) -> Self {
        const SILVER_MIN: u64 = LAMPORTS_PER_SOL / 10; // 0.1 SOL
        const GOLD_MIN: u64 = LAMPORTS_PER_SOL;          // 1 SOL
        const PLATINUM_MIN: u64 = 10 * LAMPORTS_PER_SOL; // 10 SOL

        if total == 0 {
            Tier::None
        } else if total < SILVER_MIN {
            Tier::Bronze
        } else if total < GOLD_MIN {
            Tier::Silver
        } else if total < PLATINUM_MIN {
            Tier::Gold
        } else {
            Tier::Platinum
        }
    }
}
