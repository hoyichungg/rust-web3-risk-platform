SHELL := /bin/bash

# 根目錄快速指令
.PHONY: chain deploy backend frontend dev seed lint backend-test prod-up prod-down

# 啟動本地 Hardhat 鏈（contracts 專案）
chain:
	cd contracts && pnpm run node

# 部署 RoleManager 到本地鏈
deploy:
	cd contracts && pnpm run deploy:local

# 啟動後端 API（需要正確 .env）
backend:
	cd backend && cargo run -p api

# 啟動前端
frontend:
	cd frontend && pnpm dev

# 跑 demo seed（建立示範錢包/資產/告警/價格）
seed:
	cd backend && cargo run -p api --bin dev_seed

# 後端測試（包含 SQLx migrations）
backend-test:
	cd backend && cargo test -p api -- --nocapture

# 前端 lint
lint:
	cd frontend && pnpm lint

# 啟動生產環境 (Docker Compose)
prod-up:
	docker compose -f docker-compose.prod.yml up -d --build

# 關閉生產環境
prod-down:
	docker compose -f docker-compose.prod.yml down
