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
    createInitializeAccountInstruction(keypair.publicKey, mint, owner, TOKEN_PROGRAM_ID)
  );
  await anchor.web3.sendAndConfirmTransaction(connection, tx, [payer, keypair]);
  return keypair.publicKey;
}

describe("week13-vault-liquidity-position", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const vaultProgram = anchor.workspace.Week8LiquidityVault as Program<Week8LiquidityVault>;
  const ammProgram = anchor.workspace.SimpleAmm as Program<SimpleAmm>;

  const admin = (provider.wallet as anchor.Wallet).payer;
  const user = Keypair.generate();

  let mintA: PublicKey, mintB: PublicKey, shareMint: PublicKey;
  let vaultTokenA: PublicKey, vaultTokenB: PublicKey, vaultLpAccount: PublicKey;
  let vaultStatePda: PublicKey;
  let ammPoolPda: PublicKey, ammLpMintPda: PublicKey;
  let ammVaultA: PublicKey, ammVaultB: PublicKey, ammPoolBump: number;

  let adminA: PublicKey, adminB: PublicKey, adminLp: PublicKey;
  let userPosPda: PublicKey;

  before(async () => {
    const sig = await provider.connection.requestAirdrop(user.publicKey, 5 * anchor.web3.LAMPORTS_PER_SOL);
    await provider.connection.confirmTransaction(sig);

    mintA = await createMint(provider.connection, admin, admin.publicKey, null, 6);
    mintB = await createMint(provider.connection, admin, admin.publicKey, null, 6);

    [vaultStatePda] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault_state"), mintA.toBuffer()],
      vaultProgram.programId
    );

    shareMint = await createMint(provider.connection, admin, vaultStatePda, null, 6);

    [ammPoolPda, ammPoolBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("pool"), mintA.toBuffer(), mintB.toBuffer()], ammProgram.programId
    );
    [ammLpMintPda] = PublicKey.findProgramAddressSync([Buffer.from("lp_mint"), ammPoolPda.toBuffer()], ammProgram.programId);
    [ammVaultA] = PublicKey.findProgramAddressSync([Buffer.from("vault_a"), ammPoolPda.toBuffer()], ammProgram.programId);
    [ammVaultB] = PublicKey.findProgramAddressSync([Buffer.from("vault_b"), ammPoolPda.toBuffer()], ammProgram.programId);

    // AMM init pool
    await ammProgram.methods.initializePool(30, ammPoolBump).accounts({
      pool: ammPoolPda, tokenAMint: mintA, tokenBMint: mintB, lpMint: ammLpMintPda,
      vaultA: ammVaultA, vaultB: ammVaultB, authority: admin.publicKey,
      systemProgram: SystemProgram.programId, tokenProgram: TOKEN_PROGRAM_ID, rent: SYSVAR_RENT_PUBKEY,
    }).signers([admin]).rpc();

    // AMM add liquidity (external LP)
    adminA = await createAccount(provider.connection, admin, mintA, admin.publicKey);
    adminB = await createAccount(provider.connection, admin, mintB, admin.publicKey);
    adminLp = await createAccount(provider.connection, admin, ammLpMintPda, admin.publicKey);
    await mintTo(provider.connection, admin, mintA, adminA, admin, 1_000_000_000);
    await mintTo(provider.connection, admin, mintB, adminB, admin, 1_000_000_000);
    await ammProgram.methods.addLiquidity(
      new anchor.BN(500_000_000), new anchor.BN(500_000_000), new anchor.BN(0)
    ).accounts({
      pool: ammPoolPda, tokenAMint: mintA, tokenBMint: mintB,
      vaultA: ammVaultA, vaultB: ammVaultB, lpMint: ammLpMintPda,
      userA: adminA, userB: adminB, userLp: adminLp, user: admin.publicKey,
      tokenProgram: TOKEN_PROGRAM_ID,
    }).signers([admin]).rpc();

    // Vault token accounts
    vaultTokenA = await createRawTokenAccount(provider.connection, admin, mintA, vaultStatePda);
    vaultTokenB = await createRawTokenAccount(provider.connection, admin, mintB, vaultStatePda);
    vaultLpAccount = await createRawTokenAccount(provider.connection, admin, ammLpMintPda, vaultStatePda);

    const [, expectedBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault_state"), mintA.toBuffer()], vaultProgram.programId
    );

    await vaultProgram.methods.initializeVault(
      expectedBump, mintB, vaultTokenA, vaultTokenB, vaultLpAccount, ammPoolPda
    ).accounts({
      vaultState: vaultStatePda, tokenMint: mintA, shareMint: shareMint,
      authority: admin.publicKey, systemProgram: SystemProgram.programId, tokenProgram: TOKEN_PROGRAM_ID,
    }).signers([admin]).rpc();

    [userPosPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("user_position"), user.publicKey.toBuffer()], vaultProgram.programId
    );
    await vaultProgram.methods.openPosition().accounts({
      userPosition: userPosPda, user: user.publicKey, systemProgram: SystemProgram.programId,
    }).signers([user]).rpc();

    await mintTo(provider.connection, admin, mintA, vaultTokenA, admin, 100_000_000);
    await mintTo(provider.connection, admin, mintB, vaultTokenB, admin, 100_000_000);
  });

  // ---- Вспомогательная функция: аккаунты для position-инструкций ----
  const positionAccounts = () => ({
    admin: admin.publicKey,
    vaultState: vaultStatePda,
    tokenMint: mintA,
    tokenBMint: mintB,
    vaultTokenA,
    vaultTokenB,
    vaultLpAccount,
    ammPool: ammPoolPda,
    ammVaultA,
    ammVaultB,
    ammLpMint: ammLpMintPda,
    ammProgram: ammProgram.programId,
    tokenProgram: TOKEN_PROGRAM_ID,
  });

  it("1. initializes vault with LP account", async () => {
    const vs = await vaultProgram.account.vaultState.fetch(vaultStatePda);
    assert.equal(vs.vaultLpAccount.toBase58(), vaultLpAccount.toBase58());
    assert.isFalse(vs.positionOpen);
    assert.equal(vs.initialA.toNumber(), 0);
  });

  it("2. admin opens AMM position: LP minted to vault_lp_account", async () => {
    const beforeA = (await getAccount(provider.connection, vaultTokenA)).amount;
    const beforeB = (await getAccount(provider.connection, vaultTokenB)).amount;

    await vaultProgram.methods.openAmmPosition(
      new anchor.BN(50_000_000), new anchor.BN(50_000_000), new anchor.BN(0)
    ).accounts(positionAccounts()).signers([admin]).rpc();

    const afterA = (await getAccount(provider.connection, vaultTokenA)).amount;
    const afterB = (await getAccount(provider.connection, vaultTokenB)).amount;
    const lp = (await getAccount(provider.connection, vaultLpAccount)).amount;

    assert.equal(Number(beforeA - afterA), 50_000_000);
    assert.equal(Number(beforeB - afterB), 50_000_000);
    assert.isTrue(Number(lp) > 0);

    const vs = await vaultProgram.account.vaultState.fetch(vaultStatePda);
    assert.isTrue(vs.positionOpen);
    assert.equal(vs.initialA.toNumber(), 50_000_000);
  });

  it("3. rejects withdraw while position_open=true", async () => {
    // Пытаемся withdraw 1 share — должно упасть PositionOpen
    let failed = false;
    try {
      const userTokenA = await createRawTokenAccount(provider.connection, admin, mintA, user.publicKey);
      const userShare = await createRawTokenAccount(provider.connection, admin, shareMint, user.publicKey);
      await vaultProgram.methods.withdraw(new anchor.BN(1)).accounts({
        vaultState: vaultStatePda, tokenMint: mintA,
        userTokenAccount: userTokenA, vaultTokenAccount: vaultTokenA,
        shareMint: shareMint, userShareAccount: userShare,
        userPosition: userPosPda, user: user.publicKey, tokenProgram: TOKEN_PROGRAM_ID,
      }).signers([user]).rpc();
    } catch (e) {
      failed = true;
    }
    assert.isTrue(failed, "ожидали PositionOpen");
  });

  it("4. external swaps move price: pool reserves change", async () => {
    const poolBefore = await ammProgram.account.pool.fetch(ammPoolPda);
    const userA = await createRawTokenAccount(provider.connection, admin, mintA, admin.publicKey);
    await mintTo(provider.connection, admin, mintA, userA, admin, 50_000_000);
    const userB = await createRawTokenAccount(provider.connection, admin, mintB, admin.publicKey);

    await ammProgram.methods.swap(
      new anchor.BN(20_000_000), new anchor.BN(0), true
    ).accounts({
      pool: ammPoolPda, tokenAMint: mintA, tokenBMint: mintB,
      vaultA: ammVaultA, vaultB: ammVaultB,
      userA: userA, userB: userB, user: admin.publicKey,
      tokenProgram: TOKEN_PROGRAM_ID,
    }).signers([admin]).rpc();

    const poolAfter = await ammProgram.account.pool.fetch(ammPoolPda);
    assert.isTrue(poolAfter.reserveA.gt(poolBefore.reserveA), "reserveA вырос");
    assert.isTrue(poolAfter.reserveB.lt(poolBefore.reserveB), "reserveB упал");
  });

  it("5. admin collects part of fees (partial remove_liquidity)", async () => {
    const lpBefore = (await getAccount(provider.connection, vaultLpAccount)).amount;
    const aBefore = (await getAccount(provider.connection, vaultTokenA)).amount;
    const bBefore = (await getAccount(provider.connection, vaultTokenB)).amount;

    const lpToCollect = Math.floor(Number(lpBefore) / 10); // 10% LP

    await vaultProgram.methods.collectFees(new anchor.BN(lpToCollect))
      .accounts(positionAccounts()).signers([admin]).rpc();

    const lpAfter = (await getAccount(provider.connection, vaultLpAccount)).amount;
    const aAfter = (await getAccount(provider.connection, vaultTokenA)).amount;
    const bAfter = (await getAccount(provider.connection, vaultTokenB)).amount;

    assert.equal(Number(lpBefore - lpAfter), lpToCollect);
    assert.isTrue(Number(aAfter) > Number(aBefore), "A вернулись");
    assert.isTrue(Number(bAfter) > Number(bBefore), "B вернулись");

    const vs = await vaultProgram.account.vaultState.fetch(vaultStatePda);
    assert.isTrue(vs.positionOpen, "позиция всё ещё открыта");
  });

  it("6. admin closes position: both tokens back to vault", async () => {
    const lpBefore = (await getAccount(provider.connection, vaultLpAccount)).amount;
    assert.isTrue(Number(lpBefore) > 0);

    const aBefore = (await getAccount(provider.connection, vaultTokenA)).amount;
    const bBefore = (await getAccount(provider.connection, vaultTokenB)).amount;

    await vaultProgram.methods.closeAmmPosition(
      new anchor.BN(0), new anchor.BN(0)
    ).accounts(positionAccounts()).signers([admin]).rpc();

    const lpAfter = (await getAccount(provider.connection, vaultLpAccount)).amount;
    const aAfter = (await getAccount(provider.connection, vaultTokenA)).amount;
    const bAfter = (await getAccount(provider.connection, vaultTokenB)).amount;

    assert.equal(Number(lpAfter), 0, "LP сожжены");
    assert.isTrue(Number(aAfter) > Number(aBefore));
    assert.isTrue(Number(bAfter) > Number(bBefore));

    const vs = await vaultProgram.account.vaultState.fetch(vaultStatePda);
    assert.isFalse(vs.positionOpen);
    assert.equal(vs.initialA.toNumber(), 0);
  });

  it("7. rebalance: close → swap ratio → open new position", async () => {
    // swap B→A через AMM, чтобы сместить пропорцию
    const userB = await createRawTokenAccount(provider.connection, admin, mintB, admin.publicKey);
    await mintTo(provider.connection, admin, mintB, userB, admin, 30_000_000);
    const userA = await createRawTokenAccount(provider.connection, admin, mintA, admin.publicKey);

    await ammProgram.methods.swap(
      new anchor.BN(15_000_000), new anchor.BN(0), false
    ).accounts({
      pool: ammPoolPda, tokenAMint: mintA, tokenBMint: mintB,
      vaultA: ammVaultA, vaultB: ammVaultB,
      userA: userA, userB: userB, user: admin.publicKey, tokenProgram: TOKEN_PROGRAM_ID,
    }).signers([admin]).rpc();

    // Открываем новую позицию
    await vaultProgram.methods.openAmmPosition(
      new anchor.BN(10_000_000), new anchor.BN(10_000_000), new anchor.BN(0)
    ).accounts(positionAccounts()).signers([admin]).rpc();

    const vs = await vaultProgram.account.vaultState.fetch(vaultStatePda);
    assert.isTrue(vs.positionOpen);
  });

  it("8. rejects collect_fees when position closed", async () => {
    // Сначала закроем
    await vaultProgram.methods.closeAmmPosition(
      new anchor.BN(0), new anchor.BN(0)
    ).accounts(positionAccounts()).signers([admin]).rpc();

    let failed = false;
    try {
      await vaultProgram.methods.collectFees(new anchor.BN(1))
        .accounts(positionAccounts()).signers([admin]).rpc();
    } catch (e) {
      failed = true;
    }
    assert.isTrue(failed, "ожидали PositionNotOpen");
  });
});
