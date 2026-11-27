use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer, MintTo};

declare_id!("ABatkoJpEYR3sGBKeLf7aYZaKgBTtgvonxJNEM5MSm5q"); // Твой ID

#[program]
pub mod week8_liquidity_vault {
    use super::*;

    pub fn initialize_vault(ctx: Context<InitializeVault>, bump: u8) -> Result<()> {
        let vault_state = &mut ctx.accounts.vault_state;
        vault_state.authority = ctx.accounts.authority.key();
        vault_state.token_mint = ctx.accounts.token_mint.key();
        vault_state.share_mint = ctx.accounts.share_mint.key();
        vault_state.bump = bump;
        Ok(())
    }

    // ФУНКЦИЯ ДЕПОЗИТА
    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        // 1. Переводим токены от пользователя на аккаунт Vault
        let cpi_accounts = Transfer {
            from: ctx.accounts.user_token_account.to_account_info(),
            to: ctx.accounts.vault_token_account.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.key(), cpi_accounts);
        token::transfer(cpi_ctx, amount)?;

        // 2. Выдаем пользователю Share-токены (1 к 1)
        let cpi_accounts_mint = MintTo {
            mint: ctx.accounts.share_mint.to_account_info(),
            to: ctx.accounts.user_share_account.to_account_info(),
            authority: ctx.accounts.vault_state.to_account_info(),
        };
        
        let token_mint_key = ctx.accounts.token_mint.key();
        let seeds = &[
            b"vault_state",
            token_mint_key.as_ref(),
            &[ctx.accounts.vault_state.bump],
        ];
        let signer = &[&seeds[..]];
        
        let cpi_ctx_mint = CpiContext::new_with_signer(ctx.accounts.token_program.key(), cpi_accounts_mint, signer);
        token::mint_to(cpi_ctx_mint, amount)?;

        msg!("Deposited {} tokens and minted {} shares", amount, amount);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializeVault<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + 32 + 32 + 32 + 1,
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
pub struct Deposit<'info> {
    #[account(
        mut,
        seeds = [b"vault_state", token_mint.key().as_ref()],
        bump = vault_state.bump
    )]
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

    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[account]
pub struct VaultState {
    pub authority: Pubkey,
    pub token_mint: Pubkey,
    pub share_mint: Pubkey,
    pub bump: u8,
}
