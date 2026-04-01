use anchor_lang::prelude::*;
use anchor_spl::token_2022::{self, Token2022, MintTo, Burn};

declare_id!("7d5r9XEy9kNNxU7uHuq8p3oiPPocz2LnTyi6UpydFRQh");

/// Item types available in the game
pub const ITEM_COUNT: usize = 4;

/// Item names
pub const ITEM_NAMES: [&str; ITEM_COUNT] = [
    "Cossack Saber",
    "Elder Staff",
    "Kharakternik Armor",
    "Battle Bracelet",
];

/// Item symbols
pub const ITEM_SYMBOLS: [&str; ITEM_COUNT] = [
    "SABER", "STAFF", "ARMOR", "BRACELET",
];

/// Recipes: each item requires specific resource amounts.
/// Index = resource_id (0=WOOD,1=IRON,2=GOLD,3=LEATHER,4=STONE,5=DIAMOND)
pub const RECIPES: [[u8; 6]; ITEM_COUNT] = [
    // Saber: 1 WOOD + 3 IRON + 1 LEATHER
    [1, 3, 0, 1, 0, 0],
    // Staff: 2 WOOD + 1 GOLD + 1 DIAMOND
    [2, 0, 1, 0, 0, 1],
    // Armor: 2 IRON + 1 GOLD + 4 LEATHER
    [0, 2, 1, 4, 0, 0],
    // Bracelet: 4 IRON + 2 GOLD + 2 DIAMOND
    [0, 4, 2, 0, 0, 2],
];

/// Metadata for a crafted item NFT
#[account]
pub struct ItemMetadata {
    /// Item type (0-3)
    pub item_type: u8,
    /// Current owner
    pub owner: Pubkey,
    /// NFT mint address
    pub mint: Pubkey,
    /// PDA bump
    pub bump: u8,
}

impl ItemMetadata {
    pub const SIZE: usize = 8 + 1 + 32 + 32 + 1;
}

/// Config for item_nft program
#[account]
pub struct ItemConfig {
    pub admin: Pubkey,
    /// Counter for generating unique item IDs
    pub item_counter: u64,
    pub bump: u8,
}

impl ItemConfig {
    pub const SIZE: usize = 8 + 32 + 8 + 1;
}

#[program]
pub mod item_nft {
    use super::*;

    /// Initialize the item NFT config
    pub fn initialize(ctx: Context<InitializeItemConfig>) -> Result<()> {
        let config = &mut ctx.accounts.item_config;
        config.admin = ctx.accounts.admin.key();
        config.item_counter = 0;
        config.bump = ctx.bumps.item_config;

        msg!("ItemNFT program initialized");
        Ok(())
    }

    /// Mint an NFT for a crafted item.
    /// Called via CPI from crafting program.
    pub fn mint_item(
        ctx: Context<MintItem>,
        item_type: u8,
    ) -> Result<()> {
        require!(
            (item_type as usize) < ITEM_COUNT,
            ItemError::InvalidItemType
        );

        // Increment counter for unique ID
        let config = &mut ctx.accounts.item_config;
        let item_id = config.item_counter;
        config.item_counter += 1;

        // Store item metadata
        let metadata = &mut ctx.accounts.item_metadata;
        metadata.item_type = item_type;
        metadata.owner = ctx.accounts.player.key();
        metadata.mint = ctx.accounts.item_mint.key();
        metadata.bump = ctx.bumps.item_metadata;

        // Mint 1 NFT to the player
        let bump = config.bump;
        let signer_seeds: &[&[&[u8]]] = &[&[b"item_config", &[bump]]];

        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            MintTo {
                mint: ctx.accounts.item_mint.to_account_info(),
                to: ctx.accounts.player_token_account.to_account_info(),
                authority: ctx.accounts.item_config.to_account_info(),
            },
            signer_seeds,
        );
        token_2022::mint_to(cpi_ctx, 1)?;

        msg!(
            "Minted item NFT: {} (id={}) to {}",
            ITEM_NAMES[item_type as usize],
            item_id,
            ctx.accounts.player.key()
        );
        Ok(())
    }

    /// Burn an item NFT. Called via CPI from marketplace program.
    pub fn burn_item(ctx: Context<BurnItem>) -> Result<()> {
        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Burn {
                mint: ctx.accounts.item_mint.to_account_info(),
                from: ctx.accounts.player_token_account.to_account_info(),
                authority: ctx.accounts.player.to_account_info(),
            },
        );
        token_2022::burn(cpi_ctx, 1)?;

        msg!("Burned item NFT: {}", ctx.accounts.item_mint.key());
        Ok(())
    }
}

/// Accounts for initialize
#[derive(Accounts)]
pub struct InitializeItemConfig<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        init,
        payer = admin,
        space = ItemConfig::SIZE,
        seeds = [b"item_config"],
        bump,
    )]
    pub item_config: Account<'info, ItemConfig>,

    pub system_program: Program<'info, System>,
}

/// Accounts for mint_item
#[derive(Accounts)]
#[instruction(item_type: u8)]
pub struct MintItem<'info> {
    #[account(mut)]
    pub player: Signer<'info>,

    #[account(
        mut,
        seeds = [b"item_config"],
        bump = item_config.bump,
    )]
    pub item_config: Account<'info, ItemConfig>,

    /// CHECK: NFT mint account, created by caller
    #[account(mut)]
    pub item_mint: AccountInfo<'info>,

    /// Item metadata PDA
    #[account(
        init,
        payer = player,
        space = ItemMetadata::SIZE,
        seeds = [b"item_metadata", item_mint.key().as_ref()],
        bump,
    )]
    pub item_metadata: Account<'info, ItemMetadata>,

    /// CHECK: Player's token account for the NFT
    #[account(mut)]
    pub player_token_account: AccountInfo<'info>,

    pub token_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

/// Accounts for burn_item
#[derive(Accounts)]
pub struct BurnItem<'info> {
    pub player: Signer<'info>,

    #[account(
        seeds = [b"item_config"],
        bump = item_config.bump,
    )]
    pub item_config: Account<'info, ItemConfig>,

    /// CHECK: NFT mint to burn
    #[account(mut)]
    pub item_mint: AccountInfo<'info>,

    /// CHECK: Player's token account holding the NFT
    #[account(mut)]
    pub player_token_account: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [b"item_metadata", item_mint.key().as_ref()],
        bump = item_metadata.bump,
        close = player,
    )]
    pub item_metadata: Account<'info, ItemMetadata>,

    pub token_program: Program<'info, Token2022>,
}

#[error_code]
pub enum ItemError {
    #[msg("Invalid item type. Must be 0-3.")]
    InvalidItemType,
}
