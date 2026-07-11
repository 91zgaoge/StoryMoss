#!/usr/bin/env python3
"""
StoryMoss MN-Oblivion-26B 本地模型部署脚本
- 下载推荐的 HITOP Q5_K_M GGUF
- 创建 Metal 优化的 llama-server 启动脚本
- 自动将模型注册为 StoryMoss 的活跃 LLM profile
"""
import json
import os
import sqlite3
import subprocess
import sys
from pathlib import Path

# 代理配置（如果环境未设置，使用本机常见 Clash/V2Ray 端口）
PROXY = os.environ.get("HTTP_PROXY") or os.environ.get("http_proxy") or "http://127.0.0.1:10808"
if not os.environ.get("HTTP_PROXY"):
    os.environ["HTTP_PROXY"] = PROXY
    os.environ["HTTPS_PROXY"] = PROXY
    os.environ["ALL_PROXY"] = PROXY

REPO_ID = "DavidAU/MN-Oblivion-26B-UNCENSORED-NEO-Imatrix-GGUF"
# 默认使用 Q6_K：在 M5 Max 128GB 上质量更高，速度仍可接受
RECOMMENDED_FILE = "MN-Oblivion-26B-UNCENSORED-HITOP-Q6_K.gguf"
ALTERNATIVES = {
    "balanced": "MN-Oblivion-26B-UNCENSORED-HITOP-Q5_K_M.gguf",
    "speed": "MN-Oblivion-26B-UNCENSORED-HITOP-Q4_K_M.gguf",
    "max_quality": "MN-Oblivion-26B-UNCENSORED-HITOP-Q8_0.gguf",
}
MODEL_DIR = Path("/Users/yuzaimu/models/mn-oblivion-26b")
LAUNCH_SCRIPT = Path.home() / ".storymoss" / "bin" / "launch-mn-oblivion.sh"
APP_DATA_DIR = Path.home() / "Library" / "Application Support" / "com.storymoss.app"
DB_PATH = APP_DATA_DIR / "cinema_ai.db"
PROFILE_ID = "mn-oblivion-26b-hitop-q6-k"
SERVER_HOST = "127.0.0.1"
SERVER_PORT = 11500


def download_model(filename: str) -> Path:
    MODEL_DIR.mkdir(parents=True, exist_ok=True)
    target = MODEL_DIR / filename
    # 使用 HuggingFace 直链（走 HTTP 代理）
    url = f"https://huggingface.co/{REPO_ID}/resolve/main/{filename}"
    print(f"正在下载 {filename} 到 {target} ...")
    print(f"URL: {url}")
    print(f"代理: {PROXY}")

    cmd = [
        "curl",
        "-L",
        "-C", "-",
        "--progress-bar",
        "--connect-timeout", "30",
        "--max-time", "0",
        "-x", PROXY,
        "-o", str(target),
        url,
    ]
    try:
        subprocess.run(cmd, check=True)
    except subprocess.CalledProcessError as e:
        print(f"下载失败: {e}")
        sys.exit(1)
    return target


def create_launch_script(gguf_path: Path) -> Path:
    LAUNCH_SCRIPT.parent.mkdir(parents=True, exist_ok=True)
    # M5 Max / Apple Silicon 优化参数
    script = f"""#!/bin/bash
# MN-Oblivion-26B HITOP Q6_K 本地推理服务
# 由 scripts/setup_mn_oblivion.py 自动生成
set -e

MODEL="{gguf_path}"
HOST={SERVER_HOST}
PORT={SERVER_PORT}

if [ ! -f "$MODEL" ]; then
    echo "错误：模型文件不存在：$MODEL"
    exit 1
fi

echo "启动 MN-Oblivion-26B 本地服务..."
echo "端点: http://$HOST:$PORT/v1"
echo "模型: $MODEL"

# 参数说明：
# -ngl 99           :  offload 所有层到 Apple Silicon GPU/Neural Engine
# -c 32768          :  32K 上下文窗口（模型支持 128K，32K 在本地最稳）
# -n 4096           :  单次最多生成 4096 tokens
# -t 12             :  使用 12 线程，留余量给系统和 StoryMoss
# -tb 12            :  prompt 处理同样 12 线程
# --mlock           :  锁定内存，避免 swap 导致卡顿
# --flash-attn on   :  开启 Flash Attention，提速并省显存
# --cont-batching   :  连续批处理，提高并发效率
# -np 1             :  单 slot，StoryMoss 内部自行调度并发
exec llama-server \\
    --model "$MODEL" \\
    --host "$HOST" \\
    --port "$PORT" \\
    --n-gpu-layers 99 \\
    --ctx-size 32768 \\
    --n-predict 4096 \\
    --threads 12 \\
    --threads-batch 12 \\
    --mlock \\
    --flash-attn on \\
    --cont-batching \\
    --parallel 1 \\
    --timeout 600 \\
    "$@"
"""
    LAUNCH_SCRIPT.write_text(script, encoding="utf-8")
    LAUNCH_SCRIPT.chmod(0o755)
    print(f"启动脚本已创建: {LAUNCH_SCRIPT}")
    return LAUNCH_SCRIPT


def patch_storymoss_config():
    if not DB_PATH.exists():
        print(f"警告：StoryMoss 数据库不存在，跳过自动配置：{DB_PATH}")
        print("请启动一次 StoryMoss 后再运行本脚本，或手动在设置中添加模型。")
        return

    conn = sqlite3.connect(DB_PATH)
    cur = conn.cursor()
    cur.execute("SELECT value FROM app_settings WHERE key='app_config'")
    row = cur.fetchone()
    if not row:
        print("警告：app_config 不存在，跳过自动配置")
        return

    config = json.loads(row[0])
    profiles = config.setdefault("llm_profiles", {})

    # 构建新 profile
    profile = {
        "id": PROFILE_ID,
        "name": "MN-Oblivion-26B HITOP Q6_K",
        "description": "本地部署的 MN-Oblivion-26B 无审查创意写作模型（HITOP Q6_K，质量优先）",
        "provider": "custom",
        "model_source": "local",
        "model": RECOMMENDED_FILE,
        "api_key": "",
        "api_base": f"http://{SERVER_HOST}:{SERVER_PORT}/v1",
        "is_local_model": True,
        "max_tokens": 4096,
        "temperature": 0.85,
        "top_p": None,
        "frequency_penalty": None,
        "presence_penalty": None,
        "timeout_seconds": 300,
        "is_default": True,
        "enabled": True,
        "kind": "chat",
        "capabilities": [
            "chat",
            "completion",
            "long_context",
            "streaming",
            "reasoning",
            "function_calling",
        ],
        "max_context_length": 32768,
        "quality_tier": "high",
        "speed_tier": "normal",
        "cost_per_1k_input": None,
        "cost_per_1k_output": None,
        "tags": ["local", "creative", "uncensored", "mn-oblivion"],
        "supports_system_prompt": True,
        "supports_streaming": True,
        "knowledge_cutoff": None,
        "reasoning_effort": None,
    }

    # 取消其它 profile 的 default
    for pid, p in profiles.items():
        if pid != PROFILE_ID:
            p["is_default"] = False

    profiles[PROFILE_ID] = profile
    config["active_llm_profile"] = PROFILE_ID

    # 调优生成参数以适配本地大模型
    config["generation_mode"] = config.get("generation_mode", "tri_shot")
    config["candidate_timeout_local_seconds"] = max(config.get("candidate_timeout_local_seconds", 60), 120)
    config["writer_local_concurrency"] = 1
    config["llm_first_chunk_timeout_secs"] = max(config.get("llm_first_chunk_timeout_secs", 60), 90)

    new_value = json.dumps(config, ensure_ascii=False, separators=(",", ":"))
    cur.execute(
        "UPDATE app_settings SET value=?, updated_at=datetime('now') WHERE key='app_config'",
        (new_value,),
    )
    conn.commit()
    conn.close()
    print(f"StoryMoss 配置已更新：活跃模型设为 {PROFILE_ID}")


def main():
    print("=" * 60)
    print("StoryMoss MN-Oblivion-26B 本地模型部署")
    print("=" * 60)
    print(f"推荐版本: {RECOMMENDED_FILE}")
    print("理由：HITOP Q6_K（20.31 GB）质量接近原始精度，")
    print("      在 M5 Max 128GB 统一内存上仍可全层 offload 到 Metal，")
    print("      适合以输出质量为优先的创意写作场景。")
    print("=" * 60)

    # 允许通过环境变量切换版本
    filename = os.environ.get("MN_OBLIVION_VARIANT", RECOMMENDED_FILE)
    if filename not in [RECOMMENDED_FILE] + list(ALTERNATIVES.values()):
        print(f"警告：未知版本 {filename}，使用默认 {RECOMMENDED_FILE}")
        filename = RECOMMENDED_FILE

    gguf_path = download_model(filename)
    print(f"模型已就绪: {gguf_path}")
    print(f"文件大小: {gguf_path.stat().st_size / (1024**3):.2f} GB")

    create_launch_script(gguf_path)
    patch_storymoss_config()

    print("\n" + "=" * 60)
    print("部署完成！接下来请执行：")
    print(f"  1. 启动模型服务：{LAUNCH_SCRIPT}")
    print("  2. 启动 StoryMoss，模型已自动设为 MN-Oblivion-26B")
    print("=" * 60)


if __name__ == "__main__":
    main()
