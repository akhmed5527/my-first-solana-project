use anchor_lang::prelude::*;

declare_id!("2SzinKjtqKfnq2eSRC3UYwdvGeVPiNapAy3a7Z9WK8V8");

#[program]
pub mod week6_pda_profile {
    use super::*;

    // Создаем профиль
    pub fn create_profile(ctx: Context<CreateProfile>, name: String, bio: String) -> Result<()> {
        let profile = &mut ctx.accounts.profile;
        profile.authority = ctx.accounts.user.key();
        profile.name = name;
        profile.bio = bio;
        Ok(())
    }

    // Обновляем профиль
    pub fn update_profile(ctx: Context<UpdateProfile>, new_bio: String) -> Result<()> {
        let profile = &mut ctx.accounts.profile;
        profile.bio = new_bio;
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(name: String)]
pub struct CreateProfile<'info> {
    #[account(
        init,
        payer = user,
        space = 8 + 32 + 4 + 50 + 4 + 200, // 8 (дискриминатор) + 32 (Pubkey) + строки (4 байта длины + данные)
        seeds = [b"profile", user.key().as_ref()], // Уникальный PDA для каждого юзера
        bump
    )]
    pub profile: Account<'info, Profile>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateProfile<'info> {
    #[account(
        mut,
        seeds = [b"profile", user.key().as_ref()],
        bump,
        has_one = authority // Проверка, что обновляет именно владелец
    )]
    pub profile: Account<'info, Profile>,
    pub authority: Signer<'info>,
    pub user: Signer<'info>, 
}

#[account]
pub struct Profile {
    pub authority: Pubkey,
    pub name: String,
    pub bio: String,
}
