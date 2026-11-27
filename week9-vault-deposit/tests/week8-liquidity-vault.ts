import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Week8LiquidityVault } from "../target/types/week8_liquidity_vault";
import {
  Connection, Keypair, PublicKey, SystemProgram, LAMPORTS_PER_SOL,
  Transaction, sendAndConfirmTransaction,
} from "@solana/web3.js";
import {
  createMint, createInitializeAccountInstruction, mintTo, TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { assert } from "chai";

async function createTokenAccount(
  connection: Connection, payer: Keypair, mint: PublicKey, owner: PublicKey
): Promise<PublicKey> {
  const accountKeypair = Keypair.generate();
  const lamports = await connection.getMinimumBalanceForRentExemption(165);
  const tx = new Transaction().add(
    SystemProgram.createAccount({
      fromPubkey: payer.publicKey,
      newAccountPubkey: accountKeypair.publicKey,
      lamports, space: 165, programId: TOKEN_PROGRAM_ID,
    }),
    createInitializeAccountInstruction(accountKeypair.publicKey, mint, owner)
  );
  await sendAndConfirmTransaction(connection, tx, [payer, accountKeypair], { commitment: "confirmed" });
  return accountKeypair.publicKey;
}

describe("week8-liquidity-vault (Weeks 8-9)", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.Week8LiquidityVault as Program<Week8LiquidityVault>;
  const authority = (provider.wallet as anchor.Wallet).payer;

  // 🔑 СВЕЖИЙ user каждый запуск — user_position PDA не конфликтует
  const user = Keypair.generate();

  let tokenMint: PublicKey, shareMint: PublicKey;
  let vaultState: PublicKey, vaultStateBump: number;
  let vaultTokenAccount: PublicKey, userTokenAccount: PublicKey;
  let userShareAccount: PublicKey, userPosition: PublicKey;

  const ONE_TOKEN = 1_000_000;

  before(async () => {
    // Пополняем user SOL — он платит за init user_position
    const sig = await provider.connection.requestAirdrop(user.publicKey, 10 * LAMPORTS_PER_SOL);
    await provider.connection.confirmTransaction(sig);

    tokenMint = await createMint(provider.connection, authority, authority.publicKey, null, 6);
    [vaultState, vaultStateBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault_state"), tokenMint.toBuffer()], program.programId
    );
    shareMint = await createMint(provider.connection, authority, vaultState, null, 6);
    vaultTokenAccount = await createTokenAccount(provider.connection, authority, tokenMint, vaultState);
    userTokenAccount = await createTokenAccount(provider.connection, authority, tokenMint, user.publicKey);
    userShareAccount = await createTokenAccount(provider.connection, authority, shareMint, user.publicKey);
    await mintTo(provider.connection, authority, tokenMint, userTokenAccount, authority, 1000 * ONE_TOKEN);
    [userPosition] = PublicKey.findProgramAddressSync(
      [Buffer.from("user_position"), user.publicKey.toBuffer()], program.programId
    );

    console.log("\n=== Addresses ===");
    console.log("User:              ", user.publicKey.toBase58());
    console.log("Vault State PDA:   ", vaultState.toBase58());
    console.log("Vault Token Acct:  ", vaultTokenAccount.toBase58());
    console.log("User Token Acct:   ", userTokenAccount.toBase58());
    console.log("User Share Acct:   ", userShareAccount.toBase58());
    console.log("User Position PDA: ", userPosition.toBase58());
    console.log("=================\n");
  });

  it("Initializes the vault", async () => {
    await program.methods.initializeVault(vaultStateBump).accounts({
      vaultState, tokenMint, shareMint,
      authority: authority.publicKey,
      systemProgram: SystemProgram.programId,
      tokenProgram: TOKEN_PROGRAM_ID,
    }).rpc();
    const s = await program.account.vaultState.fetch(vaultState);
    assert.equal(s.totalTokens.toNumber(), 0);
    assert.equal(s.totalShares.toNumber(), 0);
    console.log("✅ Vault initialized");
  });

  it("Opens a user position", async () => {
    await program.methods.openPosition().accounts({
      userPosition, user: user.publicKey,
      systemProgram: SystemProgram.programId,
    }).signers([user]).rpc();
    const p = await program.account.userPosition.fetch(userPosition);
    assert.equal(p.owner.toBase58(), user.publicKey.toBase58());
    assert.equal(p.shares.toNumber(), 0);
    console.log("✅ User position opened");
  });

  it("First deposit: 1:1 (100 tokens → 100 shares)", async () => {
    const amount = 100 * ONE_TOKEN;
    await program.methods.deposit(new anchor.BN(amount)).accounts({
      vaultState, tokenMint, userTokenAccount, vaultTokenAccount,
      shareMint, userShareAccount, userPosition,
      user: user.publicKey, tokenProgram: TOKEN_PROGRAM_ID,
    }).signers([user]).rpc();

    const state = await program.account.vaultState.fetch(vaultState);
    assert.equal(state.totalTokens.toNumber(), amount);
    assert.equal(state.totalShares.toNumber(), amount);
    console.log("✅ First deposit: 100 tokens → 100 shares (1:1)");
  });

  it("Second deposit: still 1:1 (no earnings yet)", async () => {
    const amount = 50 * ONE_TOKEN;
    await program.methods.deposit(new anchor.BN(amount)).accounts({
      vaultState, tokenMint, userTokenAccount, vaultTokenAccount,
      shareMint, userShareAccount, userPosition,
      user: user.publicKey, tokenProgram: TOKEN_PROGRAM_ID,
    }).signers([user]).rpc();

    const state = await program.account.vaultState.fetch(vaultState);
    assert.equal(state.totalTokens.toNumber(), 150 * ONE_TOKEN);
    assert.equal(state.totalShares.toNumber(), 150 * ONE_TOKEN);
    console.log("✅ Second deposit: 50 tokens → 50 shares");
  });

  it("Rejects zero-amount deposit", async () => {
    try {
      await program.methods.deposit(new anchor.BN(0)).accounts({
        vaultState, tokenMint, userTokenAccount, vaultTokenAccount,
        shareMint, userShareAccount, userPosition,
        user: user.publicKey, tokenProgram: TOKEN_PROGRAM_ID,
      }).signers([user]).rpc();
      assert.fail("Expected ZeroAmount");
    } catch (err: any) {
      assert.include(err.toString(), "ZeroAmount");
      console.log("✅ Zero-amount rejected");
      console.log("\n🎉🎉🎉 WEEK 9 PASSED! 🎉🎉🎉\n");
    }
  });
});
