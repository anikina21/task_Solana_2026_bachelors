# Cossack Business — Solana On-Chain Game

Blockchain-based resource management game on Solana (Anchor Framework).
Players search for resources, craft unique NFT items, and trade them on a marketplace for MagicTokens.

## Program IDs (Devnet)

| Program | Program ID |
|---------|-----------|
| resource_manager | `691PdUpsPbrTAWHkiBNXHjk49FMWoBrTRUsmsyNT7Tjm` |
| magic_token | `Gy652wSGwe1tiAhAMNb4xcVsEcVVJD1qvVDeLdDDKPCf` |
| search | `8hbndEaekE2x7qKYBdWhVqqm5sHbnpZZ7pJkgdZ3xrhg` |
| item_nft | `E3f7Fab3eH6J6DMcyd4yoXhtrmFCMJhu21zZKfjK33ha` |
| crafting | `FHqRv6fKfxXiXLe1xWeTxwwK7nEEoCBjR54shEvsQptC` |
| marketplace | `H9qAUvfX5qCncku494UC1t2Yg6SXmEsinNuPdcmWWQLU` |

## Architecture

6 programs communicating via CPI (Cross-Program Invocation):
```
search ──CPI──> resource_manager ──CPI──> Token-2022 (mint resources)
crafting ──CPI──> resource_manager (burn resources)
         ──CPI──> item_nft (mint NFT)
marketplace ──CPI──> magic_token (mint MagicTokens)
            ──CPI──> Token-2022 (burn NFT)
```

### Programs

- **resource_manager** — Creates 6 SPL Token-2022 resource mints (WOOD, IRON, GOLD, LEATHER, STONE, DIAMOND). Mint authority is a PDA (`MintAuthority`), ensuring only this program can mint/burn resources.
- **magic_token** — MagicToken (SPL Token-2022) used as marketplace currency. Mint authority is a PDA, minting only possible via CPI from marketplace.
- **search** — Players search for 3 random resources every 60 seconds. On-chain cooldown via PDA timestamp. Mints resources via CPI to resource_manager.
- **item_nft** — Creates NFT items (supply=1, decimals=0) with on-chain metadata (ItemMetadata PDA).
- **crafting** — Burns resources according to recipes and mints NFT items via CPI.
- **marketplace** — List/cancel/buy items. Buying burns the NFT and mints MagicTokens to the seller via CPI.

## Resources (SPL Token-2022, decimals=0)

| ID | Symbol | Name |
|----|--------|------|
| 0 | WOOD | Wood |
| 1 | IRON | Iron |
| 2 | GOLD | Gold |
| 3 | LEATHER | Leather |
| 4 | STONE | Stone |
| 5 | DIAMOND | Diamond |

## Crafting Recipes

| Item | WOOD | IRON | GOLD | LEATHER | STONE | DIAMOND |
|------|------|------|------|---------|-------|---------|
| Cossack Saber | 1 | 3 | - | 1 | - | - |
| Elder Staff | 2 | - | 1 | - | - | 1 |
| Kharakternik Armor | - | 2 | 1 | 4 | - | - |
| Battle Bracelet | - | 4 | 2 | - | - | 2 |

## Security

- **PDA-controlled minting**: All mints use PDA authorities — no direct minting possible
- **On-chain cooldown**: 60-second timer enforced via PDA timestamp
- **Admin verification**: `has_one` constraints ensure only admin manages resources
- **CPI-only operations**: MagicToken minting only through marketplace CPI
- **Owner checks**: Player and item ownership verified on every operation

## Prerequisites

- Rust (rustup)
- Solana CLI
- Anchor CLI (0.32.1)
- Node.js 20+ and Yarn

## Setup & Run
```bash
git clone <repo-url>
cd cossack_business
yarn install
anchor build
anchor test    # runs 16 integration tests on localnet
```

## Deploy to Devnet
```bash
solana config set --url devnet
anchor deploy
```

## Test Coverage (16 tests)

- **Resource Manager**: init game config + mint authority, create 6 resource mints, create ATAs, invalid index, unauthorized admin
- **Magic Token**: init with mint creation
- **Item NFT**: init config
- **Search**: register player, search + mint 3 resources via CPI, cooldown enforcement, unregistered player
- **Crafting**: full craft cycle — burn resources + mint NFT via CPI
- **Marketplace**: list item, cancel listing, buy item (burn NFT + mint MagicTokens via CPI), zero price validation

## Game Flow

1. Admin calls `resource_manager::initialize_game` + `create_resource_mint` x6
2. Admin calls `magic_token::initialize` + `item_nft::initialize`
3. Player calls `search::register_player`
4. Player calls `search::search_resources` → receives 3 random resources (60s cooldown)
5. Player calls `crafting::craft_item` → burns resources, receives NFT
6. Player calls `marketplace::list_item` → lists NFT for MagicTokens
7. Buyer calls `marketplace::buy_item` → NFT burned, seller gets MagicTokens

## Project Structure
```
cossack_business/
├── programs/
│   ├── resource_manager/   # Token-2022 resource management + MintAuthority PDA
│   ├── magic_token/        # MagicToken mint + MagicMintAuthority PDA
│   ├── search/             # Resource search with on-chain cooldown
│   ├── item_nft/           # NFT creation + ItemMetadata PDA
│   ├── crafting/           # Recipe-based crafting via CPI
│   └── marketplace/        # Item trading via CPI
├── tests/
│   └── cossack_business.ts # 16 integration tests
├── Anchor.toml
└── README.md
```
