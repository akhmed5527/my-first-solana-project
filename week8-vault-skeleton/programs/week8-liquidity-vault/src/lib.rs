use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token};

pub mod error;

declare_id!("ABatkoJpEYR3sGBKeLf7aYZaKgBTtgvonxJNEM5MSm5q");

#[program]
pub mod week8_liquidity_vault {
    use super::*;

    // ============ НЕДЕЛЯ 8: создание vault ============
    /// Один раз создаёт настройки vault. Share-mint и vault-token-account
    /// создаются СНАРУЖИ (скриптом) и передаются сюда — программа
    /// лишь сохраняет их адреса.
    pub fn initialize_vault(ctx: Context<InitializeVault>, bump: u8) -> Result<()> {
        let vault_state = &mut ctx.accounts.vault_state;
        vault_state.authority = ctx.accounts.authority.key();
        vault_state.token_mint = ctx.accounts.token_mint.key();
        vault_state.share_mint = ctx.accounts.share_mint.key();
        vault_state.total_tokens = 0;
        vault_state.total_shares = 0;
        vault_state.bump = bump;

        msg!("Vault initialized. Authority: {}", vault_state.authority);
        msg!("  Token mint: {}", vault_state.token_mint);
        msg!("  Share mint: {}", vault_state.share_mint);
        Ok(())
    }

    // ============ НЕДЕЛЯ 8: карточка пользователя ============
    /// Пользователь создаёт свою "карточку" — PDA от своего адреса.
    /// Адрес детерминирован: seeds = [b"user_position", user.key()].
    pub fn open_position(ctx: Context<OpenPosition>) -> Result<()> {
        let user_position = &mut ctx.accounts.user_position;
        user_position.owner = ctx.accounts.user.key();
        user_position.amount = 0;
        user_position.shares = 0;
        user_position.bump = ctx.bumps.user_position;

        msg!("User position opened for {}", user_position.owner);
        Ok(())
    }
}

// ===================== КОНТЕКСТЫ =====================

#[derive(Accounts)]
pub struct InitializeVault<'info> {
    #[account(
        init,
        payer = authority,
        // 8 (discriminator) + 32 + 32 + 32 + 8 + 8 + 1 = 121
        space = 8 + 32 + 32 + 32 + 8 + 8 + 1,
        seeds = [b"vault_state", token_mint.key().as_ref()],
        bump
    )]
    pub vault_state: Account<'info, VaultState>,
    pub token_mint: Account<'info, Mint>,
    pub share_mint: Account<'info, Mint>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct OpenPosition<'info> {
    #[account(
        init,
        payer = user,
        // 8 + 32 + 8 + 8 + 1 = 57
        space = 8 + 32 + 8 + 8 + 1,
        seeds = [b"user_position", user.key().as_ref()],
        bump
    )]
    pub user_position: Account<'info, UserPosition>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

// ===================== STATE =====================

#[account]
pub struct VaultState {
    pub authority: Pubkey,    // 32
    pub token_mint: Pubkey,   // 32
    pub share_mint: Pubkey,   // 32
    pub total_tokens: u64,    // 8
    pub total_shares: u64,    // 8
    pub bump: u8,             // 1
}

#[account]
pub struct UserPosition {
    pub owner: Pubkey, // 32
    pub amount: u64,   // 8
    pub shares: u64,   // 8
    pub bump: u8,      // 1
}
