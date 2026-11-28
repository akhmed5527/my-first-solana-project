import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Week8LiquidityVault } from "../target/types/week8_liquidity_vault";
import { SimpleAmm } from "../target/types/simple_amm";
import { Keypair, PublicKey, SystemProgram, SYSVAR_RENT_PUBKEY } from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID, createMint, createAccount, mintTo, getAccount,
  AccountLayout, createInitializeAccountInstruction,
} from "@solana/spl-token";
import { assert } from "chai";

async function createRawTokenAccount(
  connection: anchor.web3.Connection,
  payer: anchor.web3.Keypair,
  mint: PublicKey,
  owner: PublicKey
): Promise<PublicKey> {
  const keypair = anchor.web3.Keypair.generate();
  const lamports = await connection.getMinimumBalanceForRentExemption(AccountLayout.span);
  const tx = new anchor.web3.Transaction().add(
    anchor.web3.SystemProgram.createAccount({
      fromPubkey: payer.publicKey,
      newAccountPubkey: keypair.publicKey,
      space: AccountLayout.span,
      lamports,
      programId: TOKEN_PROGRAM_ID,
    }),
    createInitializeAccountInstruction(
      keypair.publicKey,
      mint,
      owner,
      TOKEN_PROGRAM_ID
    )
  );
  await anchor.web3.sendAndConfirmTransaction(connection, tx, [payer, keypair]);
  return keypair.publicKey;
}

describe("week12-vault-swap", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const vaultProgram = anchor.workspace.Week8LiquidityVault as Program<Week8LiquidityVault>;
  const ammProgram = anchor.workspace.SimpleAmm as Program<SimpleAmm>;

  const admin = (provider.wallet as anchor.Wallet).payer;
  const user = Keypair.generate();

  let mintA: PublicKey;
  let mintB: PublicKey;
  let shareMint: PublicKey;
  let vaultTokenA: PublicKey;
  let vaultTokenB: PublicKey;
  let vaultStatePda: PublicKey;
  let ammPoolPda: PublicKey;
  let ammLpMintPda: PublicKey;
  let ammVaultA: PublicKey;
  let ammVaultB: PublicKey;
  let ammPoolBump: number;

  before(async () => {
    const sig = await provider.connection.requestAirdrop(
      user.publicKey, 5 * anchor.web3.LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(sig);

    // === PDA ===
    [vaultStatePda] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault_state"), mintA?.toBuffer() || Buffer.alloc(32)],
      vaultProgram.programId
    );

    mintA = await createMint(provider.connection, admin, admin.publicKey, null, 6);
    mintB = await createMint(provider.connection, admin, admin.publicKey, null, 6);

    // vault_state PDA — пересчитать с реальным mintA
    [vaultStatePda] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault_state"), mintA.toBuffer()],
      vaultProgram.programId
    );

    shareMint = await createMint(
      provider.connection, admin,
      vaultStatePda,  // mint authority = vault_state PDA
      null, 6
    );

    // === AMM PDAs ===
    [ammPoolPda, ammPoolBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("pool"), mintA.toBuffer(), mintB.toBuffer()],
      ammProgram.programId
    );
    [ammLpMintPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("lp_mint"), ammPoolPda.toBuffer()],
      ammProgram.programId
    );
    [ammVaultA] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault_a"), ammPoolPda.toBuffer()],
      ammProgram.programId
    );
    [ammVaultB] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault_b"), ammPoolPda.toBuffer()],
      ammProgram.programId
    );

    // === AMM: initialize pool ===
    await ammProgram.methods
      .initializePool(30, ammPoolBump)
      .accounts({
        pool: ammPoolPda,
        tokenAMint: mintA,
        tokenBMint: mintB,
        lpMint: ammLpMintPda,
        vaultA: ammVaultA,
        vaultB: ammVaultB,
        authority: admin.publicKey,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .signers([admin])
      .rpc();

    // === AMM: add liquidity ===
    const adminA = await createAccount(provider.connection, admin, mintA, admin.publicKey);
    const adminB = await createAccount(provider.connection, admin, mintB, admin.publicKey);
    const adminLp = await createAccount(provider.connection, admin, ammLpMintPda, admin.publicKey);

    await mintTo(provider.connection, admin, mintA, adminA, admin, 1_000_000_000);
    await mintTo(provider.connection, admin, mintB, adminB, admin, 1_000_000_000);

    await ammProgram.methods
      .addLiquidity(
        new anchor.BN(500_000_000),
        new anchor.BN(500_000_000),
        new anchor.BN(0)
      )
      .accounts({
        pool: ammPoolPda,
        tokenAMint: mintA,
        tokenBMint: mintB,
        vaultA: ammVaultA,
        vaultB: ammVaultB,
        lpMint: ammLpMintPda,
        userA: adminA,
        userB: adminB,
        userLp: adminLp,
        user: admin.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([admin])
      .rpc();

    // === Vault: token accounts (authority = vault_state PDA) ===
      vaultTokenA = await createRawTokenAccount(provider.connection, admin, mintA, vaultStatePda);
      vaultTokenB = await createRawTokenAccount(provider.connection, admin, mintB, vaultStatePda);

    // === Vault: initialize ===
    const [expectedPda, expectedBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault_state"), mintA.toBuffer()],
      vaultProgram.programId
    );

    await vaultProgram.methods
      .initializeVault(
        expectedBump,
        mintB,
        vaultTokenA,
        vaultTokenB,
        ammPoolPda
      )
      .accounts({
        vaultState: expectedPda,
        tokenMint: mintA,
        shareMint: shareMint,
        authority: admin.publicKey,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([admin])
      .rpc();

    // === Vault: user position ===
    const [userPosPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("user_position"), user.publicKey.toBuffer()],
      vaultProgram.programId
    );

    await vaultProgram.methods
      .openPosition()
      .accounts({
        userPosition: userPosPda,
        user: user.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([user])
      .rpc();

    // === наливаем токенов в vault_token_a/b ===
    await mintTo(provider.connection, admin, mintA, vaultTokenA, admin, 100_000_000);
    await mintTo(provider.connection, admin, mintB, vaultTokenB, admin, 100_000_000);
  });

  it("initializes vault with AMM pointers", async () => {
    const vs = await vaultProgram.account.vaultState.fetch(vaultStatePda);
    assert.equal(vs.tokenMint.toBase58(), mintA.toBase58());
    assert.equal(vs.tokenBMint.toBase58(), mintB.toBase58());
    assert.equal(vs.ammPool.toBase58(), ammPoolPda.toBase58());
    assert.equal(vs.vaultTokenA.toBase58(), vaultTokenA.toBase58());
    assert.equal(vs.vaultTokenB.toBase58(), vaultTokenB.toBase58());
  });

  it("admin swaps A → B", async () => {
    const beforeA = (await getAccount(provider.connection, vaultTokenA)).amount;
    const beforeB = (await getAccount(provider.connection, vaultTokenB)).amount;

    await vaultProgram.methods
      .swap(new anchor.BN(10_000_000), new anchor.BN(1), true)
      .accounts({
        admin: admin.publicKey,
        vaultState: vaultStatePda,
        tokenMint: mintA,
        tokenBMint: mintB,
        vaultTokenA,
        vaultTokenB,
        ammPool: ammPoolPda,
        ammVaultA,
        ammVaultB,
        ammProgram: ammProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([admin])
      .rpc();

    const afterA = (await getAccount(provider.connection, vaultTokenA)).amount;
    const afterB = (await getAccount(provider.connection, vaultTokenB)).amount;

    assert.equal(Number(beforeA - afterA), 10_000_000, "A уменьшился");
    assert.isTrue(afterB > beforeB, "B вырос");
  });

  it("admin swaps B → A back", async () => {
    const beforeA = (await getAccount(provider.connection, vaultTokenA)).amount;
    const beforeB = (await getAccount(provider.connection, vaultTokenB)).amount;

    await vaultProgram.methods
      .swap(new anchor.BN(5_000_000), new anchor.BN(1), false)
      .accounts({
        admin: admin.publicKey,
        vaultState: vaultStatePda,
        tokenMint: mintA,
        tokenBMint: mintB,
        vaultTokenA,
        vaultTokenB,
        ammPool: ammPoolPda,
        ammVaultA,
        ammVaultB,
        ammProgram: ammProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([admin])
      .rpc();

    const afterA = (await getAccount(provider.connection, vaultTokenA)).amount;
    const afterB = (await getAccount(provider.connection, vaultTokenB)).amount;
    assert.isTrue(afterA > beforeA, "A вырос");
    assert.equal(Number(beforeB - afterB), 5_000_000, "B уменьшился");
  });

  it("rejects swap when min_amount_out too big (slippage)", async () => {
    let failed = false;
    try {
      await vaultProgram.methods
        .swap(new anchor.BN(10_000_000), new anchor.BN(999_000_000_000), true)
        .accounts({
          admin: admin.publicKey,
          vaultState: vaultStatePda,
          tokenMint: mintA,
          tokenBMint: mintB,
          vaultTokenA,
          vaultTokenB,
          ammPool: ammPoolPda,
          ammVaultA,
          ammVaultB,
          ammProgram: ammProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([admin])
        .rpc();
    } catch (e) {
      failed = true;
    }
    assert.isTrue(failed, "ожидали SlippageExceeded");
  });

  it("rejects swap from non-admin", async () => {
    let failed = false;
    try {
      await vaultProgram.methods
        .swap(new anchor.BN(1_000_000), new anchor.BN(1), true)
        .accounts({
          admin: user.publicKey,
          vaultState: vaultStatePda,
          tokenMint: mintA,
          tokenBMint: mintB,
          vaultTokenA,
          vaultTokenB,
          ammPool: ammPoolPda,
          ammVaultA,
          ammVaultB,
          ammProgram: ammProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([user])
        .rpc();
    } catch (e) {
      failed = true;
    }
    assert.isTrue(failed, "не-админ не должен свапать");
  });
});
