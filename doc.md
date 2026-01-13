
## 本地開發流程
- Terminal 1（鏈）：`cd contracts && pnpm run node`，保持常駐。
- Terminal 2（部署）：`cd contracts && pnpm run deploy:local`，複製輸出的 `ROLE_MANAGER_ADDRESS` 到 `backend/.env`。
- Terminal 3（DB）：`docker compose up db -d`。
- Terminal 4（Seed，可選）：`cd backend && cargo run -p api --bin dev_seed`，寫入示範使用者/錢包/資產/告警/價格歷史。
- Terminal 5（後端）：`cd backend && DATABASE_URL=... cargo run -p api`。
- Terminal 6（前端）：`cd frontend && pnpm dev`。

## Hardhat / 合約
- 啟鏈：`cd contracts && pnpm run node`
- 部署 RoleManager：`cd contracts && pnpm run deploy:local`
- 改角色（Hardhat console）：
  ```
  const [owner] = await ethers.getSigners();
  const roleMgr = await ethers.getContractAt("RoleManager", "<ROLE_MANAGER_ADDRESS>");
  await roleMgr.connect(owner).setRole("<wallet>", 1); // 1=admin, 2=viewer, 0=remove
  ```
- 指令版改角：`cd contracts && ROLE_MANAGER_ADDRESS=0x... TARGET_ADDRESS=0x... ROLE=1 pnpm role:set`
- 測試：`cd contracts && pnpm test`（包含權限失敗案例）。

## Seed / 假資料
- DB 與鏈啟好後，`cd backend && cargo run -p api --bin dev_seed`
  - 預設錢包 `0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266`、Chain 31337，可用 `DEV_SEED_WALLET_ADDRESS` / `DEV_SEED_CHAIN_ID` 覆寫。
  - 會清掉同錢包既有 seed，再重建快照/交易/告警/價格歷史。

## 角色快取 / Session 管理
- 後端 Admin API：
  - 列出 sessions：`GET /api/admin/sessions`
  - 撤銷：`POST /api/admin/sessions/{id}/revoke`
  - 刷新所有錢包角色：`POST /api/admin/roles/refresh`
- CLI 工具：`cd backend && cargo run -p api --bin admin_tools -- session-list|session-revoke <id>|roles-refresh`

## 登入與角色
1. Hardhat 部署者默認是 Admin。若要讓自己登入的錢包有權限，執行 `pnpm role:set`（見上方）。
2. `.env` 需填 `ROLE_MANAGER_ADDRESS`、`RPC_URL=http://localhost:8545`，後端重啟後前端再登入。
3. 如角色查詢失敗，檢查後端 log 是否有 `role lookup failed`。

## 連接真實 RPC (Alchemy/Infura)
1. 申請 RPC Key：Alchemy（Dashboard 建立 App）或 Infura（Create API Key），選擇要跑的鏈（Mainnet/Sepolia/Polygon 等）。
2. 更新 `backend/.env`（建議從 `.env.production.example` 複製）：
   ```bash
   # 選一個提供商
   RPC_URL=https://eth-mainnet.g.alchemy.com/v2/<ALCHEMY_KEY>
   # RPC_URL=https://mainnet.infura.io/v3/<INFURA_KEY>

   # 如果要多鏈同步，按 chain_id 列出
   CHAIN_RPC_URLS=1=https://eth-mainnet.g.alchemy.com/v2/<ALCHEMY_KEY>,137=https://polygon-mainnet.g.alchemy.com/v2/<ALCHEMY_KEY>
   # CHAIN_RPC_URLS=1=https://mainnet.infura.io/v3/<INFURA_KEY>,137=https://polygon-mainnet.infura.io/v3/<INFURA_KEY>

   # 需要 WebSocket 觸發時再開
   CHAIN_WS_URLS=1=wss://eth-mainnet.g.alchemy.com/v2/<ALCHEMY_KEY>
   # CHAIN_WS_URLS=1=wss://mainnet.infura.io/ws/v3/<INFURA_KEY>
   PORTFOLIO_WS_TRIGGER=false   # 付費 RPC 建議先關掉 WS 觸發
   PORTFOLIO_SIMULATION=false   # 用真實資產時務必關掉模擬
   ```
   - `chain_id` 要與你的錢包鏈別一致（例如主網=1、Polygon=137、Sepolia=11155111）。
3. Token 與角色：
   - `ERC20_TOKENS` 請換成該鏈常用 Token（`.env.production.example` 已列主網 USDC/DAI/WBTC 範例）。
   - `ROLE_MANAGER_ADDRESS` 需是你在目標鏈部署的 RoleManager（沒有的話需先部署，或在測試網跑）。
4. 驗證 RPC 正常：
   ```bash
   curl -s -X POST "$RPC_URL" \
     -H "Content-Type: application/json" \
     --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'
   ```
   能回傳區塊號即代表連線 OK。修改完 .env 後重新啟動 backend，再觀察 log 中的 `price refresh` / `portfolio snapshot updated` 是否正常。

## 策略 / 回測
- 建立策略：`POST /api/strategies`（type: `ma_cross`/`volatility`/`correlation`，參數對應 short/long/lag 等）。
- 回測：`POST /api/strategies/{id}/backtest`，帶 `symbol`/`days`，會先讀 `price_history`，不足時抓 Coingecko，再落盤；失敗時會用合成價格避免 502。
- 查看結果：`GET /api/strategies/{id}/backtests?limit=5`
- 前端 `/strategies` 可匯入 CSV、自動抓價、查看回測歷史與 Equity Curve。

## 告警系統
- 建立/更新規則：`/api/alerts` 支援 `tvl_drop_pct`、`exposure_pct`、`net_outflow_pct`、`approval_spike`、`tvl_below`，可設定 `cooldown_secs`。
- 模擬觸發：`POST /api/alerts/{id}/test`
- 前端 `/alerts` 可完整 CRUD、模擬、顯示觸發歷史。
- Alert worker：`ENABLE_ALERT_WORKER=true` 時 API 會啟動；也可 `cargo run -p api --bin alert_worker` 獨立跑。

## 資產同步與價格
- Portfolio 同步預設 15 分鐘最小間隔，寫入 `portfolio_snapshots` / `portfolio_daily_snapshots` / `wallet_transactions`。
- 價格：`price_cache` 每 60s 取價（Coingecko → 靜態價格備援），`price_history` 帶 chain_id 落盤。
- 取得快照：`GET /api/portfolio/{wallet_id}/snapshots?days=7`，前端 Dashboard 圖表已使用。

## 常用查詢（SQL）
- 檢查快照：`SELECT wallet_id,total_usd_value,snapshot_time FROM portfolio_snapshots ORDER BY snapshot_time DESC LIMIT 20;`
- 檢查價格快取：`SELECT * FROM price_cache ORDER BY updated_at DESC;`
- 檢查角色快取：`SELECT address,role_cache,role_cache_updated_at FROM wallets;`
# 5. 執行 seed 資料
# 之後只需手動啟動 backend 和 frontend

# 如果是第一次執行:
cp backend/.env.minimal backend/.env  # 複製最小配置


// 📋 完整驗證流程
# 詳見 VERIFICATION.md
# 包含所有功能的檢查清單與預期結果


// 🎯 系統架構說明
# 詳見 DEPLOYMENT.md
# 包含完整的部署步驟、環境變數說明、問題排查
