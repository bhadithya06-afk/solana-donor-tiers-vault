import { Keypair, PublicKey, SYSVAR_INSTRUCTIONS_PUBKEY } from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  AccountLayout,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import { assert } from "chai";

import {
  sendIxs,
  findPDA,
  oneToken,
  expectReverted,
  numToHex,
} from "../helpers";
import { setupAirdrop, MerkleFixture } from "./fixture";
import { buildClaimSignatureIx } from "./signatures";

describe("04-merkle-airdrop", () => {
  let env: MerkleFixture;

  const claimPda = (claimant: PublicKey): PublicKey =>
    findPDA(["claim", claimant], env.program)[0];

  before(async () => {
    env = await setupAirdrop();
  });

  it("prevents re-initializing an already initialized config", async () => {
    const { context, program, payer, mint, tree, signer } = env;

    const reinitIx = await program.methods
      .initialize([...tree.root], signer.publicKey)
      .accountsPartial({
        mint,
        payer: payer.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .instruction();

    let reverted = false;
    try {
      await sendIxs(context, [payer], reinitIx);
    } catch {
      reverted = true;
    }
    assert.isTrue(reverted, "initialize must not run twice on the same config");
  });

  it("claims an allocation with a valid Merkle proof", async () => {
    const { context, program, mint, vault, entries, tree, claimants } = env;

    const claimant = claimants[0];
    const amount = entries[0].amount;
    const proof = tree.getProof(0).map((node) => [...node]);
    const claimantAta = getAssociatedTokenAddressSync(mint, claimant.publicKey);

    const ix = await program.methods
      .claim(amount, proof)
      .accountsPartial({
        claim: claimPda(claimant.publicKey),
        claimant: claimant.publicKey,
        claimantAta,
        mint,
        vault,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .instruction();
    await sendIxs(context, [claimant], ix);

    const acc = await context.banksClient.getAccount(claimantAta);
    const balance = AccountLayout.decode(acc!.data).amount;
    assert.equal(balance.toString(), amount.toString());
  });

  it("rejects double-claiming via Merkle proof or signature after a Merkle claim", async () => {
    const { context, program, mint, vault, entries, tree, claimants, signer } =
      env;

    const claimant = claimants[0];
    const amount = entries[0].amount;
    const proof = tree.getProof(0).map((node) => [...node]);
    const claimantAta = getAssociatedTokenAddressSync(mint, claimant.publicKey);

    const repeatMerkleIx = await program.methods
      .claim(amount, proof)
      .accountsPartial({
        claim: claimPda(claimant.publicKey),
        claimant: claimant.publicKey,
        claimantAta,
        mint,
        vault,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .instruction();

    let merkleReverted = false;
    try {
      await sendIxs(context, [claimant], repeatMerkleIx);
    } catch {
      merkleReverted = true;
    }
    assert.isTrue(merkleReverted, "repeat Merkle claim must fail");

    const sigIx = buildClaimSignatureIx(
      signer,
      claimant.publicKey,
      amount,
      program.programId,
    );
    const claimSigIx = await program.methods
      .claimWithSignature(amount)
      .accountsPartial({
        claim: claimPda(claimant.publicKey),
        claimant: claimant.publicKey,
        claimantAta,
        mint,
        vault,
        instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .instruction();

    let sigReverted = false;
    try {
      await sendIxs(context, [claimant], sigIx, claimSigIx);
    } catch {
      sigReverted = true;
    }
    assert.isTrue(sigReverted, "signature claim after Merkle claim must fail");
  });

  it("rejects an invalid Merkle proof or tampered amount", async () => {
    const { context, program, mint, vault, entries, tree, claimants } = env;

    const claimant = claimants[1];
    const amount = entries[1].amount;
    const wrongProof = tree.getProof(0).map((node) => [...node]);
    const claimantAta = getAssociatedTokenAddressSync(mint, claimant.publicKey);

    // 6001 = InvalidProof
    const badProofIx = await program.methods
      .claim(amount, wrongProof)
      .accountsPartial({
        claim: claimPda(claimant.publicKey),
        claimant: claimant.publicKey,
        claimantAta,
        mint,
        vault,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .instruction();
    await expectReverted(
      { revertedWith: { message: numToHex(6001) } },
      sendIxs(context, [claimant], badProofIx),
    );

    const validProof = tree.getProof(1).map((node) => [...node]);
    const inflatedAmountIx = await program.methods
      .claim(amount.muln(2), validProof)
      .accountsPartial({
        claim: claimPda(claimant.publicKey),
        claimant: claimant.publicKey,
        claimantAta,
        mint,
        vault,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .instruction();
    await expectReverted(
      { revertedWith: { message: numToHex(6001) } },
      sendIxs(context, [claimant], inflatedAmountIx),
    );
  });

  it("claims an allocation with a valid authority signature and blocks subsequent Merkle claim", async () => {
    const { context, program, mint, vault, entries, tree, claimants, signer } =
      env;

    const claimant = claimants[2];
    const amount = entries[2].amount;
    const claimantAta = getAssociatedTokenAddressSync(mint, claimant.publicKey);

    const sigIx = buildClaimSignatureIx(
      signer,
      claimant.publicKey,
      amount,
      program.programId,
    );
    const claimSigIx = await program.methods
      .claimWithSignature(amount)
      .accountsPartial({
        claim: claimPda(claimant.publicKey),
        claimant: claimant.publicKey,
        claimantAta,
        mint,
        vault,
        instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .instruction();
    await sendIxs(context, [claimant], sigIx, claimSigIx);

    const acc = await context.banksClient.getAccount(claimantAta);
    const balance = AccountLayout.decode(acc!.data).amount;
    assert.equal(balance.toString(), amount.toString());

    const proof = tree.getProof(2).map((node) => [...node]);
    const merkleIx = await program.methods
      .claim(amount, proof)
      .accountsPartial({
        claim: claimPda(claimant.publicKey),
        claimant: claimant.publicKey,
        claimantAta,
        mint,
        vault,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .instruction();

    let reverted = false;
    try {
      await sendIxs(context, [claimant], merkleIx);
    } catch {
      reverted = true;
    }
    assert.isTrue(reverted, "Merkle claim after signature claim must fail");
  });

  it("rejects a signature meant for another recipient, untrusted signer, or wrong program", async () => {
    const { context, program, mint, vault, claimants, signer } = env;

    const claimant = claimants[3];
    const otherUser = claimants[1];
    const amount = oneToken();
    const claimantAta = getAssociatedTokenAddressSync(mint, claimant.publicKey);

    // 1. Signature signed for `otherUser` presented by `claimant` -> 6004 = InvalidSignedMessage
    const wrongRecipientSigIx = buildClaimSignatureIx(
      signer,
      otherUser.publicKey,
      amount,
      program.programId,
    );
    const claimIx = await program.methods
      .claimWithSignature(amount)
      .accountsPartial({
        claim: claimPda(claimant.publicKey),
        claimant: claimant.publicKey,
        claimantAta,
        mint,
        vault,
        instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .instruction();
    await expectReverted(
      { revertedWith: { message: numToHex(6004) } },
      sendIxs(context, [claimant], wrongRecipientSigIx, claimIx),
    );

    // 2. Signature signed by an untrusted keypair -> 6003 = InvalidSigner
    const fakeSigner = Keypair.generate();
    const fakeSignerSigIx = buildClaimSignatureIx(
      fakeSigner,
      claimant.publicKey,
      amount,
      program.programId,
    );
    await expectReverted(
      { revertedWith: { message: numToHex(6003) } },
      sendIxs(context, [claimant], fakeSignerSigIx, claimIx),
    );

    // 3. Signature signed for a different deployment (programId) -> 6004 = InvalidSignedMessage
    const otherProgramId = Keypair.generate().publicKey;
    const wrongProgramSigIx = buildClaimSignatureIx(
      signer,
      claimant.publicKey,
      amount,
      otherProgramId,
    );
    await expectReverted(
      { revertedWith: { message: numToHex(6004) } },
      sendIxs(context, [claimant], wrongProgramSigIx, claimIx),
    );

    // 4. Missing Ed25519 instruction -> 6002 = InvalidSignatureInstruction
    await expectReverted(
      { revertedWith: { message: numToHex(6002) } },
      sendIxs(context, [claimant], claimIx),
    );
  });
});
