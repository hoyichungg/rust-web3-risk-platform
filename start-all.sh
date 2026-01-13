#!/bin/bash
# 一鍵啟動完整系統

set -e

echo "🚀 啟動 Rust Web3 Risk Platform..."

# 顏色
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

# 檢查依賴
check_command() {
    if ! command -v $1 &> /dev/null; then
        echo -e "${RED}✗ $1 未安裝${NC}"
        exit 1
    fi
}

echo -e "\n${YELLOW}[1/7] 檢查依賴...${NC}"
check_command docker
check_command psql
check_command cargo
check_command pnpm
echo -e "${GREEN}✓ 所有依賴已就緒${NC}"

# 啟動資料庫
echo -e "\n${YELLOW}[2/7] 啟動 PostgreSQL...${NC}"
docker compose up db -d
sleep 3
echo -e "${GREEN}✓ 資料庫已啟動${NC}"

# 檢查 .env
echo -e "\n${YELLOW}[3/7] 檢查環境變數...${NC}"
if [ ! -f "backend/.env" ]; then
    echo -e "${RED}✗ backend/.env 不存在${NC}"
    echo "請執行: cp backend/.env.production.example backend/.env"
    exit 1
fi
echo -e "${GREEN}✓ .env 配置存在${NC}"

# 啟動 Hardhat (背景)
echo -e "\n${YELLOW}[4/7] 啟動 Hardhat 節點...${NC}"
cd contracts
pnpm install --silent 2>/dev/null || true
pkill -f "hardhat node" 2>/dev/null || true
pnpm run node > ../hardhat.log 2>&1 &
HARDHAT_PID=$!
echo $HARDHAT_PID > ../hardhat.pid
cd ..
sleep 5
echo -e "${GREEN}✓ Hardhat 節點運行中 (PID: $HARDHAT_PID)${NC}"

# 部署合約
echo -e "\n${YELLOW}[5/7] 部署 RoleManager 合約...${NC}"
cd contracts
DEPLOY_OUTPUT=$(pnpm run deploy:local 2>&1)
echo "$DEPLOY_OUTPUT"
ROLE_MANAGER_ADDRESS=$(echo "$DEPLOY_OUTPUT" | grep -o '0x[a-fA-F0-9]\{40\}' | head -1)

if [ -z "$ROLE_MANAGER_ADDRESS" ]; then
    echo -e "${RED}✗ 部署失敗，無法取得合約地址${NC}"
    exit 1
fi

echo -e "${GREEN}✓ RoleManager 部署至: $ROLE_MANAGER_ADDRESS${NC}"

# 更新 .env
if grep -q "ROLE_MANAGER_ADDRESS=" ../backend/.env; then
    sed -i.bak "s/ROLE_MANAGER_ADDRESS=.*/ROLE_MANAGER_ADDRESS=$ROLE_MANAGER_ADDRESS/" ../backend/.env
else
    echo "ROLE_MANAGER_ADDRESS=$ROLE_MANAGER_ADDRESS" >> ../backend/.env
fi
cd ..

# 執行 seed
echo -e "\n${YELLOW}[6/7] 執行 seed 資料...${NC}"
cd backend
cargo run -p api --bin dev_seed 2>&1 | tail -20
cd ..
echo -e "${GREEN}✓ Seed 完成${NC}"

# 啟動服務
echo -e "\n${YELLOW}[7/7] 啟動 Backend & Frontend...${NC}"
echo -e "${YELLOW}在新的 terminal 執行:${NC}"
echo -e "  cd backend && ENABLE_ALERT_WORKER=true PORTFOLIO_SIMULATION=true cargo run -p api"
echo -e "  cd frontend && pnpm dev"
echo ""
echo -e "${GREEN}🎉 基礎設施已就緒!${NC}"
echo ""
echo "下一步:"
echo "1. Terminal 1: cd backend && ENABLE_ALERT_WORKER=true cargo run -p api"
echo "2. Terminal 2: cd frontend && pnpm dev"
echo "3. 開啟瀏覽器: http://localhost:3000"
echo ""
echo "停止 Hardhat: kill $(cat hardhat.pid 2>/dev/null || echo '-1')"
echo "停止資料庫: docker compose down"
