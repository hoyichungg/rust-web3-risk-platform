# Rust Web3 Risk Platform

此專案提供「多鏈資產風險儀表板＋策略回測／告警」的骨架，採用 Rust Axum 後端、Hardhat/Foundry 合約與 Next.js 前端。

## 結構
- `backend/`: Rust workspace，包含 API、auth、策略引擎、indexer、alert 引擎等 crate。
- `contracts/`: Hardhat 專案（RoleManager.sol、MockERC20.sol、部署/改角腳本、測試）。
- `frontend/`: Next.js App Router（Dashboard/Alerts/Strategies/Admin 頁）。
- `infra/`: docker-compose（Postgres、Redis、Anvil/Hardhat）。
- `doc.md`: 開發/部署/測試流程備忘。

## 本地啟動（建議流程）
1. 啟 Hardhat 節點：`cd contracts && pnpm run node`（保持常駐）。
2. 部署 RoleManager：`cd contracts && pnpm run deploy:local`，把輸出的 `ROLE_MANAGER_ADDRESS` 寫到 `backend/.env`。
3. 起 DB：`docker compose up db -d`（預設 6543 對應本機）。
4. Seed 假資料（可選）：`cd backend && cargo run -p api --bin dev_seed`（示範錢包/資產/告警/價格）。
5. 後端：`cd backend && DATABASE_URL=... cargo run -p api`。
6. 前端：`cd frontend && pnpm dev`。

（也可用根目錄 `Makefile`：`make chain` / `make deploy` / `make backend` / `make frontend` / `make seed`）

## Docker 一鍵部署 (Production)
若要模擬正式環境或進行部署，可使用新增的 `docker-compose.prod.yml`：

```bash
docker compose -f docker-compose.prod.yml up -d --build
```
這將會啟動 Postgres, Redis, Rust Backend (Port 8080) 與 Next.js Frontend (Port 3000)。

## 主要環境變數
   - `DATABASE_URL`、`RPC_URL`、`ROLE_MANAGER_ADDRESS`
   - `JWT_SECRET`、`SIWE_DOMAIN`、`SIWE_URI`、`SIWE_STATEMENT`
   - `FRONTEND_ORIGIN` / `FRONTEND_ORIGINS`（CORS 允許來源，多值以逗號分隔）
   - `COOKIE_SECURE`（httpOnly cookie 是否加上 secure flag）、`COOKIE_SAMESITE`（Lax/Strict/None）
   - `JWT_AUDIENCE`、`JWT_ISSUER`（JWT 驗證的 aud/iss），預設 `rw3p` / `rw3p-api`
   - `REDIS_URL`（可選，用來存 nonce throttle；未設定則退回記憶體版）
   - `ACCESS_TOKEN_TTL_SECS` / `REFRESH_TOKEN_TTL_SECS`、`NONCE_THROTTLE_SECONDS`
   - 投組索引器：`PORTFOLIO_SYNC_INTERVAL_SECS`（預設 900，15 分鐘）、`PORTFOLIO_MAX_CONCURRENCY`（預設 4）、`PORTFOLIO_SYNC_RETRIES`（預設 3）
   - 告警 worker：`ENABLE_ALERT_WORKER`（預設 true，若要獨立運行 alert worker 可在 API server 設為 false，另外跑 `cargo run -p api --bin alert_worker`）
   - 管理工具：`cargo run -p api --bin admin_tools -- session-list|session-revoke <id>|roles-refresh`
   - 多鏈 RPC：`RPC_URL` 為預設值，可用 `CHAIN_RPC_URLS` 以逗號列出 `chain_id=url`（例 `1=https://...,137=https://...`）；`CHAIN_WS_URLS` 可選、搭配 `PORTFOLIO_WS_TRIGGER=true` 啟動 newHeads 推播即時同步
   - 角色快取 TTL：`ROLE_CACHE_TTL_SECS`（預設值），`ROLE_CACHE_TTL_OVERRIDES` 支援逗號分隔的 `<chain>=<秒>`（例如 `1=600,137=300`）
   - Token 與價格：`ERC20_TOKENS` 以 `SYMBOL:ADDRESS:DECIMALS:CHAIN_ID` 逗號分隔，`TOKEN_PRICES` 以 `SYMBOL=價格` 逗號分隔（作為靜態報價）
   - 動態報價：`COINGECKO_API_BASE`（預設 `https://api.coingecko.com/api/v3`）、`TOKEN_PRICE_IDS`（`SYMBOL:coingecko-id`，未設定會用內建 mapping 或以 symbol 轉小寫查詢）、`PRICE_CACHE_TTL_SECS`（預設 60 秒，Coingecko 快取）
   - 參考 `.env.example` 直接複製一份調整。
   - **Production 推薦值**：`COOKIE_SECURE=true`、`FRONTEND_ORIGIN=https://<你的正式網域>`
4. 日誌：設定 `RUST_LOG=info` 會輸出 JSON 結構化 log，內建 `request_id`（可自帶 `X-Request-Id` header 追蹤）。

### Demo seed（本地假資料）
- 開啟 Postgres（`docker compose up db -d` 或自備 DB），確保 `.env` 的 `DATABASE_URL` 指向該庫。
- 按 `contracts/` 的流程啟動 Hardhat node 並 `pnpm run deploy:local`，把 `ROLE_MANAGER_ADDRESS` 寫入 `.env`。
- 在 `backend/` 執行 `cargo run -p api --bin dev_seed`，會跑 migrations 並插入示範使用者/錢包/資產/告警/價格歷史。
  - 預設錢包 `DEV_SEED_WALLET_ADDRESS=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266`、`DEV_SEED_CHAIN_ID=31337`，可自行覆寫。
  - 每次執行會清掉同錢包的舊 seed 資料，方便保持介面乾淨。

### 測試 / 品質檢查
- 後端整套檢查：`cd backend && DATABASE_URL=postgres://postgres:postgres@localhost:5432/postgres cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --all-targets`
- API 整合測試：`cd backend && DATABASE_URL=postgres://... cargo test -p api get_me_returns_profile -- --nocapture`（使用 sqlx 內建 Postgres 測試 DB ＋ Axum 路由）
- 前端：`cd frontend && pnpm lint`

### API 範例
- `GET /healthz`：健康檢查。
- `GET /auth/nonce`：取得 nonce。
- `POST /auth/login`：範例 body
  ```json
  {"message":"<完整 SIWE 訊息文字>","signature":"0x..."}
  ```
  需遵循 SIWE 標準訊息格式（前端預設會組出
  `Sign in to Rust Web3 Risk Platform` 的訊息，包含 Domain/URI/Chain ID/Nonce/Issued At）。成功時後端會設置 `rw3p_token`/`rw3p_role` httpOnly cookies，回傳 body 只包含角色資訊。
- `POST /auth/logout`：清除 session 並刪除 cookies。
- `GET /api/me`：回傳目前登入者在 DB 的 user + wallets 設定（包含 Role）。
- `POST /strategies`：建立策略。
- `POST /strategies/{id}/backtest`：
  ```json
  {"short_window":5,"long_window":20,"prices":[{"timestamp":"2024-01-01T00:00:00Z","price":100.0}]}
  ```
  若未提供 `prices` 則使用合成價格序列。
- `GET /alerts` / `POST /alerts`：管理告警規則。
- `GET /portfolio/{wallet_id}`：取得最新資產快照（示範資料）。
- `GET /portfolio/{wallet_id}/history?limit=50`：取得歷史快照（預設 50 筆，最多 500）。
- 錢包與主錢包：
  - `POST /wallets` 建立錢包。
  - `POST /wallets/:wallet_id/primary` 切換主錢包。
- 管理介面：
  - `GET /api/admin/users`：列出用戶＋綁定錢包與角色快取。
  - `GET /api/admin/sessions`：列出所有登入 session，支援 Admin 撤銷。
  - `POST /api/admin/sessions/{id}/revoke`：撤銷指定 session（包含已旋轉的 refresh）。
  - `POST /api/admin/roles/refresh`：強制重新查詢所有錢包的鏈上角色並更新快取。
- 策略 / 回測：
  - `GET /api/strategies`：列出當前使用者策略。
  - `POST /api/strategies`：建立策略（`name`/`type`/`params`）。
  - `POST /api/strategies/{id}/backtest`：跑 MA 交叉回測，接受 `prices`、`short_window`、`long_window`。結果會存入 `strategy_backtests`。
  - 告警：
    - `GET /api/alerts` / `POST /api/alerts` / `PUT /api/alerts/:id` / `DELETE /api/alerts/:id`：告警規則 CRUD。
    - `GET /api/alerts/triggers`：查看近期觸發。
    - 背景 Job：每 60s 檢查 `tvl_drop_pct` 規則，若任一錢包最新 TVL 較前一筆下跌超過 threshold% 則寫入觸發紀錄。

### OpenAPI 規格
- OpenAPI 3.1 檔案：`backend/api/openapi.yaml`（可直接匯入 Swagger UI/Postman）。
- 覆蓋的端點：auth（nonce/login/logout/refresh）、/api/me、wallets、strategies/backtest、portfolio 及 healthcheck；安全性採用 Bearer token 或登入後的 httpOnly cookies。

### CI
- `.github/workflows/ci.yml`：Backend 跑 cargo fmt/clippy/test（Postgres service）、Frontend 跑 pnpm lint。

## 系統架構完整度

### ✅ 已完成功能
- **身分認證**: SIWE + RoleManager 鏈上角色驗證 + Session 管理 + Refresh Token
- **資產索引**: 
  - 定期同步 (15分鐘) + WebSocket 即時觸發
  - 支援 ETH + ERC20 餘額查詢
  - 自動抓取 Transfer/Approval 交易記錄
  - Portfolio 歷史快照 (15分鐘粒度)
- **價格系統**:
  - CoinGecko API (主要) + 靜態配置 (fallback)
  - 三層架構: Cache (Postgres) → Recording (price_history) → Oracle
  - 自動刷新 (60秒) 避免 API 限制
- **告警引擎**:
  - 5 種規則: TVL下跌/單幣暴露/淨流出/Approval激增/TVL低於閾值
  - 冷卻機制避免重複觸發
  - 背景 Worker 每 60 秒評估
- **策略回測**:
  - 3 種策略: MA交叉/波動率/相關性
  - 自動從 CoinGecko 抓歷史價格
  - 支援自訂參數與價格序列
- **多鏈支持**: 可配置不同鏈的 RPC/WS 端點
- **前端 UI**: Next.js + MUI 完整實作 Dashboard/Alerts/Strategies/Admin 頁面

### 🎯 功能開關 (環境變數)
```bash
# 告警 Worker (預設關閉,建議開啟)
ENABLE_ALERT_WORKER=true

# WebSocket 即時同步 (預設關閉,可選)
PORTFOLIO_WS_TRIGGER=true
CHAIN_WS_URLS=1=wss://...

# 模擬資產 (開發測試用)
PORTFOLIO_SIMULATION=true
```

### 📊 資料表結構
- **users**: 使用者基本資訊
- **wallets**: 錢包列表 (支援多錢包)
- **portfolio_snapshots**: 資產歷史快照 (15分鐘粒度)
- **portfolio_daily**: 每日彙總快照
- **wallet_transactions**: ERC20 交易記錄
- **price_cache**: 價格快取 (60秒 TTL)
- **price_history**: 歷史價格 (回測用)
- **strategies**: 策略定義
- **strategy_backtests**: 回測結果
- **alert_rules**: 告警規則
- **alert_triggers**: 告警觸發歷史
- **sessions**: 登入 Session
- **indexer_runs**: 索引器運行日誌

## 後續優化方向
- 增加更多策略類型 (RSI/MACD/網格交易)
- 支援更多告警通知管道 (Telegram/Email/Webhook)
- 增加清算風險預警
- 支援更多 DeFi 協議 (Uniswap/Aave/Compound)
- 效能優化: 增加 Redis 快取層
- ✅ 部署: Docker Compose 一鍵部署方案
