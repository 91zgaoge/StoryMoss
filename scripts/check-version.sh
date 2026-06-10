#!/bin/bash
# 版本号一致性校验脚本
# 检查根 package.json、src-tauri/Cargo.toml、src-frontend/package.json、README.md 中的版本号是否一致

set -e

ROOT_VERSION=$(node -p "require('./package.json').version")
TAURI_VERSION=$(grep '^version' src-tauri/Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
FRONTEND_VERSION=$(node -p "require('./src-frontend/package.json').version")
README_VERSION=$(grep -oE 'v[0-9]+\.[0-9]+\.[0-9]+' README.md | head -1 | sed 's/v//')

echo "=== StoryForge 版本号校验 ==="
echo "根 package.json:     v$ROOT_VERSION"
echo "src-tauri/Cargo.toml: v$TAURI_VERSION"
echo "src-frontend/package.json: v$FRONTEND_VERSION"
echo "README.md:           v$README_VERSION"
echo ""

if [ "$ROOT_VERSION" != "$TAURI_VERSION" ] || [ "$ROOT_VERSION" != "$FRONTEND_VERSION" ] || [ "$ROOT_VERSION" != "$README_VERSION" ]; then
    echo "❌ 版本号不一致！"
    exit 1
fi

echo "✅ 所有版本号一致: v$ROOT_VERSION"
