use anchor_lang::prelude::*;
use anchor_spl::token_2022::Token2022;
use resource_manager as rm;
use resource_manager::cpi::accounts::MintResource;

declare_id!("8hbndEaekE2x7qKYBdWhVqqm5sHbnpZZ7pJkgdZ3xrhg");

pub const SEARCH_COOLDOWN: i64 = 60;
pub const RESOURCE_COUNT: usize = 6;

#[account]
pub struct Player {
    pub owner: Pubkey,
    pub last_search_timestamp: i64,
    pub bump: u8,
}

impl Player {
    pub const SIZE: usize = 8 + 32 + 8 + 1;
}

#[program]
pub mod search {
    use super::*;

    pub fn register_player(ctx: Context<RegisterPlayer>) -> Result<()> {
        let player = &mut ctx.accounts.player;
        player.owner = ctx.accounts.owner.key();
        player.last_search_timestamp = 0;
        player.bump = ctx.bumps.player;
        msg!("Player registered: {}", player.owner);
        Ok(())
    }

    /// Search for resources. Mints 3 random resources via CPI.
    /// Remaining accounts: 6 pairs of [resource_mint, player_ata] ordered by index.
    pub fn search_resources<'info>(
        ctx: Context<'_, '_, '_, 'info, SearchResources<'info>>,
    ) -> Result<()> {
        let player = &mut ctx.accounts.player;
        let clock = Clock::get()?;
        let now = clock.unix_timestamp;

        require!(
            now - player.last_search_timestamp >= SEARCH_COOLDOWN,
            SearchError::CooldownNotExpired
        );
        player.last_search_timestamp = now;

        let seed = clock.slot.wrapping_add(now as u64);
        let resource_indices = [
            ((seed % 6) as usize),
            (((seed / 6) % 6) as usize),
            (((seed / 36) % 6) as usize),
        ];

        let remaining = &ctx.remaining_accounts;
        require!(remaining.len() >= RESOURCE_COUNT * 2, SearchError::NotEnoughAccounts);

        let game_config = &ctx.accounts.game_config;

        for &idx in &resource_indices {
            let resource_mint = &remaining[idx * 2];
            let player_ata = &remaining[idx * 2 + 1];

            require!(
                resource_mint.key() == game_config.resource_mints[idx],
                SearchError::InvalidResourceMint
            );

            // CPI to resource_manager::mint_resource
            let cpi_accounts = MintResource {
                player: ctx.accounts.owner.to_account_info(),
                mint_authority: ctx.accounts.mint_authority.to_account_info(),
                resource_mint: resource_mint.to_account_info(),
                player_token_account: player_ata.to_account_info(),
                token_program: ctx.accounts.token_program.to_account_info(),
            };
            let cpi_ctx = CpiContext::new(
                ctx.accounts.resource_manager_program.to_account_info(),
                cpi_accounts,
            );
            rm::cpi::mint_resource(cpi_ctx, 1)?;

            msg!("Minted resource [{}] to player", idx);
        }

        msg!("Search complete: [{}, {}, {}]", resource_indices[0], resource_indices[1], resource_indices[2]);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct RegisterPlayer<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        init,
        payer = owner,
        space = Player::SIZE,
        seeds = [b"player", owner.key().as_ref()],
        bump,
    )]
    pub player: Account<'info, Player>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SearchResources<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        seeds = [b"player", owner.key().as_ref()],
        bump = player.bump,
        has_one = owner @ SearchError::Unauthorized,
    )]
    pub player: Account<'info, Player>,

    /// Game config from resource_manager
    #[account(
        seeds = [b"game_config"],
        bump = game_config.bump,
        seeds::program = resource_manager_program.key(),
    )]
    pub game_config: Account<'info, rm::GameConfig>,

    /// Mint authority PDA from resource_manager (needed for CPI)
    /// CHECK: Verified by resource_manager during CPI
    #[account(
        seeds = [b"mint_authority"],
        bump = mint_authority.bump,
        seeds::program = resource_manager_program.key(),
    )]
    pub mint_authority: Account<'info, rm::MintAuthority>,

    pub resource_manager_program: Program<'info, rm::program::ResourceManager>,
    pub token_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

#[error_code]
pub enum SearchError {
    #[msg("Search cooldown not expired. Wait 60 seconds.")]
    CooldownNotExpired,
    #[msg("Unauthorized.")]
    Unauthorized,
    #[msg("Not enough remaining accounts.")]
    NotEnoughAccounts,
    #[msg("Invalid resource mint.")]
    InvalidResourceMint,
}
