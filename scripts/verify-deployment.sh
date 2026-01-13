#!/bin/bash
# 快速驗證所有功能是否正常運作

set -e

echo "開始測試 Rust Web3 Risk Platform..."

# 顏色輸出
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

API_BASE="${API_BASE:-http://localhost:8081}"

# 測試健康檢查
echo -e "\n${YELLOW}[1/6] 測試健康檢查...${NC}"
if curl -f -s "${API_BASE}/healthz" > /dev/null; then
    echo -e "${GREEN}✓ API 運行正常${NC}"
else
    echo -e "${RED}✗ API 無法連線${NC}"
    exit 1
fi

# 檢查資料庫連線
echo -e "\n${YELLOW}[2/6] 檢查資料庫...${NC}"
if psql "${DATABASE_URL}" -c "SELECT COUNT(*) FROM users;" > /dev/null 2>&1; then
    USER_COUNT=$(psql "${DATABASE_URL}" -t -c "SELECT COUNT(*) FROM users;")
    echo -e "${GREEN}✓ 資料庫連線正常 (${USER_COUNT} users)${NC}"
else
    echo -e "${RED}✗ 資料庫連線失敗${NC}"
    exit 1
fi

# 檢查價格快取
echo -e "\n${YELLOW}[3/6] 檢查價格系統...${NC}"
PRICE_COUNT=$(psql "${DATABASE_URL}" -t -c "SELECT COUNT(*) FROM price_cache WHERE updated_at > NOW() - INTERVAL '5 minutes';")
if [ "${PRICE_COUNT// /}" -gt 0 ]; then
    echo -e "${GREEN}✓ 價格系統運作中 (${PRICE_COUNT} 個最近價格)${NC}"
    psql "${DATABASE_URL}" -c "SELECT symbol, price_usd, updated_at FROM price_cache ORDER BY updated_at DESC LIMIT 5;"
else
    echo -e "${YELLOW}⚠ 價格快取為空，可能尚未開始刷新${NC}"
fi

# 檢查 Portfolio Snapshots
echo -e "\n${YELLOW}[4/6] 檢查資產快照...${NC}"
SNAPSHOT_COUNT=$(psql "${DATABASE_URL}" -t -c "SELECT COUNT(*) FROM portfolio_snapshots WHERE snapshot_time > NOW() - INTERVAL '1 hour';")
if [ "${SNAPSHOT_COUNT// /}" -gt 0 ]; then
    echo -e "${GREEN}✓ 資產索引運作中 (${SNAPSHOT_COUNT} 個近期快照)${NC}"
else
    echo -e "${YELLOW}⚠ 尚無最近快照，可能首次運行或無活躍錢包${NC}"
fi

# 檢查告警規則
echo -e "\n${YELLOW}[5/6] 檢查告警系統...${NC}"
ALERT_COUNT=$(psql "${DATABASE_URL}" -t -c "SELECT COUNT(*) FROM alert_rules;")
TRIGGER_COUNT=$(psql "${DATABASE_URL}" -t -c "SELECT COUNT(*) FROM alert_triggers;")
echo -e "${GREEN}✓ 告警規則: ${ALERT_COUNT}, 觸發次數: ${TRIGGER_COUNT}${NC}"

if [ "${ENABLE_ALERT_WORKER}" = "true" ]; then
    echo -e "${GREEN}✓ 告警 Worker 已啟用${NC}"
else
    echo -e "${YELLOW}⚠ 告警 Worker 未啟用 (ENABLE_ALERT_WORKER=false)${NC}"
fi

# 檢查索引器日誌
echo -e "\n${YELLOW}[6/6] 檢查索引器運行狀態...${NC}"
INDEXER_RUNS=$(psql "${DATABASE_URL}" -t -c "SELECT COUNT(*) FROM indexer_runs WHERE started_at > NOW() - INTERVAL '1 hour';")
if [ "${INDEXER_RUNS// /}" -gt 0 ]; then
    echo -e "${GREEN}✓ 索引器運作中 (${INDEXER_RUNS} 次近期運行)${NC}"
    psql "${DATABASE_URL}" -c "SELECT wallet_id, status, error, started_at FROM indexer_runs ORDER BY started_at DESC LIMIT 5;"
else
    echo -e "${YELLOW}⚠ 索引器尚未運行或無錢包需同步${NC}"
fi

echo -e "\n${GREEN}🎉 測試完成!${NC}"
echo ""
echo "建議檢查項目:"
echo "1. 前端 Dashboard 是否顯示資產數據"
echo "2. 價格是否為實時數據 (非靜態配置)"
echo "3. 告警規則是否能正確觸發"
echo "4. 策略回測是否能抓到歷史價格"
echo ""
echo "查看詳細日誌: docker compose logs -f api"
