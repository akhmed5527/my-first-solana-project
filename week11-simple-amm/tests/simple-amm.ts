import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { SimpleAmm } from "../target/types/simple_amm";
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

describe("simple-amm (Week 11)", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.SimpleAmm as Program<SimpleAmm>;
  const authority = (provider.wallet as anchor.Wallet).payer;
  const user = Keypair.generate();

  let tokenA: PublicKey, tokenB: PublicKey;
  let pool: PublicKey, poolBump: number;
  let lpMint: PublicKey, vaultA: PublicKey, vaultB: PublicKey;
  let userA: PublicKey, userB: PublicKey;
  let userLp: PublicKey;   // ⚠️ создадим после initialize_pool

  const ONE = 1_000_000;

  before(async () => {
    const sig = await provider.connection.requestAirdrop(user.publicKey, 10 * LAMPORTS_PER_SOL);
    await provider.connection.confirmTransaction(sig);

    tokenA = await createMint(provider.connection, authority, authority.publicKey, null, 6);
    tokenB = await createMint(provider.connection, authority, authority.publicKey, null, 6);

    [pool, poolBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("pool"), tokenA.toBuffer(), tokenB.toBuffer()],
      program.programId
    );
    [lpMint] = PublicKey.findProgramAddressSync(
      [Buffer.from("lp_mint"), pool.toBuffer()], program.programId
    );
    [vaultA] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault_a"), pool.toBuffer()], program.programId
    );
    [vaultB] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault_b"), pool.toBuffer()], program.programId
    );

    // Создаём userA и userB (mints уже существуют — ок)
    userA = await createTokenAccount(provider.connection, authority, tokenA, user.publicKey);
    userB = await createTokenAccount(provider.connection, authority, tokenB, user.publicKey);

    await mintTo(provider.connection, authority, tokenA, userA, authority, 1000 * ONE);
    await mintTo(provider.connection, authority, tokenB, userB, authority, 1000 * ONE);

    console.log("\n=== Addresses ===");
    console.log("Token A:   ", tokenA.toBase58());
    console.log("Token B:   ", tokenB.toBase58());
    console.log("Pool PDA:  ", pool.toBase58());
    console.log("LP Mint:   ", lpMint.toBase58());
    console.log("Vault A:   ", vaultA.toBase58());
    console.log("Vault B:   ", vaultB.toBase58());
    console.log("=================\n");
  });

  it("Initializes the pool", async () => {
    await program.methods.initializePool(30, poolBump).accounts({
      pool, tokenAMint: tokenA, tokenBMint: tokenB,
      lpMint, vaultA, vaultB,
      authority: authority.publicKey,
      systemProgram: SystemProgram.programId,
      tokenProgram: TOKEN_PROGRAM_ID,
      rent: anchor.web3.SYSVAR_RENT_PUBKEY,
    }).rpc();

    const p = await program.account.pool.fetch(pool);
    assert.equal(p.reserveA.toNumber(), 0);
    assert.equal(p.reserveB.toNumber(), 0);
    assert.equal(p.totalLp.toNumber(), 0);
    assert.equal(p.feeBps, 30);
    console.log("✅ Pool initialized (fee = 30 bps)");

    // 🔑 Создаём userLp ТОЛЬКО СЕЙЧАС, когда lpMint уже существует
    userLp = await createTokenAccount(provider.connection, authority, lpMint, user.publicKey);
    console.log("✅ User LP account created:", userLp.toBase58());
  });

  it("First add_liquidity: 100A + 100B → 100 LP (1:1)", async () => {
    await program.methods.addLiquidity(
      new anchor.BN(100 * ONE), new anchor.BN(100 * ONE), new anchor.BN(0)
    ).accounts({
      pool, tokenAMint: tokenA, tokenBMint: tokenB,
      vaultA, vaultB, lpMint,
      userA, userB, userLp,
      user: user.publicKey,
      tokenProgram: TOKEN_PROGRAM_ID,
    }).signers([user]).rpc();

    const p = await program.account.pool.fetch(pool);
    assert.equal(p.reserveA.toNumber(), 100 * ONE);
    assert.equal(p.reserveB.toNumber(), 100 * ONE);
    assert.equal(p.totalLp.toNumber(), 100 * ONE);
    console.log("✅ First add_liquidity: 100A + 100B → 100 LP");
  });

  it("Second add_liquidity: 50A + 50B → 50 LP (still 1:1)", async () => {
    await program.methods.addLiquidity(
      new anchor.BN(50 * ONE), new anchor.BN(50 * ONE), new anchor.BN(0)
    ).accounts({
      pool, tokenAMint: tokenA, tokenBMint: tokenB,
      vaultA, vaultB, lpMint,
      userA, userB, userLp,
      user: user.publicKey,
      tokenProgram: TOKEN_PROGRAM_ID,
    }).signers([user]).rpc();

    const p = await program.account.pool.fetch(pool);
    assert.equal(p.reserveA.toNumber(), 150 * ONE);
    assert.equal(p.reserveB.toNumber(), 150 * ONE);
    assert.equal(p.totalLp.toNumber(), 150 * ONE);
    console.log("✅ Second add_liquidity: 50A + 50B → 50 LP");
  });

  it("Swap 10 A → B (price impact + 0.3% fee)", async () => {
    const userBBefore = await provider.connection.getTokenAccountBalance(userB);

    await program.methods.swap(
      new anchor.BN(10 * ONE),
      new anchor.BN(0),
      true,
    ).accounts({
      pool, tokenAMint: tokenA, tokenBMint: tokenB,
      vaultA, vaultB,
      userA, userB,
      user: user.publicKey,
      tokenProgram: TOKEN_PROGRAM_ID,
    }).signers([user]).rpc();

    const p = await program.account.pool.fetch(pool);
    const userBAfter = await provider.connection.getTokenAccountBalance(userB);
    const received = Number(userBAfter.value.amount) - Number(userBBefore.value.amount);

    assert.equal(p.reserveA.toNumber(), 160 * ONE);
    assert.isTrue(p.reserveB.toNumber() < 150 * ONE);
    assert.isTrue(received > 9 * ONE && received < 10 * ONE);
    console.log(`✅ Swap 10A → ${(received / ONE).toFixed(3)}B`);
  });

  it("Swap 10 B → A (reverse direction)", async () => {
    const userABefore = await provider.connection.getTokenAccountBalance(userA);

    await program.methods.swap(
      new anchor.BN(10 * ONE),
      new anchor.BN(0),
      false,
    ).accounts({
      pool, tokenAMint: tokenA, tokenBMint: tokenB,
      vaultA, vaultB,
      userA, userB,
      user: user.publicKey,
      tokenProgram: TOKEN_PROGRAM_ID,
    }).signers([user]).rpc();

    const userAAfter = await provider.connection.getTokenAccountBalance(userA);
    const received = Number(userAAfter.value.amount) - Number(userABefore.value.amount);
    assert.isTrue(received > 0);
    console.log(`✅ Swap 10B → ${(received / ONE).toFixed(3)}A`);
  });

  it("Remove 30 LP → proportional A + B back", async () => {
    const aBefore = await provider.connection.getTokenAccountBalance(userA);
    const bBefore = await provider.connection.getTokenAccountBalance(userB);

    await program.methods.removeLiquidity(new anchor.BN(30 * ONE)).accounts({
      pool, tokenAMint: tokenA, tokenBMint: tokenB,
      vaultA, vaultB, lpMint,
      userA, userB, userLp,
      user: user.publicKey,
      tokenProgram: TOKEN_PROGRAM_ID,
    }).signers([user]).rpc();

    const aAfter = await provider.connection.getTokenAccountBalance(userA);
    const bAfter = await provider.connection.getTokenAccountBalance(userB);
    const aOut = Number(aAfter.value.amount) - Number(aBefore.value.amount);
    const bOut = Number(bAfter.value.amount) - Number(bBefore.value.amount);

    assert.isTrue(aOut > 0 && bOut > 0);
    console.log(`✅ Remove 30 LP → ${(aOut / ONE).toFixed(3)}A + ${(bOut / ONE).toFixed(3)}B`);

    console.log("\n🎉🎉🎉 WEEK 11 PASSED! 🎉🎉🎉\n");
  });
});
