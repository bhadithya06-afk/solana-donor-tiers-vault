use anchor_lang::prelude::*;

#[error_code]
pub enum StakingError {
    #[msg("Amount must be greater than zero")]
    ZeroAmount,
    #[msg("Stake position is still within the 3-day lock period")]
    StillLocked,
    #[msg("Stake position has already been claimed")]
    AlreadyClaimed,
    #[msg("vRDDK mint must have the Token-2022 NonTransferable extension")]
    NotNonTransferable,
    #[msg("Invalid mint authority or decimals for vRDDK")]
    InvalidVrddkMint,
    #[msg("Arithmetic overflow")]
    Overflow,
}
