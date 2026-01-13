import { expect } from "chai";
import { ethers } from "hardhat";

describe("RoleManager", function () {
  it("assigns admin role to deployer and updates roles", async function () {
    const [owner, other] = await ethers.getSigners();
    const RoleManager = await ethers.getContractFactory("RoleManager");
    const contract = await RoleManager.deploy();
    await contract.waitForDeployment();

    expect(await contract.owner()).to.equal(owner.address);
    expect(await contract.getRole(owner.address)).to.equal(1n);

    await (await contract.setRole(other.address, 2)).wait();
    expect(await contract.getRole(other.address)).to.equal(2n);
  });

  it("blocks non-admin role changes", async function () {
    const [owner, attacker, user] = await ethers.getSigners();
    const RoleManager = await ethers.getContractFactory("RoleManager");
    const contract = await RoleManager.deploy();
    await contract.waitForDeployment();

    await expect(
      contract.connect(attacker).setRole(user.address, 1),
    ).to.be.revertedWith("RoleManager: insufficient permissions");
  });

  it("prevents zero address role assignment", async function () {
    const RoleManager = await ethers.getContractFactory("RoleManager");
    const contract = await RoleManager.deploy();
    await contract.waitForDeployment();

    await expect(
      contract.setRole(ethers.ZeroAddress, 1),
    ).to.be.revertedWith("RoleManager: zero user");
  });

  it("transferOwnership promotes new owner to admin", async function () {
    const [owner, newOwner, user] = await ethers.getSigners();
    const RoleManager = await ethers.getContractFactory("RoleManager");
    const contract = await RoleManager.deploy();
    await contract.waitForDeployment();

    await (await contract.transferOwnership(newOwner.address)).wait();
    expect(await contract.owner()).to.equal(newOwner.address);
    expect(await contract.getRole(newOwner.address)).to.equal(1n);

    // Old owner loses owner power but stays admin; admin can set role.
    await (await contract.connect(newOwner).setRole(user.address, 2)).wait();
    expect(await contract.getRole(user.address)).to.equal(2n);
  });

  it("renounceOwnership clears owner but keeps roles", async function () {
    const [owner, other] = await ethers.getSigners();
    const RoleManager = await ethers.getContractFactory("RoleManager");
    const contract = await RoleManager.deploy();
    await contract.waitForDeployment();

    await (await contract.setRole(other.address, 1)).wait();
    await (await contract.renounceOwnership()).wait();
    expect(await contract.owner()).to.equal(ethers.ZeroAddress);
    // Admin still exists even after renounce.
    expect(await contract.getRole(other.address)).to.equal(1n);
  });
});
