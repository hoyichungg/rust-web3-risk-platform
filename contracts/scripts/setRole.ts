import { ethers } from "hardhat";

async function main() {
  const contractAddress = process.env.ROLE_MANAGER_ADDRESS;
  const targetAddress = process.env.TARGET_ADDRESS;
  const roleValue = Number(process.env.ROLE ?? "2");

  if (!contractAddress) {
    throw new Error("ROLE_MANAGER_ADDRESS env is required");
  }
  if (!targetAddress) {
    throw new Error("TARGET_ADDRESS env is required");
  }
  if (![0, 1, 2].includes(roleValue)) {
    throw new Error("ROLE must be 0 (None), 1 (Admin), or 2 (Viewer)");
  }

  const roleManager = await ethers.getContractAt(
    "RoleManager",
    contractAddress,
  );
  const [owner] = await ethers.getSigners();
  const tx = await roleManager.connect(owner).setRole(targetAddress, roleValue);
  await tx.wait();
  console.log(
    `Role for ${targetAddress} updated to ${roleValue} on ${contractAddress}`,
  );
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
