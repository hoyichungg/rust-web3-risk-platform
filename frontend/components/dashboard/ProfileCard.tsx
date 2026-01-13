"use client";

import { UserProfile } from "../../lib/auth-context";

type ProfileCardProps = {
  profile: UserProfile;
};

export function ProfileCard({ profile }: ProfileCardProps) {
  return (
    <section
      style={{
        border: "1px solid #eee",
        padding: 16,
        borderRadius: 8,
        marginBottom: 24,
      }}
    >
      <h2 style={{ marginTop: 0 }}>基本資訊</h2>
      <p>使用者 ID：{profile.id}</p>
      <p>Primary Wallet：{profile.primary_wallet}</p>
      <p>角色：{profile.role}</p>
    </section>
  );
}
