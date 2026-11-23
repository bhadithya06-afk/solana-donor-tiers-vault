use anchor_lang::prelude::*;

#[error_code]
pub enum MerkleAirdropError {
    #[msg("Claim amount must be greater than zero")]
    ZeroAmount,
    #[msg("Invalid Merkle proof")]
    InvalidProof,
    #[msg("Missing or invalid Ed25519 signature verification instruction")]
    InvalidSignatureInstruction,
    #[msg("Ed25519 signer does not match configured authority")]
    InvalidSigner,
    #[msg("Ed25519 signed message does not match claimant, amount, and program")]
    InvalidSignedMessage,
}
