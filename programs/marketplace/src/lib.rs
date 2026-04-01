use anchor_lang::prelude::*;
use anchor_spl::token_2022::{self, Token2022, Burn};
use item_nft::ItemMetadata;

declare_id!("8K1tuWtT4vow5wWpwraNEvir84JfHm86DHYBpdrLxpq");

#[account]
pub struct Listing {
    pub seller: Pubkey,
    pub item_mint: Pubkey,
    pub item_type: u8,
    pub price: u64,
    pub active: bool,
    pub bump: u8,
}

impl Listing {
    pub const SIZE: usize = 8 + 32 + 32 + 1 + 8 + 1 + 1;
}

#[program]
pub mod marketplace {
    use super::*;

    /// List an item for sale.
    pub fn list_item(ctx: Context<ListItem>, price: u64) -> Result<()> {
        require!(price > 0, MarketError::InvalidPrice);

        let listing = &mut ctx.accounts.listing;
        listing.seller = ctx.accounts.seller.key();
        listing.item_mint = ctx.accounts.item_mint.key();
        listing.item_type = ctx.accounts.item_metadata.item_type;
        listing.price = price;
        listing.active = true;
        listing.bump = ctx.bumps.listing;

        msg!(
            "Listed {} for {} MagicTokens",
            item_nft::ITEM_NAMES[listing.item_type as usize],
            price
        );
        Ok(())
    }

    /// Buy an item: burn NFT + mint MagicTokens to seller via CPI.
    pub fn buy_item(ctx: Context<BuyItem>) -> Result<()> {
        let listing = &mut ctx.accounts.listing;
        require!(listing.active, MarketError::ListingNotActive);

        let price = listing.price;
        listing.active = false;

        // Burn the NFT
        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Burn {
                mint: ctx.accounts.item_mint.to_account_info(),
                from: ctx.accounts.seller_item_token_account.to_account_info(),
                authority: ctx.accounts.seller.to_account_info(),
            },
        );
        token_2022::burn(cpi_ctx, 1)?;

        // Mint MagicTokens to seller via CPI
        let cpi_accounts = magic_token::cpi::accounts::MintMagicToken {
            player: ctx.accounts.seller.to_account_info(),
            magic_config: ctx.accounts.magic_config.to_account_info(),
            magic_mint_authority: ctx.accounts.magic_mint_authority.to_account_info(),
            magic_mint: ctx.accounts.magic_mint.to_account_info(),
            player_token_account: ctx.accounts.seller_magic_token_account.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(
            ctx.accounts.magic_token_program.to_account_info(),
            cpi_accounts,
        );
        magic_token::cpi::mint_magic_token(cpi_ctx, price)?;

        msg!("Item sold for {} MagicTokens", price);
        Ok(())
    }

    /// Cancel a listing.
    pub fn cancel_listing(ctx: Context<CancelListing>) -> Result<()> {
        let listing = &mut ctx.accounts.listing;
        require!(listing.active, MarketError::ListingNotActive);
        listing.active = false;
        msg!("Listing cancelled");
        Ok(())
    }
}

#[derive(Accounts)]
pub struct ListItem<'info> {
    #[account(mut)]
    pub seller: Signer<'info>,

    /// CHECK: Item NFT mint
    pub item_mint: AccountInfo<'info>,

    #[account(
        seeds = [b"item_metadata", item_mint.key().as_ref()],
        bump = item_metadata.bump,
        seeds::program = item_nft_program.key(),
        constraint = item_metadata.owner == seller.key() @ MarketError::NotItemOwner,
    )]
    pub item_metadata: Account<'info, ItemMetadata>,

    #[account(
        init,
        payer = seller,
        space = Listing::SIZE,
        seeds = [b"listing", item_mint.key().as_ref()],
        bump,
    )]
    pub listing: Account<'info, Listing>,

    pub item_nft_program: Program<'info, item_nft::program::ItemNft>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct BuyItem<'info> {
    #[account(mut)]
    pub buyer: Signer<'info>,

    /// CHECK: Seller (must sign for NFT burn)
    #[account(mut)]
    pub seller: Signer<'info>,

    /// CHECK: Item NFT mint
    #[account(mut)]
    pub item_mint: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [b"listing", item_mint.key().as_ref()],
        bump = listing.bump,
        constraint = listing.seller == seller.key() @ MarketError::SellerMismatch,
    )]
    pub listing: Account<'info, Listing>,

    /// CHECK: Seller's token account holding the NFT
    #[account(mut)]
    pub seller_item_token_account: AccountInfo<'info>,

    /// CHECK: MagicToken mint
    #[account(mut)]
    pub magic_mint: AccountInfo<'info>,

    /// CHECK: MagicConfig PDA
    pub magic_config: AccountInfo<'info>,

    /// CHECK: MagicMintAuthority PDA
    pub magic_mint_authority: AccountInfo<'info>,

    /// CHECK: Seller's MagicToken account
    #[account(mut)]
    pub seller_magic_token_account: AccountInfo<'info>,

    pub magic_token_program: Program<'info, magic_token::program::MagicToken>,
    pub token_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CancelListing<'info> {
    #[account(mut)]
    pub seller: Signer<'info>,

    /// CHECK: Item NFT mint
    pub item_mint: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [b"listing", item_mint.key().as_ref()],
        bump = listing.bump,
        constraint = listing.seller == seller.key() @ MarketError::NotItemOwner,
    )]
    pub listing: Account<'info, Listing>,
}

#[error_code]
pub enum MarketError {
    #[msg("Invalid price.")]
    InvalidPrice,
    #[msg("Listing not active.")]
    ListingNotActive,
    #[msg("Not item owner.")]
    NotItemOwner,
    #[msg("Seller mismatch.")]
    SellerMismatch,
}
