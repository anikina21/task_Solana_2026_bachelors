use anchor_lang::prelude::*;
use anchor_spl::token_2022::{self, Token2022, MintTo, Burn};

declare_id!("691PdUpsPbrTAWHkiBNXHjk49FMWoBrTRUsmsyNT7Tjm");

pub const RESOURCE_COUNT: usize = 6;
pub const RESOURCE_SYMBOLS: [&str; RESOURCE_COUNT] = [
    "WOOD", "IRON", "GOLD", "LEATHER", "STONE", "DIAMOND",
];

#[account]
pub struct GameConfig {
    pub admin: Pubkey,
    pub resource_mints: [Pubkey; RESOURCE_COUNT],
    pub magic_token_mint: Pubkey,
    pub item_prices: [u64; 4],
    pub bump: u8,
}

impl GameConfig {
    pub const SIZE: usize = 8 + 32 + 32 * RESOURCE_COUNT + 32 + 8 * 4 + 1;
}

/// Mint authority PDA — separate from game_config so it can sign CPI calls
#[account]
pub struct MintAuthority {
    pub bump: u8,
}

impl MintAuthority {
    pub const SIZE: usize = 8 + 1;
}

#[program]
pub mod resource_manager {
    use super::*;

    /// Initialize game config and mint authority PDA.
    pub fn initialize_game(
        ctx: Context<InitializeGame>,
        item_prices: [u64; 4],
    ) -> Result<()> {
        let config = &mut ctx.accounts.game_config;
        config.admin = ctx.accounts.admin.key();
        config.resource_mints = [Pubkey::default(); RESOURCE_COUNT];
        config.magic_token_mint = Pubkey::default();
        config.item_prices = item_prices;
        config.bump = ctx.bumps.game_config;

        let auth = &mut ctx.accounts.mint_authority;
        auth.bump = ctx.bumps.mint_authority;

        msg!("Game initialized by admin: {}", config.admin);
        Ok(())
    }

    /// Create a resource mint (Token-2022, decimals=0).
    /// Mint authority = mint_authority PDA.
    pub fn create_resource_mint(
        ctx: Context<CreateResourceMint>,
        resource_index: u8,
    ) -> Result<()> {
        require!(
            (resource_index as usize) < RESOURCE_COUNT,
            GameError::InvalidResourceIndex
        );

        let config = &mut ctx.accounts.game_config;
        config.resource_mints[resource_index as usize] = ctx.accounts.resource_mint.key();

        msg!(
            "Created resource mint [{}] {}: {}",
            resource_index,
            RESOURCE_SYMBOLS[resource_index as usize],
            ctx.accounts.resource_mint.key()
        );
        Ok(())
    }

    /// Mint resources to a player. Called via CPI from search/crafting.
    pub fn mint_resource(
        ctx: Context<MintResource>,
        amount: u64,
    ) -> Result<()> {
        let bump = ctx.accounts.mint_authority.bump;
        let signer_seeds: &[&[&[u8]]] = &[&[b"mint_authority", &[bump]]];

        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            MintTo {
                mint: ctx.accounts.resource_mint.to_account_info(),
                to: ctx.accounts.player_token_account.to_account_info(),
                authority: ctx.accounts.mint_authority.to_account_info(),
            },
            signer_seeds,
        );
        token_2022::mint_to(cpi_ctx, amount)?;

        msg!("Minted {} resources to {}", amount, ctx.accounts.player.key());
        Ok(())
    }

    /// Burn resources from a player. Called via CPI from crafting.
    pub fn burn_resource(
        ctx: Context<BurnResource>,
        amount: u64,
    ) -> Result<()> {
        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Burn {
                mint: ctx.accounts.resource_mint.to_account_info(),
                from: ctx.accounts.player_token_account.to_account_info(),
                authority: ctx.accounts.player.to_account_info(),
            },
        );
        token_2022::burn(cpi_ctx, amount)?;

        msg!("Burned {} resources from {}", amount, ctx.accounts.player.key());
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializeGame<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        init,
        payer = admin,
        space = GameConfig::SIZE,
        seeds = [b"game_config"],
        bump,
    )]
    pub game_config: Account<'info, GameConfig>,

    #[account(
        init,
        payer = admin,
        space = MintAuthority::SIZE,
        seeds = [b"mint_authority"],
        bump,
    )]
    pub mint_authority: Account<'info, MintAuthority>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(resource_index: u8)]
pub struct CreateResourceMint<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [b"game_config"],
        bump = game_config.bump,
        has_one = admin @ GameError::Unauthorized,
    )]
    pub game_config: Account<'info, GameConfig>,

    /// Mint authority PDA — set as the mint's authority
    #[account(
        seeds = [b"mint_authority"],
        bump = mint_authority.bump,
    )]
    pub mint_authority: Account<'info, MintAuthority>,

    /// The new resource mint (Token-2022, decimals = 0)
    #[account(
        init,
        payer = admin,
        mint::decimals = 0,
        mint::authority = mint_authority,
        mint::token_program = token_program,
        seeds = [b"resource_mint" as &[u8], &[resource_index]],
        bump,
    )]
    pub resource_mint: InterfaceAccount<'info, anchor_spl::token_interface::Mint>,

    pub token_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct MintResource<'info> {
    /// CHECK: Player receiving resources
    pub player: AccountInfo<'info>,

    #[account(
        seeds = [b"mint_authority"],
        bump = mint_authority.bump,
    )]
    pub mint_authority: Account<'info, MintAuthority>,

    /// CHECK: Resource mint (Token-2022)
    #[account(mut)]
    pub resource_mint: AccountInfo<'info>,

    /// CHECK: Player's token account
    #[account(mut)]
    pub player_token_account: AccountInfo<'info>,

    pub token_program: Program<'info, Token2022>,
}

#[derive(Accounts)]
pub struct BurnResource<'info> {
    pub player: Signer<'info>,

    /// CHECK: Resource mint (Token-2022)
    #[account(mut)]
    pub resource_mint: AccountInfo<'info>,

    /// CHECK: Player's token account
    #[account(mut)]
    pub player_token_account: AccountInfo<'info>,

    pub token_program: Program<'info, Token2022>,
}

#[error_code]
pub enum GameError {
    #[msg("Invalid resource index. Must be 0-5.")]
    InvalidResourceIndex,
    #[msg("Unauthorized: only admin can perform this action.")]
    Unauthorized,
}
