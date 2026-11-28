use anchor_lang::prelude::*;

#[error_code]
pub enum VaultError {
    #[msg("Amount must be greater than zero")]
    ZeroAmount,
    #[msg("Calculated shares must be greater than zero")]
    ZeroShares,
    #[msg("Calculated tokens must be greater than zero")]
    ZeroTokens,
    #[msg("Math overflow")]
    MathOverflow,
    #[msg("Insufficient shares")]
    InsufficientShares,
    #[msg("Unauthorized: only admin")]
    Unauthorized,
    #[msg("Wrong token mint")]
    WrongTokenMint,
    #[msg("Wrong AMM vault account")]
    WrongAmmVault,
}
