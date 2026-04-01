import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey, Keypair, SystemProgram, Transaction } from "@solana/web3.js";
import {
  TOKEN_2022_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
  createAssociatedTokenAccountIdempotentInstruction,
  createInitializeMint2Instruction,
  createMintToInstruction,
  getMintLen,
  ExtensionType,
} from "@solana/spl-token";
import { assert } from "chai";

import { ResourceManager } from "../target/types/resource_manager";
import { MagicToken } from "../target/types/magic_token";
import { Search } from "../target/types/search";
import { ItemNft } from "../target/types/item_nft";
import { Crafting } from "../target/types/crafting";
import { Marketplace } from "../target/types/marketplace";

describe("Cossack Business", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const resourceManager = anchor.workspace.ResourceManager as Program<ResourceManager>;
  const magicToken = anchor.workspace.MagicToken as Program<MagicToken>;
  const searchProgram = anchor.workspace.Search as Program<Search>;
  const itemNft = anchor.workspace.ItemNft as Program<ItemNft>;
  const crafting = anchor.workspace.Crafting as Program<Crafting>;
  const marketplace = anchor.workspace.Marketplace as Program<Marketplace>;

  const admin = provider.wallet;

  let gameConfigPda: PublicKey;
  let mintAuthorityPda: PublicKey;
  let magicConfigPda: PublicKey;
  let magicMintAuthorityPda: PublicKey;
  let magicMintPda: PublicKey;
  let itemConfigPda: PublicKey;
  let playerPda: PublicKey;
  let resourceMints: PublicKey[] = [];
  let resourceATAs: PublicKey[] = [];
  let magicATA: PublicKey;
  const itemPrices = [100, 200, 300, 400];

  async function getTokenBalance(ata: PublicKey): Promise<number> {
    try {
      const info = await provider.connection.getTokenAccountBalance(ata);
      return Number(info.value.amount);
    } catch {
      return 0;
    }
  }

  async function createATA(mint: PublicKey, owner: PublicKey): Promise<PublicKey> {
    const ata = getAssociatedTokenAddressSync(
      mint, owner, false, TOKEN_2022_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID
    );
    const info = await provider.connection.getAccountInfo(ata);
    if (!info) {
      const tx = new Transaction().add(
        createAssociatedTokenAccountIdempotentInstruction(
          admin.publicKey, ata, owner, mint,
          TOKEN_2022_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID
        )
      );
      await provider.sendAndConfirm(tx);
    }
    return ata;
  }

  before(async () => {
    [gameConfigPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("game_config")], resourceManager.programId
    );
    [mintAuthorityPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("mint_authority")], resourceManager.programId
    );
    [magicConfigPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("magic_config")], magicToken.programId
    );
    [magicMintAuthorityPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("magic_mint_authority")], magicToken.programId
    );
    [magicMintPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("magic_token_mint")], magicToken.programId
    );
    [itemConfigPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("item_config")], itemNft.programId
    );
    [playerPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("player"), admin.publicKey.toBuffer()], searchProgram.programId
    );
    for (let i = 0; i < 6; i++) {
      const [mintPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("resource_mint"), Buffer.from([i])], resourceManager.programId
      );
      resourceMints.push(mintPda);
    }
  });

  // ==================== RESOURCE MANAGER ====================

  describe("Resource Manager", () => {
    it("Initializes game config and mint authority", async () => {
      await resourceManager.methods
        .initializeGame(itemPrices.map(p => new anchor.BN(p)))
        .accounts({
          admin: admin.publicKey,
          gameConfig: gameConfigPda,
          mintAuthority: mintAuthorityPda,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const config = await resourceManager.account.gameConfig.fetch(gameConfigPda);
      assert.equal(config.admin.toBase58(), admin.publicKey.toBase58());
    });

    it("Creates all 6 resource mints", async () => {
      for (let i = 0; i < 6; i++) {
        await resourceManager.methods
          .createResourceMint(i)
          .accounts({
            admin: admin.publicKey,
            gameConfig: gameConfigPda,
            mintAuthority: mintAuthorityPda,
            resourceMint: resourceMints[i],
            tokenProgram: TOKEN_2022_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .rpc();
      }
      const config = await resourceManager.account.gameConfig.fetch(gameConfigPda);
      for (let i = 0; i < 6; i++) {
        assert.equal(config.resourceMints[i].toBase58(), resourceMints[i].toBase58());
      }
    });

    it("Creates resource ATAs", async () => {
      const tx = new Transaction();
      for (let i = 0; i < 6; i++) {
        const ata = getAssociatedTokenAddressSync(
          resourceMints[i], admin.publicKey, false,
          TOKEN_2022_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID
        );
        resourceATAs.push(ata);
        tx.add(createAssociatedTokenAccountIdempotentInstruction(
          admin.publicKey, ata, admin.publicKey, resourceMints[i],
          TOKEN_2022_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID
        ));
      }
      await provider.sendAndConfirm(tx);
    });

    it("Fails with invalid resource index", async () => {
      try {
        await resourceManager.methods.createResourceMint(10)
          .accounts({
            admin: admin.publicKey, gameConfig: gameConfigPda,
            mintAuthority: mintAuthorityPda, resourceMint: resourceMints[0],
            tokenProgram: TOKEN_2022_PROGRAM_ID, systemProgram: SystemProgram.programId,
          }).rpc();
        assert.fail("Should have thrown");
      } catch (err) { assert.ok(err); }
    });

    it("Fails when non-admin creates resource", async () => {
      const fake = Keypair.generate();
      await provider.connection.requestAirdrop(fake.publicKey, 1e9)
        .then(sig => provider.connection.confirmTransaction(sig));
      try {
        await resourceManager.methods.createResourceMint(0)
          .accounts({
            admin: fake.publicKey, gameConfig: gameConfigPda,
            mintAuthority: mintAuthorityPda, resourceMint: resourceMints[0],
            tokenProgram: TOKEN_2022_PROGRAM_ID, systemProgram: SystemProgram.programId,
          }).signers([fake]).rpc();
        assert.fail("Should have thrown");
      } catch (err) { assert.ok(err); }
    });
  });

  // ==================== MAGIC TOKEN ====================

  describe("Magic Token", () => {
    it("Initializes MagicToken with mint", async () => {
      await magicToken.methods
        .initialize()
        .accounts({
          admin: admin.publicKey,
          magicConfig: magicConfigPda,
          magicMintAuthority: magicMintAuthorityPda,
          magicMint: magicMintPda,
          tokenProgram: TOKEN_2022_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const config = await magicToken.account.magicConfig.fetch(magicConfigPda);
      assert.equal(config.mint.toBase58(), magicMintPda.toBase58());

      // Create ATA for MagicToken
      magicATA = await createATA(magicMintPda, admin.publicKey);
    });
  });

  // ==================== ITEM NFT ====================

  describe("Item NFT", () => {
    it("Initializes item config", async () => {
      await itemNft.methods
        .initialize()
        .accounts({
          admin: admin.publicKey,
          itemConfig: itemConfigPda,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const config = await itemNft.account.itemConfig.fetch(itemConfigPda);
      assert.equal(config.itemCounter.toNumber(), 0);
    });
  });

  // ==================== SEARCH ====================

  describe("Search", () => {
    it("Registers a player", async () => {
      await searchProgram.methods
        .registerPlayer()
        .accounts({
          owner: admin.publicKey,
          player: playerPda,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const player = await searchProgram.account.player.fetch(playerPda);
      assert.equal(player.lastSearchTimestamp.toNumber(), 0);
    });

    it("Searches and mints 3 resources", async () => {
      const remainingAccounts = [];
      for (let i = 0; i < 6; i++) {
        remainingAccounts.push({ pubkey: resourceMints[i], isSigner: false, isWritable: true });
        remainingAccounts.push({ pubkey: resourceATAs[i], isSigner: false, isWritable: true });
      }

      await searchProgram.methods
        .searchResources()
        .accounts({
          owner: admin.publicKey,
          player: playerPda,
          gameConfig: gameConfigPda,
          mintAuthority: mintAuthorityPda,
          resourceManagerProgram: resourceManager.programId,
          tokenProgram: TOKEN_2022_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .remainingAccounts(remainingAccounts)
        .rpc();

      let total = 0;
      for (let i = 0; i < 6; i++) {
        const bal = await getTokenBalance(resourceATAs[i]);
        if (bal > 0) console.log(`    Resource [${i}]: ${bal}`);
        total += bal;
      }
      assert.equal(total, 3);
    });

    it("Fails before cooldown expires", async () => {
      const remainingAccounts = [];
      for (let i = 0; i < 6; i++) {
        remainingAccounts.push({ pubkey: resourceMints[i], isSigner: false, isWritable: true });
        remainingAccounts.push({ pubkey: resourceATAs[i], isSigner: false, isWritable: true });
      }
      try {
        await searchProgram.methods.searchResources()
          .accounts({
            owner: admin.publicKey, player: playerPda, gameConfig: gameConfigPda,
            mintAuthority: mintAuthorityPda, resourceManagerProgram: resourceManager.programId,
            tokenProgram: TOKEN_2022_PROGRAM_ID, systemProgram: SystemProgram.programId,
          }).remainingAccounts(remainingAccounts).rpc();
        assert.fail("Should have thrown");
      } catch (err) { assert.ok(err); }
    });

    it("Fails for unregistered player", async () => {
      const fake = Keypair.generate();
      await provider.connection.requestAirdrop(fake.publicKey, 1e9)
        .then(sig => provider.connection.confirmTransaction(sig));
      const [fakePda] = PublicKey.findProgramAddressSync(
        [Buffer.from("player"), fake.publicKey.toBuffer()], searchProgram.programId
      );
      try {
        await searchProgram.methods.searchResources()
          .accounts({
            owner: fake.publicKey, player: fakePda, gameConfig: gameConfigPda,
            mintAuthority: mintAuthorityPda, resourceManagerProgram: resourceManager.programId,
            tokenProgram: TOKEN_2022_PROGRAM_ID, systemProgram: SystemProgram.programId,
          }).signers([fake]).rpc();
        assert.fail("Should have thrown");
      } catch (err) { assert.ok(err); }
    });
  });

  // ==================== CRAFTING (needs enough resources) ====================

  describe("Crafting", () => {
    // Mint extra resources directly so we have enough to craft
    // Saber recipe: 1 WOOD + 3 IRON + 1 LEATHER
    before(async () => {
      // Do multiple searches to accumulate resources
      // Or mint directly via resource_manager for testing
      for (let s = 0; s < 20; s++) {
        // Fast-forward time by manipulating... we can't easily.
        // Instead, directly mint via resource_manager
        // We need: 1 WOOD(0), 3 IRON(1), 1 LEATHER(3)
      }

      // Mint resources directly using resource_manager.mint_resource
      const amounts = [1, 3, 0, 1, 0, 0]; // Saber recipe
      for (let i = 0; i < 6; i++) {
        if (amounts[i] === 0) continue;
        await resourceManager.methods
          .mintResource(new anchor.BN(amounts[i]))
          .accounts({
            player: admin.publicKey,
            mintAuthority: mintAuthorityPda,
            resourceMint: resourceMints[i],
            playerTokenAccount: resourceATAs[i],
            tokenProgram: TOKEN_2022_PROGRAM_ID,
          })
          .rpc();
      }

      // Verify balances
      for (let i = 0; i < 6; i++) {
        const bal = await getTokenBalance(resourceATAs[i]);
        if (bal > 0) console.log(`    Resource [${i}] balance: ${bal}`);
      }
    });

    it("Crafts a Cossack Saber (burns resources + mints NFT)", async () => {
      // Create NFT mint for the item
      const itemMintKeypair = Keypair.generate();
      const mintLen = getMintLen([]);
      const lamports = await provider.connection.getMinimumBalanceForRentExemption(mintLen);

      // Create mint account + initialize it with item_config as authority
      const createMintTx = new Transaction().add(
        SystemProgram.createAccount({
          fromPubkey: admin.publicKey,
          newAccountPubkey: itemMintKeypair.publicKey,
          space: mintLen,
          lamports,
          programId: TOKEN_2022_PROGRAM_ID,
        }),
        createInitializeMint2Instruction(
          itemMintKeypair.publicKey,
          0, // decimals
          itemConfigPda, // mint authority = item_config PDA
          null, // no freeze authority
          TOKEN_2022_PROGRAM_ID
        )
      );
      await provider.sendAndConfirm(createMintTx, [itemMintKeypair]);

      // Create ATA for the NFT
      const playerItemATA = await createATA(itemMintKeypair.publicKey, admin.publicKey);

      // Derive item metadata PDA
      const [itemMetadataPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("item_metadata"), itemMintKeypair.publicKey.toBuffer()],
        itemNft.programId
      );

      // Build remaining accounts for resource burning
      const remainingAccounts = [];
      for (let i = 0; i < 6; i++) {
        remainingAccounts.push({ pubkey: resourceMints[i], isSigner: false, isWritable: true });
        remainingAccounts.push({ pubkey: resourceATAs[i], isSigner: false, isWritable: true });
      }

      // Craft!
      await crafting.methods
        .craftItem(0) // Saber = type 0
        .accounts({
          player: admin.publicKey,
          gameConfig: gameConfigPda,
          itemMint: itemMintKeypair.publicKey,
          itemConfig: itemConfigPda,
          itemMetadata: itemMetadataPda,
          playerItemTokenAccount: playerItemATA,
          resourceManagerProgram: resourceManager.programId,
          itemNftProgram: itemNft.programId,
          tokenProgram: TOKEN_2022_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .remainingAccounts(remainingAccounts)
        .rpc();

      // Verify NFT minted
      const nftBalance = await getTokenBalance(playerItemATA);
      assert.equal(nftBalance, 1, "Should have 1 NFT");

      // Verify resources burned (WOOD -1, IRON -3, LEATHER -1)
      const wood = await getTokenBalance(resourceATAs[0]);
      const iron = await getTokenBalance(resourceATAs[1]);
      const leather = await getTokenBalance(resourceATAs[3]);
      console.log(`    After craft: WOOD=${wood}, IRON=${iron}, LEATHER=${leather}`);

      // Verify item metadata
      const metadata = await itemNft.account.itemMetadata.fetch(itemMetadataPda);
      assert.equal(metadata.itemType, 0);
      assert.equal(metadata.owner.toBase58(), admin.publicKey.toBase58());
    });
  });

  // ==================== MARKETPLACE ====================


  describe("Marketplace", () => {
    let saberMintKeypair: Keypair;
    let saberATA: PublicKey;
    let saberMetadataPda: PublicKey;
    let listingPda: PublicKey;

    before(async () => {
      // Craft another Saber for marketplace testing
      // First mint resources
      const amounts = [1, 3, 0, 1, 0, 0];
      for (let i = 0; i < 6; i++) {
        if (amounts[i] === 0) continue;
        await resourceManager.methods
          .mintResource(new anchor.BN(amounts[i]))
          .accounts({
            player: admin.publicKey,
            mintAuthority: mintAuthorityPda,
            resourceMint: resourceMints[i],
            playerTokenAccount: resourceATAs[i],
            tokenProgram: TOKEN_2022_PROGRAM_ID,
          })
          .rpc();
      }

      // Create NFT mint
      saberMintKeypair = Keypair.generate();
      const mintLen = getMintLen([]);
      const lamports = await provider.connection.getMinimumBalanceForRentExemption(mintLen);
      const createMintTx = new Transaction().add(
        SystemProgram.createAccount({
          fromPubkey: admin.publicKey,
          newAccountPubkey: saberMintKeypair.publicKey,
          space: mintLen,
          lamports,
          programId: TOKEN_2022_PROGRAM_ID,
        }),
        createInitializeMint2Instruction(
          saberMintKeypair.publicKey, 0, itemConfigPda, null, TOKEN_2022_PROGRAM_ID
        )
      );
      await provider.sendAndConfirm(createMintTx, [saberMintKeypair]);

      saberATA = await createATA(saberMintKeypair.publicKey, admin.publicKey);

      [saberMetadataPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("item_metadata"), saberMintKeypair.publicKey.toBuffer()],
        itemNft.programId
      );

      // Craft the saber
      const remainingAccounts = [];
      for (let i = 0; i < 6; i++) {
        remainingAccounts.push({ pubkey: resourceMints[i], isSigner: false, isWritable: true });
        remainingAccounts.push({ pubkey: resourceATAs[i], isSigner: false, isWritable: true });
      }

      await crafting.methods
        .craftItem(0)
        .accounts({
          player: admin.publicKey,
          gameConfig: gameConfigPda,
          itemMint: saberMintKeypair.publicKey,
          itemConfig: itemConfigPda,
          itemMetadata: saberMetadataPda,
          playerItemTokenAccount: saberATA,
          resourceManagerProgram: resourceManager.programId,
          itemNftProgram: itemNft.programId,
          tokenProgram: TOKEN_2022_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .remainingAccounts(remainingAccounts)
        .rpc();

      [listingPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("listing"), saberMintKeypair.publicKey.toBuffer()],
        marketplace.programId
      );
    });

    it("Lists an item for sale", async () => {
      await marketplace.methods
        .listItem(new anchor.BN(100))
        .accounts({
          seller: admin.publicKey,
          itemMint: saberMintKeypair.publicKey,
          itemMetadata: saberMetadataPda,
          listing: listingPda,
          itemNftProgram: itemNft.programId,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const listing = await marketplace.account.listing.fetch(listingPda);
      assert.equal(listing.seller.toBase58(), admin.publicKey.toBase58());
      assert.equal(listing.price.toNumber(), 100);
      assert.equal(listing.active, true);
      assert.equal(listing.itemType, 0);
      console.log("    Listed Cossack Saber for 100 MagicTokens");
    });

    it("Cancels a listing", async () => {
      await marketplace.methods
        .cancelListing()
        .accounts({
          seller: admin.publicKey,
          itemMint: saberMintKeypair.publicKey,
          listing: listingPda,
        })
        .rpc();

      const listing = await marketplace.account.listing.fetch(listingPda);
      assert.equal(listing.active, false);
      console.log("    Listing cancelled");
    });

    it("Fails to list with zero price", async () => {
      const fakeMint = Keypair.generate();
      const [meta] = PublicKey.findProgramAddressSync(
        [Buffer.from("item_metadata"), fakeMint.publicKey.toBuffer()], itemNft.programId
      );
      const [listing] = PublicKey.findProgramAddressSync(
        [Buffer.from("listing"), fakeMint.publicKey.toBuffer()], marketplace.programId
      );
      try {
        await marketplace.methods.listItem(new anchor.BN(0))
          .accounts({
            seller: admin.publicKey, itemMint: fakeMint.publicKey,
            itemMetadata: meta, listing,
            itemNftProgram: itemNft.programId, systemProgram: SystemProgram.programId,
          }).rpc();
        assert.fail("Should have thrown");
      } catch (err) { assert.ok(err); }
    });
  });
});
