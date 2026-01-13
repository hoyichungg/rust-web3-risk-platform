"use client";

import { UserWallet } from "../../lib/auth-context";

type WalletListProps = {
  wallets: UserWallet[];
};

export function WalletList({ wallets }: WalletListProps) {
  if (wallets.length === 0) {
    return <p>尚未綁定任何錢包。</p>;
  }

  return (
    <section
      style={{
        border: "1px solid #eee",
        padding: 16,
        borderRadius: 8,
        marginBottom: 24,
      }}
    >
      <h3 style={{ marginTop: 0 }}>綁定錢包</h3>
      <ul>
        {wallets.map((wallet) => (
          <li key={wallet.id}>
            {wallet.address} （Chain ID: {wallet.chain_id}）
          </li>
        ))}
      </ul>
    </section>
  );
}
