import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Week8LiquidityVault } from "../target/types/week8_liquidity_vault";
import { PublicKey, Keypair, SystemProgram, Transaction } from "@solana/web3.js";
import { 
  createMint, 
  createAccount, 
  mintTo, 
  getAccount, 
  TOKEN_PROGRAM_ID,
  createInitializeAccountInstruction,
  getMinimumBalanceForRentExemptAccount,
} from "@solana/spl-token";

describe("week8-liquidity-vault", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.Week8LiquidityVault as Program<Week8LiquidityVault>;
  const payer = (provider.wallet as anchor.Wallet).payer;

  it("Deposits tokens and mints shares!", async () => {
    // 1. Создаем базовый минт (токен, который вносим)
    const tokenMint = await createMint(provider.connection, payer, payer.publicKey, null, 9);

    // 2. Находим PDA для Vault State (делаем это ДО создания shareMint)
    const [vaultStatePda, bump] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault_state"), tokenMint.toBuffer()],
      program.programId
    );

    // 3. Создаем Share Mint. ВАЖНО: Владельцем (Mint Authority) назначаем PDA Vault!
    const shareMint = await createMint(provider.connection, payer, vaultStatePda, null, 9);

    // 4. Инициализируем Vault
    await program.methods.initializeVault(bump)
      .accounts({
        vaultState: vaultStatePda,
        tokenMint: tokenMint,
        shareMint: shareMint,
        authority: payer.publicKey,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();

    // 5. Создаем Token Accounts
    const userTokenAccount = await createAccount(provider.connection, payer, tokenMint, payer.publicKey);
    const userShareAccount = await createAccount(provider.connection, payer, shareMint, payer.publicKey);

    // Аккаунт для Vault (владелец - PDA)
    const vaultTokenAccountKeypair = Keypair.generate();
    const vaultTokenAccount = vaultTokenAccountKeypair.publicKey;
    
    const rentLamports = await getMinimumBalanceForRentExemptAccount(provider.connection);
    const tx = new Transaction().add(
      SystemProgram.createAccount({
        fromPubkey: payer.publicKey,
        newAccountPubkey: vaultTokenAccount,
        space: 165,
        lamports: rentLamports,
        programId: TOKEN_PROGRAM_ID,
      }),
      createInitializeAccountInstruction(
        vaultTokenAccount,
        tokenMint,
        vaultStatePda,
        TOKEN_PROGRAM_ID
      )
    );
    await provider.sendAndConfirm(tx, [vaultTokenAccountKeypair, payer]);

    // 6. Печатаем 1000 токенов пользователю
    await mintTo(provider.connection, payer, tokenMint, userTokenAccount, payer, 1000);

    // 7. Вызываем ДЕПОЗИТ
    await program.methods.deposit(new anchor.BN(100))
      .accounts({
        vaultState: vaultStatePda,
        tokenMint: tokenMint,
        userTokenAccount: userTokenAccount,
        vaultTokenAccount: vaultTokenAccount,
        shareMint: shareMint,
        userShareAccount: userShareAccount,
        user: payer.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();

    // 8. Проверяем результаты
    const vaultTokenBalance = (await getAccount(provider.connection, vaultTokenAccount)).amount;
    const userShareBalance = (await getAccount(provider.connection, userShareAccount)).amount;

    console.log("Vault received tokens:", vaultTokenBalance.toString());
    console.log("User received shares:", userShareBalance.toString());

    if (vaultTokenBalance.toString() !== "100" || userShareBalance.toString() !== "100") {
      throw new Error("Balances do not match! Deposit failed.");
    }
    console.log("Deposit Successful! 1:1 ratio verified.");
  });
});
