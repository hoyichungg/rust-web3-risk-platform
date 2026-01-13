import { ethers } from "hardhat";

async function main() {
  const [deployer] = await ethers.getSigners();

  const Token = await ethers.getContractFactory("MockERC20");
  const supply = ethers.parseUnits("1000000", 18);

  const token = await Token.deploy("Test Token", "TEST", 18, supply);
  await token.waitForDeployment();

  console.log("Deployer:", await deployer.getAddress());
  console.log("Token address:", await token.getAddress());
}

main().catch((err) => {
  console.error(err);
  process.exitCode = 1;
});
