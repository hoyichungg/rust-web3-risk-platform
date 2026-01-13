import { ethers } from "hardhat";

async function main() {
  const RoleManager = await ethers.getContractFactory("RoleManager");
  const roleManager = await RoleManager.deploy();
  await roleManager.waitForDeployment();
  const address: string = await (roleManager as any).getAddress();
  console.log(`RoleManager deployed to: ${address}`);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
