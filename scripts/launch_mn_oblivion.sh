#!/bin/bash
# MN-Oblivion-26B HITOP Q6_K 本地推理服务启动脚本（包装 ~/.storymoss/bin/launch-mn-oblivion.sh）
# 如果该文件不存在，请先运行 scripts/setup_mn_oblivion.py
set -e

LAUNCHER="$HOME/.storymoss/bin/launch-mn-oblivion.sh"

if [ ! -f "$LAUNCHER" ]; then
    echo "未找到 $LAUNCHER"
    echo "请先运行：python3 scripts/setup_mn_oblivion.py"
    exit 1
fi

exec "$LAUNCHER" "$@"
