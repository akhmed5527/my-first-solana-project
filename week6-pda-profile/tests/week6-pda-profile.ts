import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Week6PdaProfile } from "../target/types/week6_pda_profile";

describe("week6-pda-profile", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.Week6PdaProfile as Program<Week6PdaProfile>;
  const user = anchor.web3.Keypair.generate();

  it("Creates a profile PDA!", async () => {
    // 👇 ВОТ ЭТИ ДВЕ СТРОЧКИ: Даем пользователю 2 SOL для оплаты транзакции
    const airdropSig = await provider.connection.requestAirdrop(user.publicKey, 2 * anchor.web3.LAMPORTS_PER_SOL);
    await provider.connection.confirmTransaction(airdropSig);

    const [profilePda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("profile"), user.publicKey.toBuffer()],
      program.programId
    );

    await program.methods.createProfile("Akhmed", "Solana Builder")
      .accounts({
        profile: profilePda,
        user: user.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([user])
      .rpc();

    const profileAccount = await program.account.profile.fetch(profilePda);
    console.log("Profile Name:", profileAccount.name);
    console.log("Profile Bio:", profileAccount.bio);
  });
});
