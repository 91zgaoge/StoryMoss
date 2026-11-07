# StoryMoss Server Dockerfile
# Multi-stage build for minimal runtime image
#
# v0.89.1: 构建镜像内嵌 PostgreSQL——sqlx::query! 宏在编译期需要连接数据库
# 校验表结构（宏展开即查询），因此 builder 阶段先起临时 Postgres 并按序
# 应用 migrations，再 cargo build。rust:1（滚动最新 stable）支持 Cargo.lock v4。

# Stage 1: Build
FROM rust:1-slim-bookworm AS builder

WORKDIR /app

# Install build dependencies + temporary PostgreSQL (sqlx macro compile-time validation)
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    postgresql \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations

# Start temporary Postgres, create build user/db, apply migrations in order,
# then build. sqlx::query! expands by connecting to DATABASE_URL to validate
# SQL against the real schema.
RUN service postgresql start && \
    su postgres -c "psql -c \"CREATE USER sqlx WITH PASSWORD 'sqlx' SUPERUSER;\"" && \
    su postgres -c "createdb -O sqlx sqlx" && \
    for f in /app/migrations/*.sql; do psql "postgres://sqlx:sqlx@127.0.0.1:5432/sqlx" -v ON_ERROR_STOP=1 -f "$f" || exit 1; done && \
    DATABASE_URL="postgres://sqlx:sqlx@127.0.0.1:5432/sqlx" cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Copy binary and migrations
COPY --from=builder /app/target/release/storymoss-server /app/storymoss-server
COPY --from=builder /app/migrations /app/migrations

# Expose port
EXPOSE 8080

# Run
CMD ["./storymoss-server"]
