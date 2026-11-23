import { createHash } from "crypto";
import { BN } from "@coral-xyz/anchor";
import { PublicKey } from "@solana/web3.js";

export interface AirdropEntry {
  account: PublicKey;
  amount: BN;
}

const sha256 = (data: Buffer): Buffer =>
  createHash("sha256").update(data).digest();

export const hashLeaf = (entry: AirdropEntry): Buffer => {
  const amountLe = entry.amount.toArrayLike(Buffer, "le", 8);
  return sha256(
    Buffer.concat([Buffer.from([0x00]), entry.account.toBuffer(), amountLe]),
  );
};

const hashPair = (a: Buffer, b: Buffer): Buffer => {
  const [left, right] = Buffer.compare(a, b) <= 0 ? [a, b] : [b, a];
  return sha256(Buffer.concat([Buffer.from([0x01]), left, right]));
};

export class MerkleTree {
  private layers: Buffer[][] = [];

  constructor(entries: AirdropEntry[]) {
    if (entries.length === 0) {
      throw new Error("MerkleTree requires at least one entry");
    }

    let currentLayer = entries.map(hashLeaf);
    this.layers.push(currentLayer);

    while (currentLayer.length > 1) {
      const nextLayer: Buffer[] = [];
      for (let i = 0; i < currentLayer.length; i += 2) {
        if (i + 1 < currentLayer.length) {
          nextLayer.push(hashPair(currentLayer[i], currentLayer[i + 1]));
        } else {
          nextLayer.push(currentLayer[i]);
        }
      }
      this.layers.push(nextLayer);
      currentLayer = nextLayer;
    }
  }

  get root(): Buffer {
    return this.layers[this.layers.length - 1][0];
  }

  getProof(index: number): Buffer[] {
    if (index < 0 || index >= this.layers[0].length) {
      throw new Error(`Leaf index ${index} out of bounds`);
    }

    const proof: Buffer[] = [];
    let idx = index;

    for (let level = 0; level < this.layers.length - 1; level++) {
      const layer = this.layers[level];
      const isRightNode = idx % 2 === 1;
      const siblingIdx = isRightNode ? idx - 1 : idx + 1;

      if (siblingIdx < layer.length) {
        proof.push(layer[siblingIdx]);
      }

      idx = Math.floor(idx / 2);
    }

    return proof;
  }
}
