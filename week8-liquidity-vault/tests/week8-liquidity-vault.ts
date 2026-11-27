import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Week8LiquidityVault } from "../target/types/week8_liquidity_vault";
import { PublicKey } from "@solana/web3.js";
import { createMint } from "@solana/spl-token";

describe("week8-liquidity-vault", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.Week8LiquidityVault as Program<Week8LiquidityVault>;

  it("Initializes the Vault State!", async () => {
    // 1. Создаем фейковые минты (токен и share-токен) для теста
    const tokenMint = await createMint(
      provider.connection,
      (provider.wallet as anchor.Wallet).payer,
      provider.wallet.publicKey,
      null,
      9
    );

    const shareMint = await createMint(
      provider.connection,
      (provider.wallet as anchor.Wallet).payer,
      provider.wallet.publicKey,
      null,
      9
    );

    // 2. Находим PDA для vault_state
    const [vaultStatePda, bump] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault_state"), tokenMint.toBuffer()],
      program.programId
    );

    // 3. Инициализируем Vault
    await program.methods.initializeVault(bump)
      .accounts({
        vaultState: vaultStatePda,
        tokenMint: tokenMint,
        shareMint: shareMint,
        authority: provider.wallet.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
        tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
      })
      .rpc();

    // 4. Читаем данные из блокчейна
    const vaultState = await program.account.vaultState.fetch(vaultStatePda);
    console.log("Vault Authority:", vaultState.authority.toBase58());
    console.log("Vault Initialized Successfully!");
  });
});
