import { Keypair, PublicKey } from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  TOKEN_2022_PROGRAM_ID,
  getAssociatedTokenAddressSync,
  unpackAccount,
  createAssociatedTokenAccountInstruction,
  createTransferCheckedInstruction,
} from "@solana/spl-token";
import { ProgramTestContext } from "solana-bankrun";
import { assert } from "chai";

import {
  sendIxs,
  oneToken,
  findPDA,
  toBN,
  getTime,
  timeTravel,
  expectReverted,
  numToHex,
  DEFAULT_DECIMALS,
} from "../helpers";
import { setupStaking, StakingFixture } from "./fixture";

const LOCK_DURATION = 3n * 24n * 60n * 60n;

const fetchTokenAmount = async (
  context: ProgramTestContext,
  address: PublicKey,
  programId = TOKEN_PROGRAM_ID,
): Promise<bigint> => {
  const info = await context.banksClient.getAccount(address);
  if (!info) throw new Error(`Token account not found: ${address.toBase58()}`);
  return unpackAccount(address, info as any, programId).amount;
};

describe("02-staking", () => {
  let env: StakingFixture;

  const userState = (user: PublicKey): PublicKey =>
    findPDA(["user", user], env.program)[0];

  const stakePosition = (user: PublicKey, index: number): PublicKey =>
    findPDA(
      ["stake", user, toBN(index).toArrayLike(Buffer, "le", 8)],
      env.program,
    )[0];

  before(async () => {
    env = await setupStaking();
  });

  it("stakes one token into a new position", async () => {
    const { context, program, payer, rddkMint, userRddkAta } = env;
    const position = stakePosition(payer.publicKey, 0);

    const stakeIx = await program.methods
      .stake(oneToken())
      .accountsPartial({
        stakePosition: position,
        userRddkAta,
        rddkMint,
        user: payer.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .instruction();
    await sendIxs(context, [payer], stakeIx);

    const pos = await program.account.stakePosition.fetch(position);
    assert.equal(pos.amount.toString(), oneToken().toString());
    assert.isAbove(pos.stakedAt.toNumber(), 0);
    assert.isFalse(pos.claimed);

    const user = await program.account.userState.fetch(
      userState(payer.publicKey),
    );
    assert.equal(user.stakeCount.toNumber(), 1);
  });

  it("rejects claiming before the 3-day lock expires", async () => {
    const { context, program, payer, vrddkMint } = env;
    const position = stakePosition(payer.publicKey, 0);
    const userVrddkAta = getAssociatedTokenAddressSync(
      vrddkMint,
      payer.publicKey,
      false,
      TOKEN_2022_PROGRAM_ID,
    );

    const claimIx = await program.methods
      .claim(toBN(0))
      .accountsPartial({
        stakePosition: position,
        vrddkMint,
        userVrddkAta,
        user: payer.publicKey,
        token2022Program: TOKEN_2022_PROGRAM_ID,
      })
      .instruction();

    // 6001 = StillLocked
    await expectReverted(
      { revertedWith: { message: numToHex(6001) } },
      sendIxs(context, [payer], claimIx),
    );
  });

  it("mints vRDDK once 3 days have elapsed and blocks double-claiming", async () => {
    const { context, program, payer, vrddkMint } = env;
    const position = stakePosition(payer.publicKey, 0);
    const userVrddkAta = getAssociatedTokenAddressSync(
      vrddkMint,
      payer.publicKey,
      false,
      TOKEN_2022_PROGRAM_ID,
    );

    const now = await getTime(context);
    await timeTravel(context, now + LOCK_DURATION);

    const claimIx = await program.methods
      .claim(toBN(0))
      .accountsPartial({
        stakePosition: position,
        vrddkMint,
        userVrddkAta,
        user: payer.publicKey,
        token2022Program: TOKEN_2022_PROGRAM_ID,
      })
      .instruction();
    await sendIxs(context, [payer], claimIx);

    const pos = await program.account.stakePosition.fetch(position);
    assert.isTrue(pos.claimed);

    const vrddkBalance = await fetchTokenAmount(
      context,
      userVrddkAta,
      TOKEN_2022_PROGRAM_ID,
    );
    assert.equal(vrddkBalance.toString(), oneToken().toString());

    // 6002 = AlreadyClaimed
    await expectReverted(
      { revertedWith: { message: numToHex(6002) } },
      sendIxs(context, [payer], claimIx),
    );
  });

  it("enforces non-transferability on vRDDK tokens", async () => {
    const { context, payer, vrddkMint } = env;
    const recipient = Keypair.generate();

    const senderAta = getAssociatedTokenAddressSync(
      vrddkMint,
      payer.publicKey,
      false,
      TOKEN_2022_PROGRAM_ID,
    );
    const recipientAta = getAssociatedTokenAddressSync(
      vrddkMint,
      recipient.publicKey,
      false,
      TOKEN_2022_PROGRAM_ID,
    );

    const createAtaIx = createAssociatedTokenAccountInstruction(
      payer.publicKey,
      recipientAta,
      recipient.publicKey,
      vrddkMint,
      TOKEN_2022_PROGRAM_ID,
    );
    const transferIx = createTransferCheckedInstruction(
      senderAta,
      vrddkMint,
      recipientAta,
      payer.publicKey,
      1n,
      DEFAULT_DECIMALS,
      [],
      TOKEN_2022_PROGRAM_ID,
    );

    let failed = false;
    try {
      await sendIxs(context, [payer], createAtaIx, transferIx);
    } catch {
      failed = true;
    }
    assert.isTrue(failed, "vRDDK transfer must fail due to NonTransferable");
  });

  it("unstakes by burning vRDDK and returning matching RDDK", async () => {
    const { context, program, payer, rddkMint, vrddkMint, userRddkAta, vault } =
      env;
    const userVrddkAta = getAssociatedTokenAddressSync(
      vrddkMint,
      payer.publicKey,
      false,
      TOKEN_2022_PROGRAM_ID,
    );

    const rddkBefore = await fetchTokenAmount(context, userRddkAta);
    const vaultBefore = await fetchTokenAmount(context, vault);

    const unstakeIx = await program.methods
      .unstake(oneToken())
      .accountsPartial({
        vrddkMint,
        userVrddkAta,
        userRddkAta,
        rddkMint,
        user: payer.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
      })
      .instruction();
    await sendIxs(context, [payer], unstakeIx);

    const rddkAfter = await fetchTokenAmount(context, userRddkAta);
    const vaultAfter = await fetchTokenAmount(context, vault);
    const vrddkAfter = await fetchTokenAmount(
      context,
      userVrddkAta,
      TOKEN_2022_PROGRAM_ID,
    );

    assert.equal((rddkAfter - rddkBefore).toString(), oneToken().toString());
    assert.equal((vaultBefore - vaultAfter).toString(), oneToken().toString());
    assert.equal(vrddkAfter.toString(), "0");
  });

  it("supports multiple independent stakes with separate timers and rejects zero amounts", async () => {
    const { context, program, payer, rddkMint, vrddkMint, userRddkAta } = env;
    const userVrddkAta = getAssociatedTokenAddressSync(
      vrddkMint,
      payer.publicKey,
      false,
      TOKEN_2022_PROGRAM_ID,
    );

    // 6000 = ZeroAmount
    const zeroStakeIx = await program.methods
      .stake(toBN(0))
      .accountsPartial({
        stakePosition: stakePosition(payer.publicKey, 1),
        userRddkAta,
        rddkMint,
        user: payer.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .instruction();
    await expectReverted(
      { revertedWith: { message: numToHex(6000) } },
      sendIxs(context, [payer], zeroStakeIx),
    );

    // Stake #1 (index 1) at T0
    const stake1Ix = await program.methods
      .stake(oneToken().muln(2))
      .accountsPartial({
        stakePosition: stakePosition(payer.publicKey, 1),
        userRddkAta,
        rddkMint,
        user: payer.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .instruction();
    await sendIxs(context, [payer], stake1Ix);

    // Advance 2 days, then create Stake #2 (index 2)
    const t0 = await getTime(context);
    await timeTravel(context, t0 + 2n * 24n * 60n * 60n);

    const stake2Ix = await program.methods
      .stake(oneToken().muln(3))
      .accountsPartial({
        stakePosition: stakePosition(payer.publicKey, 2),
        userRddkAta,
        rddkMint,
        user: payer.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .instruction();
    await sendIxs(context, [payer], stake2Ix);

    // Advance 1 more day (3 days since Stake #1, only 1 day since Stake #2)
    const t1 = await getTime(context);
    await timeTravel(context, t1 + 1n * 24n * 60n * 60n);

    // Claiming Stake #1 succeeds
    const claim1Ix = await program.methods
      .claim(toBN(1))
      .accountsPartial({
        stakePosition: stakePosition(payer.publicKey, 1),
        vrddkMint,
        userVrddkAta,
        user: payer.publicKey,
        token2022Program: TOKEN_2022_PROGRAM_ID,
      })
      .instruction();
    await sendIxs(context, [payer], claim1Ix);

    // Claiming Stake #2 still fails (StillLocked)
    const claim2Ix = await program.methods
      .claim(toBN(2))
      .accountsPartial({
        stakePosition: stakePosition(payer.publicKey, 2),
        vrddkMint,
        userVrddkAta,
        user: payer.publicKey,
        token2022Program: TOKEN_2022_PROGRAM_ID,
      })
      .instruction();
    await expectReverted(
      { revertedWith: { message: numToHex(6001) } },
      sendIxs(context, [payer], claim2Ix),
    );
  });
});
