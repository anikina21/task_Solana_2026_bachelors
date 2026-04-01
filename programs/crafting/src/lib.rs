use anchor_lang::prelude::*;
use anchor_spl::token_2022::Token2022;
use item_nft::RECIPES;
use resource_manager::GameConfig;
use resource_manager::RESOURCE_COUNT;

declare_id!("Dc4NvUc5fWDz3HZcWatnsCb4CupHGb1UGWawRhYVWk8q");

#[program]
pub mod crafting {
    use super::*;

    /// Craft an item: burn resources + mint NFT via CPI.
    /// Remaining accounts: pairs of [resource_mint, player_token_account]
    /// for resources 0-5 (12 accounts total).
    pub fn craft_item<'info>(
        ctx: Context<'_, '_, '_, 'info, CraftItem<'info>>,
        item_type: u8,
    ) -> Result<()> {
        require!(
            (item_type as usize) < item_nft::ITEM_COUNT,
            CraftError::InvalidItemType
        );

        let recipe = RECIPES[item_type as usize];
        let remaining = &ctx.remaining_accounts;
        require!(remaining.len() >= RESOURCE_COUNT * 2, CraftError::NotEnoughAccounts);

        let game_config = &ctx.accounts.game_config;

        // Burn required resources
        for i in 0..RESOURCE_COUNT {
            let amount = recipe[i] as u64;
            if amount == 0 {
                continue;
            }

            let resource_mint = &remaining[i * 2];
            let player_token_account = &remaining[i * 2 + 1];

            // Verify correct resource mint
            require!(
                resource_mint.key() == game_config.resource_mints[i],
                CraftError::InvalidResourceMint
            );

            // Burn via CPI to resource_manager
            let cpi_accounts = resource_manager::cpi::accounts::BurnResource {
                player: ctx.accounts.player.to_account_info(),
                resource_mint: resource_mint.to_account_info(),
                player_token_account: player_token_account.to_account_info(),
                token_program: ctx.accounts.token_program.to_account_info(),
            };
            let cpi_ctx = CpiContext::new(
                ctx.accounts.resource_manager_program.to_account_info(),
                cpi_accounts,
            );
            resource_manager::cpi::burn_resource(cpi_ctx, amount)?;

            msg!("Burned {} of resource [{}]", amount, i);
        }

        // Mint NFT via CPI to item_nft
        let cpi_accounts = item_nft::cpi::accounts::MintItem {
            player: ctx.accounts.player.to_account_info(),
            item_config: ctx.accounts.item_config.to_account_info(),
            item_mint: ctx.accounts.item_mint.to_account_info(),
            item_metadata: ctx.accounts.item_metadata.to_account_info(),
            player_token_account: ctx.accounts.player_item_token_account.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(
            ctx.accounts.item_nft_program.to_account_info(),
            cpi_accounts,
        );
        item_nft::cpi::mint_item(cpi_ctx, item_type)?;

        msg!("Crafted item: {}", item_nft::ITEM_NAMES[item_type as usize]);
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(item_type: u8)]
pub struct CraftItem<'info> {
    #[account(mut)]
    pub player: Signer<'info>,

    #[account(
        seeds = [b"game_config"],
        bump = game_config.bump,
        seeds::program = resource_manager_program.key(),
    )]
    pub game_config: Account<'info, GameConfig>,

    /// CHECK: Item mint for the new NFT (created externally)
    #[account(mut)]
    pub item_mint: AccountInfo<'info>,

    /// CHECK: Item config from item_nft program
    #[account(mut)]
    pub item_config: AccountInfo<'info>,

    /// CHECK: Item metadata PDA (initialized by item_nft)
    #[account(mut)]
    pub item_metadata: AccountInfo<'info>,

    /// CHECK: Player's token account for the new NFT
    #[account(mut)]
    pub player_item_token_account: AccountInfo<'info>,

    pub resource_manager_program: Program<'info, resource_manager::program::ResourceManager>,
    pub item_nft_program: Program<'info, item_nft::program::ItemNft>,
    pub token_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

#[error_code]
pub enum CraftError {
    #[msg("Invalid item type.")]
    InvalidItemType,
    #[msg("Not enough remaining accounts.")]
    NotEnoughAccounts,
    #[msg("Invalid resource mint address.")]
    InvalidResourceMint,
}
