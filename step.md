下一步建議 (優先級排序)
🔥 立即執行 (本週)
  ✅ 執行 start-all.sh 啟動系統
  ✅ 完成 VERIFICATION.md 所有檢查
  ✅ 測試所有功能是否正常運作
  ✅ 調整告警閾值到合理範圍

📈 短期優化 (2-4 週)
  連接真實 RPC (Alchemy/Infura)
  增加更多 ERC20 Token 支持
  優化前端 UI/UX
  增加更多策略類型 (RSI/MACD)
  設定監控與日誌收集

🚀 中期擴展 (1-3 個月)
  支援更多鏈 (Polygon/Arbitrum/Optimism)
  增加清算風險預警
  支援 DeFi 協議 (Uniswap/Aave)
  增加告警通知 (Telegram/Discord/Email)
  移動端 (PWA 或 React Native)

💎 長期規劃 (3-6 個月)
  AI 驅動的風險預測
  社群功能 (策略分享)
  付費訂閱方案
  API 開放平台
  DAO 治理

---

連接真實 RPC (Alchemy/Infura) 落地步驟
- 申請 Alchemy 或 Infura 的 API Key，先決定要跑的鏈（Mainnet / Sepolia / Polygon 等）。
- 更新 `backend/.env`：`RPC_URL` 指向對應鏈的 HTTPS 端點，`CHAIN_RPC_URLS` / `CHAIN_WS_URLS` 依 chain_id 列出；付費端點建議 `PORTFOLIO_WS_TRIGGER=false`、`PORTFOLIO_SIMULATION=false`。
- Token 配置改成該鏈真實地址（主網示例可用 `.env.production.example` 的 USDC/DAI/WBTC），`ROLE_MANAGER_ADDRESS` 要填部署在同鏈的 RoleManager。
- 驗證 RPC：`curl -s -X POST "$RPC_URL" -H "Content-Type: application/json" --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'` 能回區塊號即 OK，重啟 backend 後觀察 `price refresh` / `portfolio snapshot updated`。
