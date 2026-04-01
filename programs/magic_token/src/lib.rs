use anchor_lang::prelude::*;
use anchor_spl::token_2022::{self, Token2022, MintTo};

declare_id!("GBNDw9juW9fdTsvSfNQNgeQ15byZxL9yHJfoDn5eSpST");

#[account]
pub struct MagicConfig {
    pub admin: Pubkey,
    pub mint: Pubkey,
    pub bump: u8,
    pub mint_authority_bump: u8,
}

impl MagicConfig {
    pub const SIZE: usize = 8 + 32 + 32 + 1 + 1;
}

/// Mint authority PDA for MagicToken
#[account]
pub struct MagicMintAuthority {
    pub bump: u8,
}

impl MagicMintAuthority {
    pub const SIZE: usize = 8 + 1;
}

#[program]
pub mod magic_token {
    use super::*;

    /// Initialize MagicToken: config + mint authority + mint (Token-2022).
    pub fn initialize(ctx: Context<InitializeMagic>) -> Result<()> {
        let config = &mut ctx.accounts.magic_config;
        config.admin = ctx.accounts.admin.key();
        config.mint = ctx.accounts.magic_mint.key();
        config.bump = ctx.bumps.magic_config;
        config.mint_authority_bump = ctx.bumps.magic_mint_authority;

        let auth = &mut ctx.accounts.magic_mint_authority;
        auth.bump = ctx.bumps.magic_mint_authority;

        msg!("MagicToken initialized. Mint: {}", config.mint);
        Ok(())
    }

    /// Mint MagicTokens to a player. Called via CPI from marketplace.
    pub fn mint_magic_token(
        ctx: Context<MintMagicToken>,
        amount: u64,
    ) -> Result<()> {
        let bump = ctx.accounts.magic_mint_authority.bump;
        let signer_seeds: &[&[&[u8]]] = &[&[b"magic_mint_authority", &[bump]]];

        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            MintTo {
                mint: ctx.accounts.magic_mint.to_account_info(),
                to: ctx.accounts.player_token_account.to_account_info(),
                authority: ctx.accounts.magic_mint_authority.to_account_info(),
            },
            signer_seeds,
        );
        token_2022::mint_to(cpi_ctx, amount)?;

        msg!("Minted {} MagicTokens to {}", amount, ctx.accounts.player.key());
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializeMagic<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        init,
        payer = admin,
        space = MagicConfig::SIZE,
        seeds = [b"magic_config"],
        bump,
    )]
    pub magic_config: Account<'info, MagicConfig>,

    #[account(
        init,
        payer = admin,
        space = MagicMintAuthority::SIZE,
        seeds = [b"magic_mint_authority"],
        bump,
    )]
    pub magic_mint_authority: Account<'info, MagicMintAuthority>,

    /// MagicToken mint (Token-2022, decimals=0)
    /// Authority = magic_mint_authority PDA
    #[account(
        init,
        payer = admin,
        mint::decimals = 0,
        mint::authority = magic_mint_authority,
        mint::token_program = token_program,
        seeds = [b"magic_token_mint"],
        bump,
    )]
    pub magic_mint: InterfaceAccount<'info, anchor_spl::token_interface::Mint>,

    pub token_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct MintMagicToken<'info> {
    /// CHECK: Player receiving MagicTokens
    pub player: AccountInfo<'info>,

    #[account(
        seeds = [b"magic_config"],
        bump = magic_config.bump,
    )]
    pub magic_config: Account<'info, MagicConfig>,

    #[account(
        seeds = [b"magic_mint_authority"],
        bump = magic_mint_authority.bump,
    )]
    pub magic_mint_authority: Account<'info, MagicMintAuthority>,

    /// CHECK: MagicToken mint
    #[account(
        mut,
        constraint = magic_mint.key() == magic_config.mint @ MagicError::InvalidMint,
    )]
    pub magic_mint: AccountInfo<'info>,

    /// CHECK: Player's MagicToken account
    #[account(mut)]
    pub player_token_account: AccountInfo<'info>,

    pub token_program: Program<'info, Token2022>,
}

#[error_code]
pub enum MagicError {
    #[msg("Invalid MagicToken mint address.")]
    InvalidMint,
}
