# StoryMoss Server 部署指南

## 概述

StoryMoss Server v4.5.0 包含三个组件：

| 组件 | 技术 | 端口 | 说明 |
|------|------|------|------|
| PostgreSQL | 数据库 | 5432 | 用户/会话数据持久化 |
| StoryMoss Server | Actix-web (Rust) | 8080 | REST API + OAuth |
| StoryMoss Web | React + Nginx | 80 | 落地页 + Web登录 + Dashboard |

## 快速开始（Docker Compose）

### 1. 准备环境

```bash
# 安装 Docker + Docker Compose
# https://docs.docker.com/get-docker/

# 克隆项目
git clone https://github.com/91zgaoge/StoryMoss.git
cd StoryMoss
```

### 2. 配置环境变量

```bash
cp .env.example .env
nano .env  # 编辑配置
```

必须配置的项：
- `POSTGRES_PASSWORD` — 数据库密码
- `JWT_SECRET` — JWT签名密钥（至少32字符）
- `GOOGLE_CLIENT_ID` / `GOOGLE_CLIENT_SECRET` — 或 GitHub OAuth

### 3. 启动服务

```bash
docker-compose up -d
```

### 4. 验证

```bash
# 检查健康状态
curl http://localhost:8080/api/health

# 访问落地页
open http://localhost
```

## OAuth 应用注册

### Google

1. 访问 https://console.cloud.google.com/apis/credentials
2. 创建 OAuth 2.0 客户端 ID
3. 应用类型选择 "Web 应用"
4. 授权重定向 URI: `http://your-domain/api/auth/google/callback`
5. 复制客户端 ID 和密钥到 `.env`

### GitHub

1. 访问 https://github.com/settings/developers
2. 新建 OAuth App
3. Authorization callback URL: `http://your-domain/api/auth/github/callback`
4. 复制 Client ID 和 Client Secret 到 `.env`

### 微信/QQ（预留）

需要在对应的开放平台注册应用，配置方式类似。

## 目录结构

```
StoryMoss/
├── src-tauri/           # 桌面端（Tauri + Rust）
│   └── src/auth/        # 桌面端认证模块
├── src-frontend/        # 桌面端前端（React）
├── src-server/          # 【服务端后端】
│   ├── src/
│   │   ├── main.rs      # Actix-web 入口
│   │   ├── config.rs    # 环境配置
│   │   ├── auth/        # OAuth + JWT
│   │   └── api/         # REST API
│   ├── migrations/      # PostgreSQL 迁移
│   └── Dockerfile
├── src-server-web/      # 【服务端前端】
│   ├── src/pages/
│   │   ├── LandingPage.tsx
│   │   ├── LoginPage.tsx
│   │   └── DashboardPage.tsx
│   └── Dockerfile
├── docker-compose.yml   # 一键部署
└── .env.example         # 配置模板
```

## API 端点

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | /api/health | 健康检查 |
| GET | /api/auth/config | 获取已启用的OAuth配置 |
| GET | /api/auth/{provider}/start | 开始OAuth登录 |
| GET | /api/auth/{provider}/callback | OAuth回调 |
| POST | /api/auth/logout | 注销 |
| GET | /api/auth/me | 获取当前用户 |
| GET | /api/users/me | 获取当前用户详情 |

## 更新

```bash
# 拉取最新代码
git pull origin main

# 重建并重启
docker-compose down
docker-compose up -d --build
```
