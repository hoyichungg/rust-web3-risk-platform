import "@nomicfoundation/hardhat-toolbox";
import { HardhatUserConfig } from "hardhat/config";

const config: HardhatUserConfig = {
  solidity: "0.8.24",
  networks: {
    localhost: {
      url: "http://127.0.0.1:8545",
    },
  },
  typechain: {
    target: "ethers-v6", // 🔥 加這行！（關鍵）
  },
};

export default config;