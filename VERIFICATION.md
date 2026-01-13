# ✅ 功能驗證清單

執行完 `./start-all.sh` 後,依序檢查:

## 1. 基礎設施 (2分鐘)

```bash
# 資料庫
psql postgresql://postgres:postgres@localhost:5432/postgres -c "SELECT COUNT(*) FROM users;"
# 應該返回至少 1 (seed 資料)

# Hardhat 節點
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'
# 應該返回區塊號碼

# Backend API
curl http://localhost:8081/healthz
# 應該返回 OK

# Frontend
open http://localhost:3000
# 應該看到登入頁面
```

## 2. 認證與角色 (3分鐘)

```bash
# 前端登入
1. 連接錢包 (MetaMask/Coinbase Wallet)
2. 切換到 Localhost 8545
3. 簽署 SIWE 訊息
4. 應該進入 Dashboard

# 設定 Admin 角色 (如果需要)
cd contracts
ROLE_MANAGER_ADDRESS=0x5FbDB... \
TARGET_ADDRESS=0x你的錢包地址 \
ROLE=1 \
pnpm role:set
```

## 3. Dashboard 功能 (5分鐘)

### ✅ 資產顯示
- [ ] 看到錢包列表
- [ ] TVL 顯示真實數字 (非 1.0)
- [ ] 資產分佈圓餅圖
- [ ] 歷史曲線有數據點

### ✅ 價格系統
```sql
-- 檢查價格快取
psql $DATABASE_URL -c "SELECT * FROM price_cache ORDER BY updated_at DESC LIMIT 5;"

-- 檢查歷史價格
psql $DATABASE_URL -c "SELECT COUNT(*) FROM price_history;"
```

### ✅ 資產同步
```sql
-- 檢查快照
psql $DATABASE_URL -c "
  SELECT wallet_id, total_usd_value, snapshot_time 
  FROM portfolio_snapshots 
  ORDER BY snapshot_time DESC LIMIT 5;
"

-- 檢查索引器狀態
psql $DATABASE_URL -c "SELECT * FROM indexer_runs ORDER BY started_at DESC LIMIT 5;"
```

## 4. 告警系統 (5分鐘)

### 前端操作
1. 進入 `/alerts` 頁面
2. 建立規則: TVL 下跌 5%
3. 點擊「模擬觸發」
4. 60秒後檢查觸發列表

### 驗證
```sql
-- 檢查規則
psql $DATABASE_URL -c "SELECT * FROM alert_rules;"

-- 檢查觸發
psql $DATABASE_URL -c "SELECT * FROM alert_triggers ORDER BY created_at DESC LIMIT 5;"
```

### 後端日誌
```bash
# 應該看到
[INFO] alert triggered wallet=0x... rule=...
```

## 5. 策略回測 (5分鐘)

### 前端操作
1. 進入 `/strategies` 頁面
2. 建立策略: MA(5,20)
3. 選擇 ETH, 30 天
4. 點擊「回測」
5. 查看結果圖表

### 驗證
```sql
-- 檢查策略
psql $DATABASE_URL -c "SELECT * FROM strategies;"

-- 檢查回測結果
psql $DATABASE_URL -c "SELECT * FROM strategy_backtests ORDER BY started_at DESC LIMIT 3;"

-- 檢查歷史價格
psql $DATABASE_URL -c "
  SELECT symbol, COUNT(*), MIN(price_ts), MAX(price_ts) 
  FROM price_history 
  GROUP BY symbol;
"
```

### CSV 匯入測試
```bash
# 建立測試 CSV
cat > /tmp/test_prices.csv << EOF
2024-01-01T00:00:00Z,3000
2024-01-02T00:00:00Z,3100
2024-01-03T00:00:00Z,3050
2024-01-04T00:00:00Z,3200
EOF

# 在前端 Strategies 頁面匯入此 CSV
# 執行回測應該使用這些價格
```

## 6. 進階功能 (5分鐘)

### Admin 功能 (需要 Admin 角色)
- [ ] Session 管理 (`/admin/sessions`)
- [ ] 用戶列表 (`/admin/users`)
- [ ] 角色刷新 (Dashboard 按鈕)

### 交易記錄
```sql
-- 檢查是否抓到 Transfer 事件
psql $DATABASE_URL -c "SELECT * FROM wallet_transactions LIMIT 5;"
```

### WebSocket 即時同步 (可選)
```bash
# 編輯 backend/.env
PORTFOLIO_WS_TRIGGER=true
CHAIN_WS_URLS=1=wss://eth-mainnet.g.alchemy.com/v2/YOUR_KEY

# 重啟 backend
# 應該看到: ws subscribe 訊息
```

---

## 🐛 問題排查

### 問題: 價格全是 1.0
```bash
# 檢查 CoinGecko 連線
curl "https://api.coingecko.com/api/v3/simple/price?ids=ethereum&vs_currencies=usd"

# 檢查後端日誌
grep "price refresh" backend.log

# 如果失敗,會 fallback 到 TOKEN_PRICES
```

### 問題: 告警不觸發
```bash
# 確認 Worker 啟動
ps aux | grep "cargo run -p api"
env | grep ENABLE_ALERT_WORKER  # 應該是 true

# 檢查是否有足夠的快照資料
psql $DATABASE_URL -c "
  SELECT wallet_id, COUNT(*) 
  FROM portfolio_snapshots 
  GROUP BY wallet_id;
"  # 需要至少 2 筆
```

### 問題: Dashboard 無資料
```bash
# 檢查 seed 是否成功
psql $DATABASE_URL -c "SELECT COUNT(*) FROM users;"  # >0
psql $DATABASE_URL -c "SELECT COUNT(*) FROM wallets;"  # >0

# 手動觸發同步
# 等待 15 分鐘或重啟 backend
```

### 問題: Hardhat 連不上
```bash
# 檢查節點
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"net_version","params":[],"id":1}'

# 如果失敗,重啟
pkill -f "hardhat node"
cd contracts && pnpm run node &
```

---

## 📊 效能指標

正常運行時應該看到:

```bash
# Backend log (每分鐘)
[INFO] portfolio snapshot updated wallet_id=... usd_value=...
[INFO] price refresh completed symbols=5

# 資料庫大小
psql $DATABASE_URL -c "
  SELECT 
    schemaname,
    tablename,
    pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) AS size
  FROM pg_tables 
  WHERE schemaname = 'public' 
  ORDER BY pg_total_relation_size(schemaname||'.'||tablename) DESC 
  LIMIT 10;
"

# API 回應時間
curl -w "@-" -o /dev/null -s http://localhost:8081/healthz << EOF
    time_total:  %{time_total}s
EOF
# 應該 < 0.1s
```

---

## ✅ 驗證成功標準

- [ ] Dashboard 顯示真實價格 (ETH ~$3000)
- [ ] 資產歷史曲線有至少 2 個數據點
- [ ] 告警可建立、觸發、顯示在列表
- [ ] 策略回測可執行並顯示圖表
- [ ] price_history 表有資料
- [ ] 後端日誌無 ERROR (WARN 可以有)
- [ ] Admin 功能可用 (如果是 Admin 角色)

---

## 🎯 下一步

驗證完成後,可以:
1. 連接真實 RPC (Alchemy/Infura)
2. 部署到測試網 (Sepolia/Goerli)
3. 增加更多 ERC20 Token 配置
4. 調整告警閾值測試
5. 匯出回測結果為 JSON/CSV
6. 設定 CI/CD 自動部署

全部通過即為**生產就緒**! 🚀
