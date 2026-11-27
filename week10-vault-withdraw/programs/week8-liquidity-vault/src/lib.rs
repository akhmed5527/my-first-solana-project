use anchor_lang::prelude::*;
use anchor_spl::token::{self, Burn, Mint, MintTo, Token, TokenAccount, Transfer};

pub mod error;
use error::VaultError;

declare_id!("ABatkoJpEYR3sGBKeLf7aYZaKgBTtgvonxJNEM5MSm5q");

#[program]
pub mod week8_liquidity_vault {
    use super::*;

    // ============ НЕДЕЛЯ 8 ============
    pub fn initialize_vault(ctx: Context<InitializeVault>, bump: u8) -> Result<()> {
        let vault_state = &mut ctx.accounts.vault_state;
        vault_state.authority = ctx.accounts.authority.key();
        vault_state.token_mint = ctx.accounts.token_mint.key();
        vault_state.share_mint = ctx.accounts.share_mint.key();
        vault_state.total_tokens = 0;
        vault_state.total_shares = 0;
        vault_state.bump = bump;
        msg!("Vault initialized. Authority: {}", vault_state.authority);
        Ok(())
    }

    pub fn open_position(ctx: Context<OpenPosition>) -> Result<()> {
        let user_position = &mut ctx.accounts.user_position;
        user_position.owner = ctx.accounts.user.key();
        user_position.amount = 0;
        user_position.shares = 0;
        user_position.bump = ctx.bumps.user_position;
        msg!("User position opened for {}", user_position.owner);
        Ok(())
    }

    // ============ НЕДЕЛЯ 9: ДЕПОЗИТ ============
    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        require!(amount > 0, VaultError::ZeroAmount);

        let total_shares = ctx.accounts.vault_state.total_shares;
        let total_tokens = ctx.accounts.vault_state.total_tokens;
        let vault_bump = ctx.accounts.vault_state.bump;

        let shares_to_mint: u64 = if total_shares == 0 {
            amount
        } else {
            let raw = (amount as u128)
                .checked_mul(total_shares as u128)
                .ok_or(VaultError::MathOverflow)?
                .checked_div(total_tokens as u128)
                .ok_or(VaultError::MathOverflow)?;
            require!(raw <= u64::MAX as u128, VaultError::MathOverflow);
            raw as u64
        };
        require!(shares_to_mint > 0, VaultError::ZeroShares);

        let cpi_transfer = Transfer {
            from: ctx.accounts.user_token_account.to_account_info(),
            to: ctx.accounts.vault_token_account.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };
        token::transfer(CpiContext::new(Token::id(), cpi_transfer), amount)?;

        let token_mint_key = ctx.accounts.token_mint.key();
        let seeds = &[b"vault_state", token_mint_key.as_ref(), &[vault_bump]];
        let signer = &[&seeds[..]];

        let cpi_mint = MintTo {
            mint: ctx.accounts.share_mint.to_account_info(),
            to: ctx.accounts.user_share_account.to_account_info(),
            authority: ctx.accounts.vault_state.to_account_info(),
        };
        token::mint_to(
            CpiContext::new_with_signer(Token::id(), cpi_mint, signer),
            shares_to_mint,
        )?;

        let vault_state = &mut ctx.accounts.vault_state;
        let user_position = &mut ctx.accounts.user_position;

        vault_state.total_tokens = vault_state
            .total_tokens
            .checked_add(amount)
            .ok_or(VaultError::MathOverflow)?;
        vault_state.total_shares = vault_state
            .total_shares
            .checked_add(shares_to_mint)
            .ok_or(VaultError::MathOverflow)?;
        user_position.amount = user_position
            .amount
            .checked_add(amount)
            .ok_or(VaultError::MathOverflow)?;
        user_position.shares = user_position
            .shares
            .checked_add(shares_to_mint)
            .ok_or(VaultError::MathOverflow)?;

        msg!(
            "Deposit: {} tokens in, {} shares minted (total: {} / {})",
            amount, shares_to_mint,
            vault_state.total_tokens, vault_state.total_shares
        );
        Ok(())
    }

    // ============ НЕДЕЛЯ 10: ВЫВОД ============
    pub fn withdraw(ctx: Context<Withdraw>, shares_amount: u64) -> Result<()> {
        require!(shares_amount > 0, VaultError::ZeroAmount);

        let total_shares = ctx.accounts.vault_state.total_shares;
        let total_tokens = ctx.accounts.vault_state.total_tokens;
        let vault_bump = ctx.accounts.vault_state.bump;
        let user_shares = ctx.accounts.user_position.shares;

        require!(shares_amount <= user_shares, VaultError::InsufficientShares);
        require!(total_shares > 0, VaultError::ZeroShares);

        let raw = (shares_amount as u128)
            .checked_mul(total_tokens as u128)
            .ok_or(VaultError::MathOverflow)?
            .checked_div(total_shares as u128)
            .ok_or(VaultError::MathOverflow)?;
        require!(raw <= u64::MAX as u128, VaultError::MathOverflow);
        let tokens_to_return = raw as u64;
        require!(tokens_to_return > 0, VaultError::ZeroTokens);

        // CPI: Burn (user signs)
        let cpi_burn = Burn {
            mint: ctx.accounts.share_mint.to_account_info(),
            from: ctx.accounts.user_share_account.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };
        token::burn(CpiContext::new(Token::id(), cpi_burn), shares_amount)?;

        // CPI: Transfer из vault (PDA signs)
        let token_mint_key = ctx.accounts.token_mint.key();
        let seeds = &[b"vault_state", token_mint_key.as_ref(), &[vault_bump]];
        let signer = &[&seeds[..]];

        let cpi_transfer = Transfer {
            from: ctx.accounts.vault_token_account.to_account_info(),
            to: ctx.accounts.user_token_account.to_account_info(),
            authority: ctx.accounts.vault_state.to_account_info(),
        };
        token::transfer(
            CpiContext::new_with_signer(Token::id(), cpi_transfer, signer),
            tokens_to_return,
        )?;

        let vault_state = &mut ctx.accounts.vault_state;
        let user_position = &mut ctx.accounts.user_position;

        vault_state.total_tokens = vault_state
            .total_tokens
            .checked_sub(tokens_to_return)
            .ok_or(VaultError::MathOverflow)?;
        vault_state.total_shares = vault_state
            .total_shares
            .checked_sub(shares_amount)
            .ok_or(VaultError::MathOverflow)?;
        user_position.amount = user_position.amount.saturating_sub(tokens_to_return);
        user_position.shares = user_position.shares.saturating_sub(shares_amount);

        msg!(
            "Withdraw: {} shares burned, {} tokens returned (total: {} / {})",
            shares_amount, tokens_to_return,
            vault_state.total_tokens, vault_state.total_shares
        );
        Ok(())
    }
}

// ===================== КОНТЕКСТЫ =====================

#[derive(Accounts)]
pub struct InitializeVault<'info> {
    #[account(
        init,
        payer = authority,
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
        space = 8 + 32 + 8 + 8 + 1,
        seeds = [b"user_position", user.key().as_ref()],
        bump
    )]
    pub user_position: Account<'info, UserPosition>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut, seeds = [b"vault_state", token_mint.key().as_ref()], bump = vault_state.bump)]
    pub vault_state: Account<'info, VaultState>,
    pub token_mint: Account<'info, Mint>,
    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub vault_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub share_mint: Account<'info, Mint>,
    #[account(mut)]
    pub user_share_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        seeds = [b"user_position", user.key().as_ref()],
        bump = user_position.bump,
        constraint = user_position.owner == user.key() @ VaultError::InsufficientShares,
    )]
    pub user_position: Account<'info, UserPosition>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut, seeds = [b"vault_state", token_mint.key().as_ref()], bump = vault_state.bump)]
    pub vault_state: Account<'info, VaultState>,
    pub token_mint: Account<'info, Mint>,
    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub vault_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub share_mint: Account<'info, Mint>,
    #[account(mut)]
    pub user_share_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        seeds = [b"user_position", user.key().as_ref()],
        bump = user_position.bump,
        constraint = user_position.owner == user.key() @ VaultError::InsufficientShares,
    )]
    pub user_position: Account<'info, UserPosition>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

// ===================== STATE =====================

#[account]
pub struct VaultState {
    pub authority: Pubkey,
    pub token_mint: Pubkey,
    pub share_mint: Pubkey,
    pub total_tokens: u64,
    pub total_shares: u64,
    pub bump: u8,
}

#[account]
pub struct UserPosition {
    pub owner: Pubkey,
    pub amount: u64,
    pub shares: u64,
    pub bump: u8,
}
