import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Week5CounterProgram } from "../target/types/week5_counter_program";

describe("week5-counter-program", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.Week5CounterProgram as Program<Week5CounterProgram>;
  const counter = anchor.web3.Keypair.generate();

  it("Is initialized!", async () => {
    await program.methods.initialize().accounts({
      counter: counter.publicKey,
      user: provider.wallet.publicKey,
      systemProgram: anchor.web3.SystemProgram.programId,
    }).signers([counter]).rpc();

    const account = await program.account.counter.fetch(counter.publicKey);
    console.log("Initial count:", account.count.toNumber());
  });

  it("Increments!", async () => {
    await program.methods.increment().accounts({
      counter: counter.publicKey,
    }).rpc();

    const account = await program.account.counter.fetch(counter.publicKey);
    console.log("Count after increment:", account.count.toNumber());
  });
});
