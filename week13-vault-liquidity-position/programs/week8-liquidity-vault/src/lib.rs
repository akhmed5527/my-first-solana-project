use anchor_lang::prelude::*;
use anchor_spl::token::{self, Burn, Mint, MintTo, Token, TokenAccount, Transfer};

pub mod error;
use error::VaultError;

use simple_amm::cpi::accounts::Swap as AmmSwap;
use simple_amm::cpi::accounts::AddLiquidity as AmmAddLiquidity;
use simple_amm::cpi::accounts::RemoveLiquidity as AmmRemoveLiquidity;
use simple_amm::program::SimpleAmm;
use simple_amm::Pool as AmmPool;

declare_id!("ABatkoJpEYR3sGBKeLf7aYZaKgBTtgvonxJNEM5MSm5q");

#[program]
pub mod week8_liquidity_vault {
    use super::*;

    // ===================== НЕДЕЛЯ 8 =====================
    pub fn initialize_vault(
        ctx: Context<InitializeVault>,
        bump: u8,
        token_b_mint: Pubkey,
        vault_token_a: Pubkey,
        vault_token_b: Pubkey,
        vault_lp_account: Pubkey,
        amm_pool: Pubkey,
    ) -> Result<()> {
        let vs = &mut ctx.accounts.vault_state;
        vs.authority = ctx.accounts.authority.key();
        vs.token_mint = ctx.accounts.token_mint.key();
        vs.token_b_mint = token_b_mint;
        vs.share_mint = ctx.accounts.share_mint.key();
        vs.vault_token_a = vault_token_a;
        vs.vault_token_b = vault_token_b;
        vs.vault_lp_account = vault_lp_account;
        vs.amm_pool = amm_pool;
        vs.total_tokens = 0;
        vs.total_shares = 0;
        vs.initial_a = 0;
        vs.initial_b = 0;
        vs.position_open = false;
        vs.bump = bump;
        msg!("Vault init: admin={}, A={}, B={}, amm_pool={}", vs.authority, vs.token_mint, vs.token_b_mint, vs.amm_pool);
        Ok(())
    }

    pub fn open_position(ctx: Context<OpenPosition>) -> Result<()> {
        let pos = &mut ctx.accounts.user_position;
        pos.owner = ctx.accounts.user.key();
        pos.amount = 0;
        pos.shares = 0;
        pos.bump = ctx.bumps.user_position;
        msg!("User position opened for {}", pos.owner);
        Ok(())
    }

    // ===================== НЕДЕЛЯ 9 =====================
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

        token::transfer(
            CpiContext::new(
                Token::id(),
                Transfer {
                    from: ctx.accounts.user_token_account.to_account_info(),
                    to: ctx.accounts.vault_token_account.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            amount,
        )?;

        let token_mint_key = ctx.accounts.token_mint.key();
        let seeds = &[b"vault_state", token_mint_key.as_ref(), &[vault_bump]];
        let signer = &[&seeds[..]];

        token::mint_to(
            CpiContext::new_with_signer(
                Token::id(),
                MintTo {
                    mint: ctx.accounts.share_mint.to_account_info(),
                    to: ctx.accounts.user_share_account.to_account_info(),
                    authority: ctx.accounts.vault_state.to_account_info(),
                },
                signer,
            ),
            shares_to_mint,
        )?;

        let vs = &mut ctx.accounts.vault_state;
        let pos = &mut ctx.accounts.user_position;
        vs.total_tokens = vs.total_tokens.checked_add(amount).ok_or(VaultError::MathOverflow)?;
        vs.total_shares = vs.total_shares.checked_add(shares_to_mint).ok_or(VaultError::MathOverflow)?;
        pos.amount = pos.amount.checked_add(amount).ok_or(VaultError::MathOverflow)?;
        pos.shares = pos.shares.checked_add(shares_to_mint).ok_or(VaultError::MathOverflow)?;

        msg!("Deposit: {} in, {} shares minted", amount, shares_to_mint);
        Ok(())
    }

    // ===================== НЕДЕЛЯ 10 =====================
    pub fn withdraw(ctx: Context<Withdraw>, shares_amount: u64) -> Result<()> {
        require!(shares_amount > 0, VaultError::ZeroAmount);
        require!(!ctx.accounts.vault_state.position_open, VaultError::PositionOpen);

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

        token::burn(
            CpiContext::new(
                Token::id(),
                Burn {
                    mint: ctx.accounts.share_mint.to_account_info(),
                    from: ctx.accounts.user_share_account.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            shares_amount,
        )?;

        let token_mint_key = ctx.accounts.token_mint.key();
        let seeds = &[b"vault_state", token_mint_key.as_ref(), &[vault_bump]];
        let signer = &[&seeds[..]];

        token::transfer(
            CpiContext::new_with_signer(
                Token::id(),
                Transfer {
                    from: ctx.accounts.vault_token_account.to_account_info(),
                    to: ctx.accounts.user_token_account.to_account_info(),
                    authority: ctx.accounts.vault_state.to_account_info(),
                },
                signer,
            ),
            tokens_to_return,
        )?;

        let vs = &mut ctx.accounts.vault_state;
        let pos = &mut ctx.accounts.user_position;
        vs.total_tokens = vs.total_tokens.checked_sub(tokens_to_return).ok_or(VaultError::MathOverflow)?;
        vs.total_shares = vs.total_shares.checked_sub(shares_amount).ok_or(VaultError::MathOverflow)?;
        pos.amount = pos.amount.saturating_sub(tokens_to_return);
        pos.shares = pos.shares.saturating_sub(shares_amount);

        msg!("Withdraw: {} shares burned, {} tokens returned", shares_amount, tokens_to_return);
        Ok(())
    }

    // ===================== НЕДЕЛЯ 12 =====================
    pub fn swap(
        ctx: Context<SwapViaAmm>,
        amount_in: u64,
        min_amount_out: u64,
        a_to_b: bool,
    ) -> Result<()> {
        require!(amount_in > 0, VaultError::ZeroAmount);

        let token_mint_key = ctx.accounts.token_mint.key();
        let vault_bump = ctx.accounts.vault_state.bump;
        let seeds = &[b"vault_state", token_mint_key.as_ref(), &[vault_bump]];
        let signer = &[&seeds[..]];

        let cpi_accounts = AmmSwap {
            pool: ctx.accounts.amm_pool.to_account_info(),
            token_a_mint: ctx.accounts.token_mint.to_account_info(),
            token_b_mint: ctx.accounts.token_b_mint.to_account_info(),
            vault_a: ctx.accounts.amm_vault_a.to_account_info(),
            vault_b: ctx.accounts.amm_vault_b.to_account_info(),
            user_a: ctx.accounts.vault_token_a.to_account_info(),
            user_b: ctx.accounts.vault_token_b.to_account_info(),
            user: ctx.accounts.vault_state.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
        };

        let cpi_ctx = CpiContext::new_with_signer(
            simple_amm::ID,
            cpi_accounts,
            signer,
        );
        simple_amm::cpi::swap(cpi_ctx, amount_in, min_amount_out, a_to_b)?;

        msg!("Vault swap: {} in, min_out={}, a_to_b={}", amount_in, min_amount_out, a_to_b);
        Ok(())
    }

    // ===================== НЕДЕЛЯ 13 =====================
    pub fn open_amm_position(
        ctx: Context<AmmPositionAccounts>,
        amount_a: u64,
        amount_b: u64,
        min_lp: u64,
    ) -> Result<()> {
        require!(!ctx.accounts.vault_state.position_open, VaultError::PositionAlreadyOpen);
        require!(amount_a > 0 && amount_b > 0, VaultError::ZeroAmount);
        require!(ctx.accounts.vault_token_a.amount >= amount_a, VaultError::InsufficientVaultBalance);
        require!(ctx.accounts.vault_token_b.amount >= amount_b, VaultError::InsufficientVaultBalance);

        let token_mint_key = ctx.accounts.token_mint.key();
        let vault_bump = ctx.accounts.vault_state.bump;
        let seeds = &[b"vault_state", token_mint_key.as_ref(), &[vault_bump]];
        let signer = &[&seeds[..]];

        let cpi_accounts = AmmAddLiquidity {
            pool: ctx.accounts.amm_pool.to_account_info(),
            token_a_mint: ctx.accounts.token_mint.to_account_info(),
            token_b_mint: ctx.accounts.token_b_mint.to_account_info(),
            vault_a: ctx.accounts.amm_vault_a.to_account_info(),
            vault_b: ctx.accounts.amm_vault_b.to_account_info(),
            lp_mint: ctx.accounts.amm_lp_mint.to_account_info(),
            user_a: ctx.accounts.vault_token_a.to_account_info(),
            user_b: ctx.accounts.vault_token_b.to_account_info(),
            user_lp: ctx.accounts.vault_lp_account.to_account_info(),
            user: ctx.accounts.vault_state.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(simple_amm::ID, cpi_accounts, signer);
        simple_amm::cpi::add_liquidity(cpi_ctx, amount_a, amount_b, min_lp)?;

        let vs = &mut ctx.accounts.vault_state;
        vs.position_open = true;
        vs.initial_a = amount_a;
        vs.initial_b = amount_b;

        msg!("Position opened: {}A + {}B, min_lp={}", amount_a, amount_b, min_lp);
        Ok(())
    }

    pub fn collect_fees(ctx: Context<AmmPositionAccounts>, lp_amount: u64) -> Result<()> {
        require!(ctx.accounts.vault_state.position_open, VaultError::PositionNotOpen);
        require!(lp_amount > 0, VaultError::ZeroAmount);

        let current_lp = ctx.accounts.vault_lp_account.amount;
        require!(lp_amount <= current_lp, VaultError::InsufficientShares);

        let current_initial_a = ctx.accounts.vault_state.initial_a;
        let current_initial_b = ctx.accounts.vault_state.initial_b;

        let new_lp = current_lp.checked_sub(lp_amount).ok_or(VaultError::MathOverflow)?;
        let new_initial_a: u64 = if new_lp == 0 {
            0
        } else {
            ((current_initial_a as u128)
                .checked_mul(new_lp as u128).ok_or(VaultError::MathOverflow)?
                .checked_div(current_lp as u128).ok_or(VaultError::MathOverflow)?) as u64
        };
        let new_initial_b: u64 = if new_lp == 0 {
            0
        } else {
            ((current_initial_b as u128)
                .checked_mul(new_lp as u128).ok_or(VaultError::MathOverflow)?
                .checked_div(current_lp as u128).ok_or(VaultError::MathOverflow)?) as u64
        };

        let token_mint_key = ctx.accounts.token_mint.key();
        let vault_bump = ctx.accounts.vault_state.bump;
        let seeds = &[b"vault_state", token_mint_key.as_ref(), &[vault_bump]];
        let signer = &[&seeds[..]];

        let cpi_accounts = AmmRemoveLiquidity {
            pool: ctx.accounts.amm_pool.to_account_info(),
            token_a_mint: ctx.accounts.token_mint.to_account_info(),
            token_b_mint: ctx.accounts.token_b_mint.to_account_info(),
            vault_a: ctx.accounts.amm_vault_a.to_account_info(),
            vault_b: ctx.accounts.amm_vault_b.to_account_info(),
            lp_mint: ctx.accounts.amm_lp_mint.to_account_info(),
            user_a: ctx.accounts.vault_token_a.to_account_info(),
            user_b: ctx.accounts.vault_token_b.to_account_info(),
            user_lp: ctx.accounts.vault_lp_account.to_account_info(),
            user: ctx.accounts.vault_state.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(simple_amm::ID, cpi_accounts, signer);
        simple_amm::cpi::remove_liquidity(cpi_ctx, lp_amount)?;

        let vs = &mut ctx.accounts.vault_state;
        vs.initial_a = new_initial_a;
        vs.initial_b = new_initial_b;

        msg!("Collected: {} LP burned, remaining initial {}A/{}B", lp_amount, new_initial_a, new_initial_b);
        Ok(())
    }

    pub fn close_amm_position(
        ctx: Context<AmmPositionAccounts>,
        min_a: u64,
        min_b: u64,
    ) -> Result<()> {
        require!(ctx.accounts.vault_state.position_open, VaultError::PositionNotOpen);

        let lp_balance = ctx.accounts.vault_lp_account.amount;
        require!(lp_balance > 0, VaultError::ZeroAmount);

        let before_a = ctx.accounts.vault_token_a.amount;
        let before_b = ctx.accounts.vault_token_b.amount;

        let token_mint_key = ctx.accounts.token_mint.key();
        let vault_bump = ctx.accounts.vault_state.bump;
        let seeds = &[b"vault_state", token_mint_key.as_ref(), &[vault_bump]];
        let signer = &[&seeds[..]];

        let cpi_accounts = AmmRemoveLiquidity {
            pool: ctx.accounts.amm_pool.to_account_info(),
            token_a_mint: ctx.accounts.token_mint.to_account_info(),
            token_b_mint: ctx.accounts.token_b_mint.to_account_info(),
            vault_a: ctx.accounts.amm_vault_a.to_account_info(),
            vault_b: ctx.accounts.amm_vault_b.to_account_info(),
            lp_mint: ctx.accounts.amm_lp_mint.to_account_info(),
            user_a: ctx.accounts.vault_token_a.to_account_info(),
            user_b: ctx.accounts.vault_token_b.to_account_info(),
            user_lp: ctx.accounts.vault_lp_account.to_account_info(),
            user: ctx.accounts.vault_state.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(simple_amm::ID, cpi_accounts, signer);
        simple_amm::cpi::remove_liquidity(cpi_ctx, lp_balance)?;

        ctx.accounts.vault_token_a.reload()?;
        ctx.accounts.vault_token_b.reload()?;
        let delta_a = ctx.accounts.vault_token_a.amount.saturating_sub(before_a);
        let delta_b = ctx.accounts.vault_token_b.amount.saturating_sub(before_b);
        require!(delta_a >= min_a, VaultError::SlippageExceeded);
        require!(delta_b >= min_b, VaultError::SlippageExceeded);

        let vs = &mut ctx.accounts.vault_state;
        vs.position_open = false;
        vs.initial_a = 0;
        vs.initial_b = 0;

        msg!("Position closed: {} LP burned, +{}A +{}B", lp_balance, delta_a, delta_b);
        Ok(())
    }
}

// ===================== КОНТЕКСТЫ =====================

#[derive(Accounts)]
pub struct InitializeVault<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + 32*8 + 8*4 + 2,
        seeds = [b"vault_state", token_mint.key().as_ref()],
        bump,
    )]
    pub vault_state: Box<Account<'info, VaultState>>,
    pub token_mint: Box<Account<'info, Mint>>,
    pub share_mint: Box<Account<'info, Mint>>,
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
        bump,
    )]
    pub user_position: Box<Account<'info, UserPosition>>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut, seeds = [b"vault_state", token_mint.key().as_ref()], bump = vault_state.bump)]
    pub vault_state: Box<Account<'info, VaultState>>,
    pub token_mint: Box<Account<'info, Mint>>,
    #[account(mut)]
    pub user_token_account: Box<Account<'info, TokenAccount>>,
    #[account(mut, address = vault_state.vault_token_a)]
    pub vault_token_account: Box<Account<'info, TokenAccount>>,
    #[account(mut, address = vault_state.share_mint)]
    pub share_mint: Box<Account<'info, Mint>>,
    #[account(mut)]
    pub user_share_account: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        seeds = [b"user_position", user.key().as_ref()],
        bump = user_position.bump,
        constraint = user_position.owner == user.key() @ VaultError::InsufficientShares,
    )]
    pub user_position: Box<Account<'info, UserPosition>>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut, seeds = [b"vault_state", token_mint.key().as_ref()], bump = vault_state.bump)]
    pub vault_state: Box<Account<'info, VaultState>>,
    pub token_mint: Box<Account<'info, Mint>>,
    #[account(mut)]
    pub user_token_account: Box<Account<'info, TokenAccount>>,
    #[account(mut, address = vault_state.vault_token_a)]
    pub vault_token_account: Box<Account<'info, TokenAccount>>,
    #[account(mut, address = vault_state.share_mint)]
    pub share_mint: Box<Account<'info, Mint>>,
    #[account(mut)]
    pub user_share_account: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        seeds = [b"user_position", user.key().as_ref()],
        bump = user_position.bump,
        constraint = user_position.owner == user.key() @ VaultError::InsufficientShares,
    )]
    pub user_position: Box<Account<'info, UserPosition>>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct SwapViaAmm<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [b"vault_state", vault_state.token_mint.as_ref()],
        bump = vault_state.bump,
        constraint = vault_state.authority == admin.key() @ VaultError::Unauthorized,
    )]
    pub vault_state: Box<Account<'info, VaultState>>,

    #[account(address = vault_state.token_mint @ VaultError::WrongTokenMint)]
    pub token_mint: Box<Account<'info, Mint>>,
    #[account(address = vault_state.token_b_mint @ VaultError::WrongTokenMint)]
    pub token_b_mint: Box<Account<'info, Mint>>,

    #[account(mut, address = vault_state.vault_token_a @ VaultError::WrongAmmVault)]
    pub vault_token_a: Box<Account<'info, TokenAccount>>,
    #[account(mut, address = vault_state.vault_token_b @ VaultError::WrongAmmVault)]
    pub vault_token_b: Box<Account<'info, TokenAccount>>,

    #[account(mut, address = vault_state.amm_pool @ VaultError::WrongAmmVault)]
    pub amm_pool: Box<Account<'info, AmmPool>>,
    #[account(mut, address = amm_pool.vault_a @ VaultError::WrongAmmVault)]
    pub amm_vault_a: Box<Account<'info, TokenAccount>>,
    #[account(mut, address = amm_pool.vault_b @ VaultError::WrongAmmVault)]
    pub amm_vault_b: Box<Account<'info, TokenAccount>>,

    pub amm_program: Program<'info, SimpleAmm>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct AmmPositionAccounts<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [b"vault_state", vault_state.token_mint.as_ref()],
        bump = vault_state.bump,
        constraint = vault_state.authority == admin.key() @ VaultError::Unauthorized,
    )]
    pub vault_state: Box<Account<'info, VaultState>>,

    #[account(address = vault_state.token_mint @ VaultError::WrongTokenMint)]
    pub token_mint: Box<Account<'info, Mint>>,
    #[account(address = vault_state.token_b_mint @ VaultError::WrongTokenMint)]
    pub token_b_mint: Box<Account<'info, Mint>>,

    #[account(mut, address = vault_state.vault_token_a @ VaultError::WrongAmmVault)]
    pub vault_token_a: Box<Account<'info, TokenAccount>>,
    #[account(mut, address = vault_state.vault_token_b @ VaultError::WrongAmmVault)]
    pub vault_token_b: Box<Account<'info, TokenAccount>>,
    #[account(mut, address = vault_state.vault_lp_account @ VaultError::WrongAmmVault)]
    pub vault_lp_account: Box<Account<'info, TokenAccount>>,

    #[account(mut, address = vault_state.amm_pool @ VaultError::WrongAmmVault)]
    pub amm_pool: Box<Account<'info, AmmPool>>,
    #[account(mut, address = amm_pool.vault_a @ VaultError::WrongAmmVault)]
    pub amm_vault_a: Box<Account<'info, TokenAccount>>,
    #[account(mut, address = amm_pool.vault_b @ VaultError::WrongAmmVault)]
    pub amm_vault_b: Box<Account<'info, TokenAccount>>,
    #[account(mut, address = amm_pool.lp_mint @ VaultError::WrongAmmVault)]
    pub amm_lp_mint: Box<Account<'info, Mint>>,

    pub amm_program: Program<'info, SimpleAmm>,
    pub token_program: Program<'info, Token>,
}

// ===================== STATE =====================

#[account]
pub struct VaultState {
    pub authority: Pubkey,
    pub token_mint: Pubkey,
    pub token_b_mint: Pubkey,
    pub share_mint: Pubkey,
    pub vault_token_a: Pubkey,
    pub vault_token_b: Pubkey,
    pub vault_lp_account: Pubkey,
    pub amm_pool: Pubkey,
    pub total_tokens: u64,
    pub total_shares: u64,
    pub initial_a: u64,
    pub initial_b: u64,
    pub position_open: bool,
    pub bump: u8,
}

#[account]
pub struct UserPosition {
    pub owner: Pubkey,
    pub amount: u64,
    pub shares: u64,
    pub bump: u8,
}
