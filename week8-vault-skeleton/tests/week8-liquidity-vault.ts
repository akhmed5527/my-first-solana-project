import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Week8LiquidityVault } from "../target/types/week8_liquidity_vault";
import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import {
  createMint,
  createInitializeAccountInstruction,
  mintTo,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { assert } from "chai";

// Ручное создание token-аккаунта (работает и для PDA-owner)
async function createTokenAccount(
  connection: Connection,
  payer: Keypair,
  mint: PublicKey,
  owner: PublicKey
): Promise<PublicKey> {
  const accountKeypair = Keypair.generate();
  const lamports = await connection.getMinimumBalanceForRentExemption(165);
  const tx = new Transaction().add(
    SystemProgram.createAccount({
      fromPubkey: payer.publicKey,
      newAccountPubkey: accountKeypair.publicKey,
      lamports,
      space: 165,
      programId: TOKEN_PROGRAM_ID,
    }),
    createInitializeAccountInstruction(
      accountKeypair.publicKey,
      mint,
      owner
    )
  );
  await sendAndConfirmTransaction(
    connection,
    tx,
    [payer, accountKeypair],
    { commitment: "confirmed" }
  );
  return accountKeypair.publicKey;
}

describe("week8-liquidity-vault (Week 8: Skeleton)", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace
    .Week8LiquidityVault as Program<Week8LiquidityVault>;
  const authority = (provider.wallet as anchor.Wallet).payer;

  let tokenMint: PublicKey;
  let shareMint: PublicKey;
  let vaultState: PublicKey;
  let vaultStateBump: number;
  let vaultTokenAccount: PublicKey;
  let userTokenAccount: PublicKey;
  let userShareAccount: PublicKey;
  let userPosition: PublicKey;

  before(async () => {
    // 1) token mint (authority — наш кошелёк)
    tokenMint = await createMint(
      provider.connection, authority, authority.publicKey, null, 6
    );

    // 2) vault_state PDA
    [vaultState, vaultStateBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault_state"), tokenMint.toBuffer()],
      program.programId
    );

    // 3) share_mint — mint authority = vault_state PDA
    shareMint = await createMint(
      provider.connection, authority, vaultState, null, 6
    );

    // 4) vault_token_account — owner = vault_state (PDA, off-curve)
    vaultTokenAccount = await createTokenAccount(
      provider.connection, authority, tokenMint, vaultState
    );

    // 5) user accounts
    userTokenAccount = await createTokenAccount(
      provider.connection, authority, tokenMint, authority.publicKey
    );
    userShareAccount = await createTokenAccount(
      provider.connection, authority, shareMint, authority.publicKey
    );

    // 6) 1000 токенов пользователю (пригодится в неделе 9)
    await mintTo(
      provider.connection, authority, tokenMint, userTokenAccount,
      authority, 1000_000_000
    );

    // 7) user_position PDA
    [userPosition] = PublicKey.findProgramAddressSync(
      [Buffer.from("user_position"), authority.publicKey.toBuffer()],
      program.programId
    );

    console.log("\n=== Set up addresses ===");
    console.log("Program ID:         ", program.programId.toBase58());
    console.log("Token Mint:         ", tokenMint.toBase58());
    console.log("Share Mint:         ", shareMint.toBase58());
    console.log("Vault State PDA:    ", vaultState.toBase58());
    console.log("Vault Token Account:", vaultTokenAccount.toBase58());
    console.log("User Token Account: ", userTokenAccount.toBase58());
    console.log("User Share Account: ", userShareAccount.toBase58());
    console.log("User Position PDA:  ", userPosition.toBase58());
    console.log("=======================\n");
  });

  it("Initializes the vault", async () => {
    await program.methods
      .initializeVault(vaultStateBump)
      .accounts({
        vaultState,
        tokenMint,
        shareMint,
        authority: authority.publicKey,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();

    const state = await program.account.vaultState.fetch(vaultState);
    assert.equal(state.authority.toBase58(), authority.publicKey.toBase58());
    assert.equal(state.tokenMint.toBase58(), tokenMint.toBase58());
    assert.equal(state.shareMint.toBase58(), shareMint.toBase58());
    assert.equal(state.totalTokens.toNumber(), 0);
    assert.equal(state.totalShares.toNumber(), 0);
    console.log("✅ Vault initialized");
  });

  it("Opens a user position", async () => {
    await program.methods
      .openPosition()
      .accounts({
        userPosition,
        user: authority.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    const pos = await program.account.userPosition.fetch(userPosition);
    assert.equal(pos.owner.toBase58(), authority.publicKey.toBase58());
    assert.equal(pos.amount.toNumber(), 0);
    assert.equal(pos.shares.toNumber(), 0);
    console.log("✅ User position opened");
    console.log("\n🎉🎉🎉 WEEK 8 PASSED! 🎉🎉🎉\n");
  });
});
