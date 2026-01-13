# Build Stage
FROM rust:slim-bookworm as builder

WORKDIR /app

# 安裝編譯所需的系統依賴
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# 複製程式碼
COPY Cargo.toml Cargo.loc* ./
COPY api ./api
COPY domain ./domain
COPY auth ./auth
COPY indexer ./indexer
COPY strategy_engine ./strategy_engine
COPY alert_engine ./alert_engine
COPY migrations ./migrations

# 編譯 binary (假設主要 binary package 名稱為 api)
# 如果有 sqlx 依賴，建議先在本地執行 cargo sqlx prepare --workspace
RUN cargo build --release -p api

# Runtime Stage
FROM debian:bookworm-slim

WORKDIR /app

# 安裝執行時期的系統依賴 (OpenSSL, CA憑證)
RUN apt-get update && apt-get install -y libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*

# 從 builder 階段複製編譯好的執行檔
COPY --from=builder /app/target/release/api /app/server

# 暴露 API Port
EXPOSE 3000

# 設定執行環境變數預設值
ENV RUST_LOG=info

CMD ["./server"]