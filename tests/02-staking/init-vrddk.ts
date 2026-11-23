import { Keypair, PublicKey, SystemProgram, Transaction } from "@solana/web3.js";
import {
  ExtensionType,
  TOKEN_2022_PROGRAM_ID,
  createInitializeMintInstruction,
  createInitializeNonTransferableMintInstruction,
  getMintLen,
} from "@solana/spl-token";
import { ProgramTestContext } from "solana-bankrun";

import { processTransaction } from "../helpers";

/**
 * Create the vRDDK Token-2022 mint with the NonTransferable extension and
 * mint authority = `mintAuthority`.
 */
export const createVrddkMint = async (
  context: ProgramTestContext,
  payer: Keypair,
  mintAuthority: PublicKey,
  decimals: number,
): Promise<PublicKey> => {
  const mint = Keypair.generate();
  const extensions = [ExtensionType.NonTransferable];
  const mintLen = getMintLen(extensions);

  const rent = await context.banksClient.getRent();
  const lamports = Number(rent.minimumBalance(BigInt(mintLen)));

  await processTransaction(
    context,
    new Transaction().add(
      SystemProgram.createAccount({
        fromPubkey: payer.publicKey,
        newAccountPubkey: mint.publicKey,
        space: mintLen,
        lamports,
        programId: TOKEN_2022_PROGRAM_ID,
      }),
      createInitializeNonTransferableMintInstruction(
        mint.publicKey,
        TOKEN_2022_PROGRAM_ID,
      ),
      createInitializeMintInstruction(
        mint.publicKey,
        decimals,
        mintAuthority,
        null,
        TOKEN_2022_PROGRAM_ID,
      ),
    ),
    [payer, mint],
  );

  return mint.publicKey;
};
