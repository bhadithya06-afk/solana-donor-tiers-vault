import { Keypair, PublicKey, Transaction } from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  AccountLayout,
  createAssociatedTokenAccountInstruction,
  createMintToInstruction,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import { assert } from "chai";

import {
  sendIxs,
  processTransaction,
  oneToken,
  findPDA,
  toBN,
  getTime,
  timeTravel,
  expectReverted,
  numToHex,
} from "../helpers";
import { setupRaffle, fulfillRandomness, RaffleFixture } from "./fixture";

describe("05-raffle", () => {
  let env: RaffleFixture;

  const rafflePda = (): PublicKey => findPDA(["raffle"], env.program)[0];
  const vaultPda = (): PublicKey => findPDA(["vault"], env.program)[0];
  const entryPda = (index: number): PublicKey =>
    findPDA(["entry", toBN(index).toArrayLike(Buffer, "le", 8)], env.program)[0];

  const fundPlayer = async (player: PublicKey, amount: bigint) => {
    const { context, payer, mint } = env;
    const ata = getAssociatedTokenAddressSync(mint, player);
    await processTransaction(
      context,
      new Transaction().add(
        createAssociatedTokenAccountInstruction(payer.publicKey, ata, player, mint),
        createMintToInstruction(mint, ata, payer.publicKey, amount),
      ),
      [payer],
    );
    return ata;
  };

  before(async () => {
    env = await setupRaffle();
  });

  it("initializes the raffle and rejects early or empty randomness requests", async () => {
    const { context, program, payer, mint, oracle, randomness } = env;

    const now = await getTime(context);
    const drawTime = now + 3600n; // 1 hour from now

    const initIx = await program.methods
      .initialize(toBN(drawTime), oracle.publicKey)
      .accountsPartial({
        raffle: rafflePda(),
        vault: vaultPda(),
        randomness,
        mint,
        payer: payer.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .instruction();
    await sendIxs(context, [payer], initIx);

    const raffle = await program.account.raffle.fetch(rafflePda());
    assert.equal(raffle.drawTime.toString(), drawTime.toString());

    const rnd = await program.account.randomness.fetch(randomness);
    assert.equal(rnd.oracle.toBase58(), oracle.publicKey.toBase58());
    assert.isFalse(rnd.fulfilled);

    // 6004 = RaffleStillOpen
    const reqIx = await program.methods
      .requestRandomness()
      .accountsPartial({
        payer: payer.publicKey,
        raffle: rafflePda(),
        randomness,
      })
      .instruction();
    await expectReverted(
      { revertedWith: { message: numToHex(6004) } },
      sendIxs(context, [payer], reqIx),
    );
  });

  it("accepts weighted deposits, rejects zero/late deposits, and rejects untrusted oracles", async () => {
    const { context, program, payer, mint, funded, randomness } = env;

    const player0 = funded[0];
    const player1 = funded[1];
    const player2 = funded[2];

    const d0 = oneToken().muln(1); // [0, 1_000_000)
    const d1 = oneToken().muln(3); // [1_000_000, 4_000_000)
    const d2 = oneToken().muln(6); // [4_000_000, 10_000_000)

    const ata0 = await fundPlayer(player0.publicKey, BigInt(d0.muln(2).toString()));
    const ata1 = await fundPlayer(player1.publicKey, BigInt(d1.toString()));
    const ata2 = await fundPlayer(player2.publicKey, BigInt(d2.toString()));

    // 6001 = ZeroAmount
    const zeroIx = await program.methods
      .deposit(toBN(0))
      .accountsPartial({
        entry: entryPda(0),
        entrantAta: ata0,
        mint,
        entrant: player0.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .instruction();
    await expectReverted(
      { revertedWith: { message: numToHex(6001) } },
      sendIxs(context, [player0], zeroIx),
    );

    // Deposit 0: 1 token
    const dep0Ix = await program.methods
      .deposit(d0)
      .accountsPartial({
        entry: entryPda(0),
        entrantAta: ata0,
        mint,
        entrant: player0.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .instruction();
    await sendIxs(context, [player0], dep0Ix);

    // Deposit 1: 3 tokens
    const dep1Ix = await program.methods
      .deposit(d1)
      .accountsPartial({
        entry: entryPda(1),
        entrantAta: ata1,
        mint,
        entrant: player1.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .instruction();
    await sendIxs(context, [player1], dep1Ix);

    // Deposit 2: 6 tokens
    const dep2Ix = await program.methods
      .deposit(d2)
      .accountsPartial({
        entry: entryPda(2),
        entrantAta: ata2,
        mint,
        entrant: player2.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .instruction();
    await sendIxs(context, [player2], dep2Ix);

    const raffle = await program.account.raffle.fetch(rafflePda());
    assert.equal(raffle.entryCount.toNumber(), 3);
    assert.equal(
      raffle.totalDeposited.toString(),
      oneToken().muln(10).toString(),
    );

    // Advance past draw_time
    await timeTravel(context, BigInt(raffle.drawTime.toString()) + 1n);

    // 6003 = RaffleClosed
    const lateDepIx = await program.methods
      .deposit(d0)
      .accountsPartial({
        entry: entryPda(3),
        entrantAta: ata0,
        mint,
        entrant: player0.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .instruction();
    await expectReverted(
      { revertedWith: { message: numToHex(6003) } },
      sendIxs(context, [player0], lateDepIx),
    );

    // 6000 = UntrustedOracle
    const fakeOracle = Keypair.generate();
    const fakeCallbackIx = await program.methods
      .consumeRandomness(Array.from({ length: 32 }, (_, i) => i))
      .accountsPartial({
        oracle: fakeOracle.publicKey,
        randomness,
      })
      .instruction();
    await expectReverted(
      { revertedWith: { message: numToHex(6000) } },
      sendIxs(context, [payer, fakeOracle], fakeCallbackIx),
    );

    // 6007 = RandomnessNotFulfilled (cannot claim before VRF callback)
    const earlyClaimIx = await program.methods
      .claim(toBN(0))
      .accountsPartial({
        entry: entryPda(0),
        winnerAta: ata0,
        mint,
        entrant: player0.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .instruction();
    await expectReverted(
      { revertedWith: { message: numToHex(6007) } },
      sendIxs(context, [player0], earlyClaimIx),
    );
  });

  it("draws a weighted winner who takes the whole pot and blocks losers and double claims", async () => {
    const { context, program, mint, funded, randomness } = env;

    const randomBytes = Array.from({ length: 32 }, (_, i) => i + 7);
    await fulfillRandomness(env, randomBytes);

    const rndAccount = await program.account.randomness.fetch(randomness);
    assert.isTrue(rndAccount.fulfilled);

    const raffle = await program.account.raffle.fetch(rafflePda());
    const totalDeposited = BigInt(raffle.totalDeposited.toString());

    // Replicate on-chain u128::from_le_bytes(value[..16]) % total_deposited
    const low16 = Buffer.from(rndAccount.value.slice(0, 16));
    const randU128 = low16.readBigUInt64LE(0) + (low16.readBigUInt64LE(8) << 64n);
    const winningTicket = randU128 % totalDeposited;

    let winningIndex = -1;
    for (let i = 0; i < 3; i++) {
      const entry = await program.account.entry.fetch(entryPda(i));
      const start = BigInt(entry.rangeStart.toString());
      const end = BigInt(entry.rangeEnd.toString());
      if (winningTicket >= start && winningTicket < end) {
        winningIndex = i;
        break;
      }
    }
    assert.notEqual(winningIndex, -1, "must find a valid winning entry");

    // Non-winning entry must fail with 6008 = NotWinner
    const losingIndex = (winningIndex + 1) % 3;
    const loser = funded[losingIndex];
    const loserAta = getAssociatedTokenAddressSync(mint, loser.publicKey);
    const loserClaimIx = await program.methods
      .claim(toBN(losingIndex))
      .accountsPartial({
        entry: entryPda(losingIndex),
        winnerAta: loserAta,
        mint,
        entrant: loser.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .instruction();
    await expectReverted(
      { revertedWith: { message: numToHex(6008) } },
      sendIxs(context, [loser], loserClaimIx),
    );

    // Winner claims the entire pot
    const winner = funded[winningIndex];
    const winnerAta = getAssociatedTokenAddressSync(mint, winner.publicKey);
    const beforeAcc = await context.banksClient.getAccount(winnerAta);
    const balanceBefore = AccountLayout.decode(beforeAcc!.data).amount;

    const winClaimIx = await program.methods
      .claim(toBN(winningIndex))
      .accountsPartial({
        entry: entryPda(winningIndex),
        winnerAta,
        mint,
        entrant: winner.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .instruction();
    await sendIxs(context, [winner], winClaimIx);

    const afterAcc = await context.banksClient.getAccount(winnerAta);
    const balanceAfter = AccountLayout.decode(afterAcc!.data).amount;
    assert.equal(
      (balanceAfter - balanceBefore).toString(),
      totalDeposited.toString(),
    );

    const vaultAcc = await context.banksClient.getAccount(vaultPda());
    const vaultBalance = AccountLayout.decode(vaultAcc!.data).amount;
    assert.equal(vaultBalance.toString(), "0");

    // 6009 = AlreadyClaimed
    await expectReverted(
      { revertedWith: { message: numToHex(6009) } },
      sendIxs(context, [winner], winClaimIx),
    );
  });
});
