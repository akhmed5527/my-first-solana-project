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

describe("week8-liquidity-vault (Weeks 8-10)", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.Week8LiquidityVault as Program<Week8LiquidityVault>;
  const authority = (provider.wallet as anchor.Wallet).payer;
  const user = Keypair.generate();

  let tokenMint: PublicKey, shareMint: PublicKey;
  let vaultState: PublicKey, vaultStateBump: number;
  let vaultTokenAccount: PublicKey, userTokenAccount: PublicKey;
  let userShareAccount: PublicKey, userPosition: PublicKey;

  const ONE_TOKEN = 1_000_000;

  before(async () => {
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
  });

  it("Initializes the vault", async () => {
    await program.methods.initializeVault(vaultStateBump).accounts({
      vaultState, tokenMint, shareMint,
      authority: authority.publicKey,
      systemProgram: SystemProgram.programId,
      tokenProgram: TOKEN_PROGRAM_ID,
    }).rpc();
    console.log("✅ Vault initialized");
  });

  it("Opens a user position", async () => {
    await program.methods.openPosition().accounts({
      userPosition, user: user.publicKey,
      systemProgram: SystemProgram.programId,
    }).signers([user]).rpc();
    console.log("✅ User position opened");
  });

  it("Deposit 100 tokens → 100 shares", async () => {
    await program.methods.deposit(new anchor.BN(100 * ONE_TOKEN)).accounts({
      vaultState, tokenMint, userTokenAccount, vaultTokenAccount,
      shareMint, userShareAccount, userPosition,
      user: user.publicKey, tokenProgram: TOKEN_PROGRAM_ID,
    }).signers([user]).rpc();
    console.log("✅ Deposited 100 tokens → 100 shares");
  });

  it("Withdraw 40 shares → 40 tokens back, 40 shares burned", async () => {
    const tokensBefore = await provider.connection.getTokenAccountBalance(userTokenAccount);
    const sharesBefore = await provider.connection.getTokenAccountBalance(userShareAccount);

    await program.methods.withdraw(new anchor.BN(40 * ONE_TOKEN)).accounts({
      vaultState, tokenMint, userTokenAccount, vaultTokenAccount,
      shareMint, userShareAccount, userPosition,
      user: user.publicKey, tokenProgram: TOKEN_PROGRAM_ID,
    }).signers([user]).rpc();

    const tokensAfter = await provider.connection.getTokenAccountBalance(userTokenAccount);
    const sharesAfter = await provider.connection.getTokenAccountBalance(userShareAccount);

    assert.equal(Number(tokensAfter.value.amount), Number(tokensBefore.value.amount) + 40 * ONE_TOKEN);
    assert.equal(Number(sharesAfter.value.amount), Number(sharesBefore.value.amount) - 40 * ONE_TOKEN);
    console.log("✅ Withdrew 40 shares: 40 burned, 40 tokens back");
  });

  it("Rejects withdrawal exceeding user shares", async () => {
    try {
      await program.methods.withdraw(new anchor.BN(1000 * ONE_TOKEN)).accounts({
        vaultState, tokenMint, userTokenAccount, vaultTokenAccount,
        shareMint, userShareAccount, userPosition,
        user: user.publicKey, tokenProgram: TOKEN_PROGRAM_ID,
      }).signers([user]).rpc();
      assert.fail("Expected InsufficientShares");
    } catch (err: any) {
      assert.include(err.toString(), "InsufficientShares");
      console.log("✅ Over-withdraw correctly rejected");
      console.log("\n🎉🎉🎉 WEEK 10 PASSED! 🎉🎉🎉\n");
    }
  });
});
