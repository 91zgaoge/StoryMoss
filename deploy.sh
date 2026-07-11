#!/bin/bash
# StoryMoss Server 一键部署脚本
# 在 Linux 服务器上执行此脚本完成部署
# 使用方法:
#   1. git clone https://github.com/91zgaoge/StoryMoss.git
#   2. cd StoryMoss
#   3. chmod +x deploy.sh
#   4. ./deploy.sh

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}🚀 StoryMoss Server 部署脚本 v4.5.0${NC}"
echo "=========================================="

# 检查 Docker 和 Docker Compose
if ! command -v docker &> /dev/null; then
    echo -e "${RED}❌ Docker 未安装${NC}"
    echo "请先安装 Docker: https://docs.docker.com/get-docker/"
    exit 1
fi

if ! command -v docker-compose &> /dev/null && ! docker compose version &> /dev/null; then
    echo -e "${RED}❌ Docker Compose 未安装${NC}"
    echo "请先安装 Docker Compose: https://docs.docker.com/compose/install/"
    exit 1
fi

# 检查 .env 文件
if [ ! -f .env ]; then
    if [ -f .env.example ]; then
        echo -e "${YELLOW}⚠️ .env 文件不存在，从 .env.example 复制${NC}"
        cp .env.example .env
        echo -e "${RED}❌ 请先编辑 .env 文件，配置数据库密码和 OAuth 密钥${NC}"
        echo "必要配置项:"
        echo "  - POSTGRES_PASSWORD"
        echo "  - JWT_SECRET"
        echo "  - GOOGLE_CLIENT_ID / GITHUB_CLIENT_ID (至少一个)"
        exit 1
    else
        echo -e "${RED}❌ .env.example 文件不存在${NC}"
        exit 1
    fi
fi

# 检查必要配置
if grep -q "your-secure-password-here" .env || grep -q "your-super-secret-jwt-key" .env; then
    echo -e "${RED}❌ .env 文件中仍使用默认密码/密钥${NC}"
    echo "请编辑 .env 文件，修改 POSTGRES_PASSWORD 和 JWT_SECRET"
    exit 1
fi

echo -e "${GREEN}✅ 环境检查通过${NC}"

# 拉取最新代码
echo -e "${YELLOW}📦 拉取最新代码...${NC}"
git pull origin master 2>/dev/null || true

# 构建并启动
echo -e "${YELLOW}🏗️ 构建并启动服务...${NC}"
if docker compose version &> /dev/null; then
    docker compose up -d --build
else
    docker-compose up -d --build
fi

# 等待服务启动
echo -e "${YELLOW}⏳ 等待服务启动...${NC}"
sleep 10

# 健康检查
echo -e "${YELLOW}🔍 健康检查...${NC}"
for i in {1..30}; do
    if curl -sf http://localhost:8080/api/health > /dev/null 2>&1; then
        echo -e "${GREEN}✅ 服务端运行正常${NC}"
        break
    fi
    if [ $i -eq 30 ]; then
        echo -e "${RED}⚠️ 服务端启动超时，请检查日志${NC}"
        echo "查看日志: docker-compose logs -f server"
    fi
    sleep 2
done

# 显示状态
echo ""
echo -e "${GREEN}🎉 StoryMoss Server 部署完成！${NC}"
echo "=========================================="
echo ""
echo "服务地址:"
echo "  🌐 落地页:     http://localhost"
echo "  🔧 API:        http://localhost:8080/api"
echo "  🗄️  PostgreSQL: localhost:5432"
echo ""
echo "常用命令:"
echo "  查看日志:   docker-compose logs -f server"
echo "  停止服务:   docker-compose down"
echo "  重启服务:   docker-compose restart"
echo "  更新部署:   ./deploy.sh"
echo ""
echo "OAuth 配置:"
echo "  编辑 .env 文件配置 GOOGLE_CLIENT_ID / GITHUB_CLIENT_ID"
echo "  重启服务生效: docker-compose restart server"
echo ""
