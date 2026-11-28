use anchor_lang::prelude::*;
use anchor_spl::token::{self, Burn, Mint, MintTo, Token, TokenAccount, Transfer};

pub mod error;
use error::AmmError;

declare_id!("CP2fNTqk9F3D6jGrfuue67Rxf4zHGeJtxsHABnTgLBmk");
// ⚠️ Anchor заменит этот ID при `anchor build`
//    либо замени вручную после: solana-keygen pubkey target/deploy/simple_amm-keypair.json

#[program]
pub mod simple_amm {
    use super::*;

    /// Создаёт пул: pool PDA, vault_a, vault_b, lp_mint (authority = pool).
    /// fee_bps — комиссия в basis points (30 = 0.3%).
    pub fn initialize_pool(ctx: Context<InitializePool>, fee_bps: u16, bump: u8) -> Result<()> {
        require!(fee_bps <= 1000, AmmError::InvalidFee);

        let pool = &mut ctx.accounts.pool;
        pool.authority = ctx.accounts.authority.key();
        pool.token_a_mint = ctx.accounts.token_a_mint.key();
        pool.token_b_mint = ctx.accounts.token_b_mint.key();
        pool.lp_mint = ctx.accounts.lp_mint.key();
        pool.vault_a = ctx.accounts.vault_a.key();
        pool.vault_b = ctx.accounts.vault_b.key();
        pool.reserve_a = 0;
        pool.reserve_b = 0;
        pool.total_lp = 0;
        pool.fee_bps = fee_bps;
        pool.bump = bump;

        msg!("Pool created: fee={}bps, vaults A/B initialized", fee_bps);
        Ok(())
    }

    /// Пользователь добавляет amount_a + amount_b, получает LP-токены.
    /// - Первый LP: sqrt(amount_a * amount_b).
    /// - Далее: min(amount_a * total_lp / reserve_a, amount_b * total_lp / reserve_b).
    pub fn add_liquidity(
        ctx: Context<AddLiquidity>,
        amount_a: u64,
        amount_b: u64,
        min_lp: u64,
    ) -> Result<()> {
        require!(amount_a > 0 && amount_b > 0, AmmError::ZeroAmount);

        // --- читаем ---
        let pool_bump = ctx.accounts.pool.bump;
        let fee_bps = ctx.accounts.pool.fee_bps;
        let reserve_a_before = ctx.accounts.pool.reserve_a;
        let reserve_b_before = ctx.accounts.pool.reserve_b;
        let total_lp_before = ctx.accounts.pool.total_lp;

        // --- переводим A и B в vault ---
        token::transfer(
            CpiContext::new(
                Token::id(),
                Transfer {
                    from: ctx.accounts.user_a.to_account_info(),
                    to: ctx.accounts.vault_a.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            amount_a,
        )?;
        token::transfer(
            CpiContext::new(
                Token::id(),
                Transfer {
                    from: ctx.accounts.user_b.to_account_info(),
                    to: ctx.accounts.vault_b.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            amount_b,
        )?;

        // --- считаем LP ---
        let lp_to_mint: u64 = if total_lp_before == 0 {
            let prod = (amount_a as u128)
                .checked_mul(amount_b as u128)
                .ok_or(AmmError::MathOverflow)?;
            integer_sqrt(prod) as u64
        } else {
            let lp_a = (amount_a as u128)
                .checked_mul(total_lp_before as u128)
                .ok_or(AmmError::MathOverflow)?
                .checked_div(reserve_a_before as u128)
                .ok_or(AmmError::MathOverflow)? as u64;
            let lp_b = (amount_b as u128)
                .checked_mul(total_lp_before as u128)
                .ok_or(AmmError::MathOverflow)?
                .checked_div(reserve_b_before as u128)
                .ok_or(AmmError::MathOverflow)? as u64;
            lp_a.min(lp_b)
        };
        require!(lp_to_mint >= min_lp, AmmError::SlippageExceeded);
        require!(lp_to_mint > 0, AmmError::ZeroLp);

        // --- минтим LP (подпись pool PDA) ---
        let token_a_key = ctx.accounts.token_a_mint.key();
        let token_b_key = ctx.accounts.token_b_mint.key();
        let seeds = &[
            b"pool",
            token_a_key.as_ref(),
            token_b_key.as_ref(),
            &[pool_bump],
        ];
        let signer = &[&seeds[..]];

        token::mint_to(
            CpiContext::new_with_signer(
                Token::id(),
                MintTo {
                    mint: ctx.accounts.lp_mint.to_account_info(),
                    to: ctx.accounts.user_lp.to_account_info(),
                    authority: ctx.accounts.pool.to_account_info(),
                },
                signer,
            ),
            lp_to_mint,
        )?;

        // --- обновляем ---
        let pool = &mut ctx.accounts.pool;
        pool.reserve_a = pool.reserve_a.checked_add(amount_a).ok_or(AmmError::MathOverflow)?;
        pool.reserve_b = pool.reserve_b.checked_add(amount_b).ok_or(AmmError::MathOverflow)?;
        pool.total_lp = pool.total_lp.checked_add(lp_to_mint).ok_or(AmmError::MathOverflow)?;

        msg!(
            "add_liquidity: {}A+{}B -> {}LP, fee={}bps (reserves {}A/{}B, total_lp={})",
            amount_a, amount_b, lp_to_mint, fee_bps,
            pool.reserve_a, pool.reserve_b, pool.total_lp
        );
        Ok(())
    }

    /// Обмен A→B (a_to_b=true) или B→A (a_to_b=false).
    /// Формула: amount_out = out_reserve * in_after_fee / (in_reserve + in_after_fee)
    /// где in_after_fee = amount_in * (10000 - fee_bps) / 10000.
    pub fn swap(
        ctx: Context<Swap>,
        amount_in: u64,
        min_amount_out: u64,
        a_to_b: bool,
    ) -> Result<()> {
        require!(amount_in > 0, AmmError::ZeroAmount);

        let pool_bump = ctx.accounts.pool.bump;
        let fee_bps = ctx.accounts.pool.fee_bps;

        let (in_reserve, out_reserve) = if a_to_b {
            (ctx.accounts.pool.reserve_a, ctx.accounts.pool.reserve_b)
        } else {
            (ctx.accounts.pool.reserve_b, ctx.accounts.pool.reserve_a)
        };

        // amount_in * (1 - fee)
        let amount_in_after_fee = (amount_in as u128)
            .checked_mul((10_000u128).checked_sub(fee_bps as u128).ok_or(AmmError::MathOverflow)?)
            .ok_or(AmmError::MathOverflow)?
            .checked_div(10_000)
            .ok_or(AmmError::MathOverflow)? as u64;

        // amount_out = out_reserve * in_after_fee / (in_reserve + in_after_fee)
        let numerator = (out_reserve as u128)
            .checked_mul(amount_in_after_fee as u128)
            .ok_or(AmmError::MathOverflow)?;
        let denominator = (in_reserve as u128)
            .checked_add(amount_in_after_fee as u128)
            .ok_or(AmmError::MathOverflow)?;
        let amount_out = numerator.checked_div(denominator).ok_or(AmmError::MathOverflow)? as u64;

        require!(amount_out >= min_amount_out, AmmError::SlippageExceeded);
        require!(amount_out > 0, AmmError::ZeroOut);
        require!(amount_out < out_reserve, AmmError::InsufficientLiquidity);

        // --- определим user_in / user_out ---
        let user_in = if a_to_b {
            ctx.accounts.user_a.to_account_info()
        } else {
            ctx.accounts.user_b.to_account_info()
        };
        let user_out = if a_to_b {
            ctx.accounts.user_b.to_account_info()
        } else {
            ctx.accounts.user_a.to_account_info()
        };
        let vault_in = if a_to_b {
            ctx.accounts.vault_a.to_account_info()
        } else {
            ctx.accounts.vault_b.to_account_info()
        };
        let vault_out = if a_to_b {
            ctx.accounts.vault_b.to_account_info()
        } else {
            ctx.accounts.vault_a.to_account_info()
        };

        // user → vault_in
        token::transfer(
            CpiContext::new(
                Token::id(),
                Transfer {
                    from: user_in,
                    to: vault_in,
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            amount_in,
        )?;

        // vault_out → user (pool PDA signs)
        let token_a_key = ctx.accounts.token_a_mint.key();
        let token_b_key = ctx.accounts.token_b_mint.key();
        let seeds = &[b"pool", token_a_key.as_ref(), token_b_key.as_ref(), &[pool_bump]];
        let signer = &[&seeds[..]];

        token::transfer(
            CpiContext::new_with_signer(
                Token::id(),
                Transfer {
                    from: vault_out,
                    to: user_out,
                    authority: ctx.accounts.pool.to_account_info(),
                },
                signer,
            ),
            amount_out,
        )?;

        // --- обновляем резервы ---
        let pool = &mut ctx.accounts.pool;
        if a_to_b {
            pool.reserve_a = pool.reserve_a.checked_add(amount_in).ok_or(AmmError::MathOverflow)?;
            pool.reserve_b = pool.reserve_b.checked_sub(amount_out).ok_or(AmmError::MathOverflow)?;
        } else {
            pool.reserve_b = pool.reserve_b.checked_add(amount_in).ok_or(AmmError::MathOverflow)?;
            pool.reserve_a = pool.reserve_a.checked_sub(amount_out).ok_or(AmmError::MathOverflow)?;
        }

        msg!(
            "swap {}->{}: {} in, {} out (fee={}bps, reserves {}A/{}B)",
            if a_to_b { "A" } else { "B" },
            if a_to_b { "B" } else { "A" },
            amount_in, amount_out, fee_bps,
            pool.reserve_a, pool.reserve_b
        );
        Ok(())
    }

    /// Сжигает LP, возвращает пропорционально A и B.
    pub fn remove_liquidity(ctx: Context<RemoveLiquidity>, lp_amount: u64) -> Result<()> {
        require!(lp_amount > 0, AmmError::ZeroAmount);

        let pool_bump = ctx.accounts.pool.bump;
        let reserve_a = ctx.accounts.pool.reserve_a;
        let reserve_b = ctx.accounts.pool.reserve_b;
        let total_lp = ctx.accounts.pool.total_lp;
        require!(total_lp > 0, AmmError::InsufficientLiquidity);

        let amount_a = (lp_amount as u128)
            .checked_mul(reserve_a as u128)
            .ok_or(AmmError::MathOverflow)?
            .checked_div(total_lp as u128)
            .ok_or(AmmError::MathOverflow)? as u64;
        let amount_b = (lp_amount as u128)
            .checked_mul(reserve_b as u128)
            .ok_or(AmmError::MathOverflow)?
            .checked_div(total_lp as u128)
            .ok_or(AmmError::MathOverflow)? as u64;

        require!(amount_a > 0 && amount_b > 0, AmmError::ZeroOut);

        // --- burn LP (user signs) ---
        token::burn(
            CpiContext::new(
                Token::id(),
                Burn {
                    mint: ctx.accounts.lp_mint.to_account_info(),
                    from: ctx.accounts.user_lp.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            lp_amount,
        )?;

        // --- transfer A и B (pool PDA signs) ---
        let token_a_key = ctx.accounts.token_a_mint.key();
        let token_b_key = ctx.accounts.token_b_mint.key();
        let seeds = &[b"pool", token_a_key.as_ref(), token_b_key.as_ref(), &[pool_bump]];
        let signer = &[&seeds[..]];

        token::transfer(
            CpiContext::new_with_signer(
                Token::id(),
                Transfer {
                    from: ctx.accounts.vault_a.to_account_info(),
                    to: ctx.accounts.user_a.to_account_info(),
                    authority: ctx.accounts.pool.to_account_info(),
                },
                signer,
            ),
            amount_a,
        )?;
        token::transfer(
            CpiContext::new_with_signer(
                Token::id(),
                Transfer {
                    from: ctx.accounts.vault_b.to_account_info(),
                    to: ctx.accounts.user_b.to_account_info(),
                    authority: ctx.accounts.pool.to_account_info(),
                },
                signer,
            ),
            amount_b,
        )?;

        // --- обновляем ---
        let pool = &mut ctx.accounts.pool;
        pool.reserve_a = pool.reserve_a.checked_sub(amount_a).ok_or(AmmError::MathOverflow)?;
        pool.reserve_b = pool.reserve_b.checked_sub(amount_b).ok_or(AmmError::MathOverflow)?;
        pool.total_lp = pool.total_lp.checked_sub(lp_amount).ok_or(AmmError::MathOverflow)?;

        msg!(
            "remove_liquidity: {} LP burned -> {}A + {}B (reserves {}A/{}B)",
            lp_amount, amount_a, amount_b, pool.reserve_a, pool.reserve_b
        );
        Ok(())
    }
}

// ===================== ВСПОМОГАТЕЛЬНОЕ =====================

fn integer_sqrt(n: u128) -> u128 {
    if n == 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

// ===================== КОНТЕКСТЫ =====================

#[derive(Accounts)]
pub struct InitializePool<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + 32*6 + 8*3 + 2 + 1,
        seeds = [b"pool", token_a_mint.key().as_ref(), token_b_mint.key().as_ref()],
        bump
    )]
    pub pool: Account<'info, Pool>,

    pub token_a_mint: Account<'info, Mint>,
    pub token_b_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = authority,
        mint::decimals = 9,
        mint::authority = pool,
        seeds = [b"lp_mint", pool.key().as_ref()],
        bump,
    )]
    pub lp_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = authority,
        token::mint = token_a_mint,
        token::authority = pool,
        seeds = [b"vault_a", pool.key().as_ref()],
        bump,
    )]
    pub vault_a: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = authority,
        token::mint = token_b_mint,
        token::authority = pool,
        seeds = [b"vault_b", pool.key().as_ref()],
        bump,
    )]
    pub vault_b: Account<'info, TokenAccount>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct AddLiquidity<'info> {
    #[account(
        mut,
        seeds = [b"pool", token_a_mint.key().as_ref(), token_b_mint.key().as_ref()],
        bump = pool.bump
    )]
    pub pool: Account<'info, Pool>,

    pub token_a_mint: Account<'info, Mint>,
    pub token_b_mint: Account<'info, Mint>,

    #[account(mut, address = pool.vault_a)]
    pub vault_a: Account<'info, TokenAccount>,

    #[account(mut, address = pool.vault_b)]
    pub vault_b: Account<'info, TokenAccount>,

    #[account(mut, address = pool.lp_mint)]
    pub lp_mint: Account<'info, Mint>,

    #[account(mut)]
    pub user_a: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_b: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_lp: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Swap<'info> {
    #[account(
        mut,
        seeds = [b"pool", token_a_mint.key().as_ref(), token_b_mint.key().as_ref()],
        bump = pool.bump
    )]
    pub pool: Account<'info, Pool>,

    pub token_a_mint: Account<'info, Mint>,
    pub token_b_mint: Account<'info, Mint>,

    #[account(mut, address = pool.vault_a)]
    pub vault_a: Account<'info, TokenAccount>,

    #[account(mut, address = pool.vault_b)]
    pub vault_b: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user_a: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_b: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct RemoveLiquidity<'info> {
    #[account(
        mut,
        seeds = [b"pool", token_a_mint.key().as_ref(), token_b_mint.key().as_ref()],
        bump = pool.bump
    )]
    pub pool: Account<'info, Pool>,

    pub token_a_mint: Account<'info, Mint>,
    pub token_b_mint: Account<'info, Mint>,

    #[account(mut, address = pool.vault_a)]
    pub vault_a: Account<'info, TokenAccount>,

    #[account(mut, address = pool.vault_b)]
    pub vault_b: Account<'info, TokenAccount>,

    #[account(mut, address = pool.lp_mint)]
    pub lp_mint: Account<'info, Mint>,

    #[account(mut)]
    pub user_a: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_b: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_lp: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub token_program: Program<'info, Token>,
}

// ===================== STATE =====================

#[account]
pub struct Pool {
    pub authority: Pubkey,      // 32
    pub token_a_mint: Pubkey,   // 32
    pub token_b_mint: Pubkey,   // 32
    pub lp_mint: Pubkey,        // 32
    pub vault_a: Pubkey,        // 32
    pub vault_b: Pubkey,        // 32
    pub reserve_a: u64,         // 8
    pub reserve_b: u64,         // 8
    pub total_lp: u64,          // 8
    pub fee_bps: u16,           // 2
    pub bump: u8,               // 1
}
