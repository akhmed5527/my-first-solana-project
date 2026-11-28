use anchor_lang::prelude::*;

#[error_code]
pub enum AmmError {
    #[msg("Amount must be greater than zero")]
    ZeroAmount,
    #[msg("LP amount must be greater than zero")]
    ZeroLp,
    #[msg("Output amount must be greater than zero")]
    ZeroOut,
    #[msg("Math overflow")]
    MathOverflow,
    #[msg("Slippage exceeded")]
    SlippageExceeded,
    #[msg("Invalid fee (must be <= 1000 bps)")]
    InvalidFee,
    #[msg("Insufficient liquidity")]
    InsufficientLiquidity,
}
