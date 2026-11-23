use anchor_lang::prelude::*;

#[error_code]
pub enum RaffleError {
    #[msg("the random value did not come from the trusted oracle")]
    UntrustedOracle,
    #[msg("Deposit amount must be greater than zero")]
    ZeroAmount,
    #[msg("Draw time must be in the future")]
    InvalidDrawTime,
    #[msg("Raffle deposit period has ended")]
    RaffleClosed,
    #[msg("Raffle has not reached draw_time yet")]
    RaffleStillOpen,
    #[msg("Cannot draw a raffle with zero deposits")]
    NoEntries,
    #[msg("Randomness has already been requested")]
    RandomnessAlreadyRequested,
    #[msg("Randomness has not been fulfilled by the oracle yet")]
    RandomnessNotFulfilled,
    #[msg("This entry did not win the draw")]
    NotWinner,
    #[msg("Raffle prize has already been claimed")]
    AlreadyClaimed,
    #[msg("Arithmetic overflow")]
    Overflow,
}
