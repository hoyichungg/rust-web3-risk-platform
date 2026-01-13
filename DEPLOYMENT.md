# 🚀 完整部署與測試指南

## 前置準備

### 安裝依賴
```bash
# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Node.js & pnpm
brew install node
npm install -g pnpm

# Docker
brew install --cask docker

# PostgreSQL 客戶端工具
brew install postgresql
```

### 環境變數配置
```bash
# Backend
cd backend
cp .env.production.example .env
# 編輯 .env 填入真實配置

# Frontend  
cd frontend
cp .env.example .env.local
# 設定 NEXT_PUBLIC_BACKEND_URL=http://localhost:8081
```

---

## 🎯 本地開發環境 (6 個 Terminal)

### Terminal 1: 資料庫
```bash
docker compose up db -d
# 確認運行: psql postgresql://postgres:postgres@localhost:5432/postgres -c "SELECT 1;"
```

### Terminal 2: Hardhat 節點
```bash
cd contracts
pnpm install
pnpm run node  # 保持運行,不要關閉
```

### Terminal 3: 部署合約
```bash
cd contracts
pnpm run deploy:local

# 輸出範例:
# RoleManager deployed to: 0x5FbDB2315678afecb367f032d93F642f64180aa3
# 複製地址到 backend/.env 的 ROLE_MANAGER_ADDRESS
```

### Terminal 4: 設定角色
```bash
cd contracts

# 設定測試帳號為 Admin
ROLE_MANAGER_ADDRESS=0x5FbDB2315678afecb367f032d93F642f64180aa3 \
TARGET_ADDRESS=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 \
ROLE=1 \
pnpm role:set

# 驗證角色
pnpm hardhat console --network localhost
> const mgr = await ethers.getContractAt("RoleManager", "0x5FbDB...")
> await mgr.getRole("0xf39...")
> // 應返回 1 (Admin)
```

### Terminal 5: Backend
```bash
cd backend

# 執行 migration + seed 資料
cargo run -p api --bin dev_seed

# 啟動 API (會自動啟動 Indexer + Alert Worker)
ENABLE_ALERT_WORKER=true \
PORTFOLIO_SIMULATION=true \
RUST_LOG=info,api=debug \
cargo run -p api

# 等待看到:
# ✓ listening on address=0.0.0.0:8081
# ✓ portfolio snapshot updated
# ✓ price refresh
```

### Terminal 6: Frontend
```bash
cd frontend
pnpm install
pnpm dev

# 開啟 http://localhost:3000
```

---

## ✅ 功能驗證清單

### 1. 認證與角色
- [ ] 前端登入 (SIWE)
- [ ] Dashboard 顯示正確角色 (Admin/Viewer)
- [ ] Admin 能看到管理選單
- [ ] Session 列表可撤銷

### 2. 資產索引
- [ ] Dashboard 顯示錢包餘額
- [ ] 數字為真實價格 (非靜態 1.0)
- [ ] 15 分鐘後自動更新快照
- [ ] 歷史曲線有數據點

**測試命令:**
```sql
-- 檢查最新快照
SELECT wallet_id, total_usd_value, snapshot_time 
FROM portfolio_snapshots 
ORDER BY snapshot_time DESC LIMIT 5;

-- 檢查價格快取
SELECT symbol, price_usd, updated_at 
FROM price_cache 
ORDER BY updated_at DESC;
```

### 3. 告警系統
- [ ] 建立告警規則 (前端 /alerts)
- [ ] 60 秒後檢查觸發列表
- [ ] 冷卻期內不重複觸發
- [ ] Dashboard 顯示最近告警

**測試 API:**
```bash
# 建立 TVL 下跌告警
curl -X POST http://localhost:8081/api/alerts \
  -H "Content-Type: application/json" \
  -b "rw3p_token=..." \
  -d '{
    "type": "tvl_drop_pct",
    "threshold": 5.0,
    "enabled": true,
    "cooldown_secs": 300
  }'

# 查看觸發歷史
curl http://localhost:8081/api/alerts/triggers \
  -b "rw3p_token=..."
```

### 4. 策略回測
- [ ] 建立策略 (前端 /strategies)
- [ ] 執行回測 (30天 ETH 資料)
- [ ] 查看 equity curve
- [ ] price_history 表有資料

**測試 API:**
```bash
# 建立 MA 策略
STRATEGY_ID=$(curl -X POST http://localhost:8081/api/strategies \
  -H "Content-Type: application/json" \
  -b "rw3p_token=..." \
  -d '{
    "name": "ETH MA 5/20",
    "type": "ma_cross",
    "params": {"short_window": 5, "long_window": 20}
  }' | jq -r '.id')

# 執行回測
curl -X POST "http://localhost:8081/api/strategies/${STRATEGY_ID}/backtest" \
  -H "Content-Type: application/json" \
  -b "rw3p_token=..." \
  -d '{
    "symbol": "ETH",
    "days": 30
  }'

# 檢查歷史價格
psql $DATABASE_URL -c \
  "SELECT COUNT(*), MIN(price_ts), MAX(price_ts) 
   FROM price_history 
   WHERE symbol = 'ETH';"
```

### 5. 價格系統
- [ ] CoinGecko API 正常運作
- [ ] Fallback 到靜態價格
- [ ] 60 秒自動刷新
- [ ] Recording 寫入 price_history

**模擬失敗測試:**
```bash
# 暫時修改 .env
COINGECKO_API_BASE=https://invalid-url.com

# 重啟 backend
# 應該看到 fallback 到 TOKEN_PRICES

# 還原正確 URL 並重啟
```

---

## 🐛 常見問題排查

### 問題 1: 價格全是 1.0
**原因**: CoinGecko API 未正常運作或未配置 TOKEN_PRICE_IDS

**解決:**
```bash
# 檢查 backend log
# 應該看到 "price refresh" 訊息

# 檢查 price_cache 表
SELECT * FROM price_cache ORDER BY updated_at DESC;

# 如果為空,檢查:
# 1. COINGECKO_API_BASE 是否正確
# 2. 網路連線是否正常
# 3. TOKEN_PRICE_IDS 是否配置
```

### 問題 2: 告警不觸發
**原因**: ENABLE_ALERT_WORKER=false 或沒有符合條件的資料

**解決:**
```bash
# 確認環境變數
echo $ENABLE_ALERT_WORKER  # 應為 true

# 檢查 backend log
# 應該每 60 秒看到 "alert evaluator" 相關訊息

# 檢查是否有足夠的快照資料
SELECT COUNT(*) FROM portfolio_snapshots;  # 至少 2 筆

# 手動測試觸發
curl -X POST http://localhost:8081/api/alerts/{alert_id}/test \
  -b "rw3p_token=..."
```

### 問題 3: Dashboard 無資產數據
**原因**: 索引器尚未運行或錢包地址無餘額

**解決:**
```bash
# 檢查索引器日誌
SELECT * FROM indexer_runs ORDER BY started_at DESC LIMIT 5;

# 如果 status = 'error',查看 error 欄位

# 確認 RPC_URL 正確且網路通暢
curl -X POST $RPC_URL \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'

# 如果是測試環境,啟用模擬資產
PORTFOLIO_SIMULATION=true cargo run -p api
```

### 問題 4: 回測無歷史價格
**原因**: 首次執行尚未抓取歷史資料

**解決:**
```bash
# 手動觸發回測,會自動抓取
curl -X POST http://localhost:8081/api/strategies/{id}/backtest \
  -H "Content-Type: application/json" \
  -b "rw3p_token=..." \
  -d '{"symbol": "ETH", "days": 30}'

# 檢查是否成功寫入
SELECT COUNT(*) FROM price_history WHERE symbol = 'ETH';

# 如果失敗,檢查 CoinGecko API
curl "https://api.coingecko.com/api/v3/coins/ethereum/market_chart?vs_currency=usd&days=30&interval=hourly"
```

---

## 📊 效能監控

### 關鍵指標
```sql
-- Portfolio 同步頻率
SELECT 
  DATE_TRUNC('hour', started_at) as hour,
  COUNT(*) as sync_count,
  COUNT(CASE WHEN status = 'error' THEN 1 END) as errors
FROM indexer_runs
WHERE started_at > NOW() - INTERVAL '24 hours'
GROUP BY hour
ORDER BY hour DESC;

-- 價格更新延遲
SELECT 
  symbol,
  price_usd,
  NOW() - updated_at as age
FROM price_cache
ORDER BY updated_at DESC;

-- 告警觸發統計
SELECT 
  DATE(created_at) as day,
  COUNT(*) as trigger_count
FROM alert_triggers
WHERE created_at > NOW() - INTERVAL '7 days'
GROUP BY day
ORDER BY day DESC;

-- 回測執行統計
SELECT 
  DATE(started_at) as day,
  COUNT(*) as backtest_count,
  AVG(EXTRACT(EPOCH FROM (completed_at - started_at))) as avg_duration_secs
FROM strategy_backtests
WHERE started_at > NOW() - INTERVAL '30 days'
GROUP BY day
ORDER BY day DESC;
```

---

## 🚀 正式環境部署

### Docker Compose 部署
```bash
# 編輯 docker-compose.yml 確認所有服務
# 編輯 backend/.env.production

docker compose up -d

# 查看日誌
docker compose logs -f api

# 執行健康檢查
./scripts/verify-deployment.sh
```

### 環境變數檢查清單
- [ ] `DATABASE_URL` 指向正式資料庫
- [ ] `JWT_SECRET` 為強隨機字串 (>32 字元)
- [ ] `COOKIE_SECURE=true`
- [ ] `FRONTEND_ORIGINS` 包含正式域名
- [ ] `RPC_URL` 使用付費方案避免限制
- [ ] `COINGECKO_API_BASE` 考慮付費方案
- [ ] `ENABLE_ALERT_WORKER=true`
- [ ] `PORTFOLIO_SIMULATION=false`

### 監控與告警
```bash
# 設定 Prometheus + Grafana
# 監控指標:
# - API 回應時間
# - 索引器成功率
# - 價格刷新延遲
# - 告警觸發頻率
# - 資料庫連線池狀態
```

---

## 📚 延伸閱讀

- [SIWE 規範](https://eips.ethereum.org/EIPS/eip-4361)
- [CoinGecko API 文檔](https://www.coingecko.com/en/api/documentation)
- [Ethers.rs 文檔](https://docs.rs/ethers/latest/ethers/)
- [Axum Web 框架](https://docs.rs/axum/latest/axum/)
