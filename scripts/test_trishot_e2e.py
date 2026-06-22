#!/usr/bin/env python3
"""
TriShot 管线端到端集成测试

模拟用户输入"写一部异星末世生存的小说"，走完完整流程：
  1. ConceptGeneration (故事概念生成) — 用活跃模型
  2. Call 1: PromptSynthesizer (路由合成器) — 用活跃模型
  3. Call 2: PromptRefiner (精修器，可选) — 用活跃模型
  4. Call 3: Writer (正文生成) — 用活跃模型

使用本机运行应用的真实模型配置（从 app_settings 表读取）。
"""

import json
import sqlite3
import urllib.request
import urllib.error
import time
import sys
import os

APP_DB = os.path.expanduser(
    "~/Library/Application Support/com.storyforge.app/cinema_ai.db"
)


def load_config():
    """从 app_settings 表读取 app_config"""
    conn = sqlite3.connect(APP_DB)
    cursor = conn.execute(
        "SELECT value FROM app_settings WHERE key = 'app_config'"
    )
    row = cursor.fetchone()
    conn.close()
    if not row:
        raise RuntimeError("app_config not found in app_settings")
    return json.loads(row[0])


def call_llm(api_base, api_key, model, messages, max_tokens=2048, temperature=0.7, timeout=120):
    """调用 OpenAI 兼容 API"""
    url = api_base.rstrip("/") + "/chat/completions"
    payload = {
        "model": model,
        "messages": messages,
        "max_tokens": max_tokens,
        "temperature": temperature,
        "stream": False,
    }
    headers = {"Content-Type": "application/json"}
    if api_key:
        headers["Authorization"] = f"Bearer {api_key}"

    data = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(url, data=data, headers=headers, method="POST")

    start = time.time()
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            body = json.loads(resp.read().decode("utf-8"))
            elapsed = time.time() - start
            content = body["choices"][0]["message"]["content"]
            tokens = body.get("usage", {}).get("completion_tokens", 0)
            return {
                "content": content,
                "tokens": tokens,
                "elapsed": elapsed,
                "model": body.get("model", model),
            }
    except urllib.error.HTTPError as e:
        elapsed = time.time() - start
        error_body = e.read().decode("utf-8", errors="replace")
        raise RuntimeError(
            f"HTTP {e.code} after {elapsed:.1f}s: {error_body[:500]}"
        )
    except Exception as e:
        elapsed = time.time() - start
        raise RuntimeError(f"Request failed after {elapsed:.1f}s: {e}")


def extract_json(content):
    """从 LLM 响应中提取 JSON（剥离 markdown 代码块）"""
    text = content.strip()
    if text.startswith("```"):
        # 去掉首行 ```json 或 ```
        nl = text.find("\n")
        if nl != -1:
            text = text[nl + 1:]
        # 去掉末尾 ```
        end = text.rfind("```")
        if end != -1:
            text = text[:end]
    text = text.strip()

    # 尝试找到 JSON 边界
    start = text.find("{")
    if start == -1:
        start = text.find("[")
    if start == -1:
        return None
    # 从末尾找闭合
    for end_char in ["}", "]"]:
        end = text.rfind(end_char)
        if end != -1 and end > start:
            candidate = text[start : end + 1]
            try:
                return json.loads(candidate)
            except json.JSONDecodeError:
                continue
    return None


def main():
    print("=" * 70)
    print("TriShot 管线端到端集成测试")
    print("用户输入: 「写一部异星末世生存的小说」")
    print("=" * 70)

    # ===== 加载配置 =====
    config = load_config()
    active_id = config.get("active_llm_profile")
    profiles = config.get("llm_profiles", {})
    active_profile = profiles.get(active_id)

    if not active_profile:
        print(f"❌ 活跃模型 {active_id} 未找到")
        sys.exit(1)

    api_base = active_profile["api_base"]
    api_key = active_profile.get("api_key", "")
    model_name = active_profile["model"]
    model_display = active_profile["name"]

    print(f"\n📋 活跃模型: {model_display} ({model_name})")
    print(f"   端点: {api_base}")
    print(f"   Profile ID: {active_id}")

    total_start = time.time()
    total_budget = 180  # 模拟 smart_execute_total_timeout_secs

    # ===== Step 1: ConceptGeneration (故事概念生成) =====
    print("\n" + "─" * 70)
    print("📌 Step 1: ConceptGeneration — 生成故事概念")
    print("─" * 70)

    concept_prompt = """你是一位资深小说编辑。请根据用户的创意，生成一个完整的故事概念。

用户输入："写一部异星末世生存的小说"

请用 JSON 格式回复：
{
  "title": "故事标题（有吸引力的中文标题）",
  "description": "一句话简介（30-50字）",
  "genre": "题材（如：都市玄幻、科幻、悬疑、古言）",
  "tone": "文风基调（如：热血、暗黑、轻松、沉重）",
  "pacing": "叙事节奏（如：快节奏、慢热、跌宕起伏）",
  "themes": ["主题1", "主题2"],
  "target_length": "预计篇幅（如：中篇30万字、长篇100万字）"
}

要求：
1. 标题要有吸引力，避免俗套
2. 简介要概括核心冲突和卖点
3. 题材必须严格遵循用户输入中的要求
4. 只输出 JSON"""

    try:
        result = call_llm(
            api_base, api_key, model_name,
            [{"role": "user", "content": concept_prompt}],
            max_tokens=512, temperature=0.7, timeout=60,
        )
    except RuntimeError as e:
        print(f"❌ ConceptGeneration 失败: {e}")
        sys.exit(1)

    print(f"   ✅ 耗时: {result['elapsed']:.1f}s | tokens: {result['tokens']} | 字符: {len(result['content'])}")
    print(f"   原始响应前200字: {result['content'][:200]}")

    concept = extract_json(result["content"])
    if not concept:
        print("❌ ConceptGeneration JSON 解析失败")
        print(f"   原始内容: {result['content']}")
        sys.exit(1)

    title = concept.get("title", "未知")
    genre = concept.get("genre", "未知")
    tone = concept.get("tone", "未知")
    pacing = concept.get("pacing", "未知")
    description = concept.get("description", "")
    themes = concept.get("themes", [])

    print(f"\n   📖 标题: {title}")
    print(f"   📂 题材: {genre}")
    print(f"   🎨 基调: {tone} | 节奏: {pacing}")
    print(f"   📝 简介: {description}")
    print(f"   💡 主题: {', '.join(themes)}")

    elapsed_so_far = time.time() - total_start
    remaining = total_budget - elapsed_so_far
    print(f"\n⏱  已用时: {elapsed_so_far:.1f}s | 剩余预算: {remaining:.1f}s")

    # ===== Step 2: Call 1 — PromptSynthesizer (路由合成器) =====
    print("\n" + "─" * 70)
    print("📌 Call 1: PromptSynthesizer — 路由合成器（选资产+合成提示词）")
    print("─" * 70)

    # 模拟 WriteTimeBundle 的约束清单（新故事，空角色/场景）
    bundle_prompt = f"""【当前故事约束清单】
故事标题：{title}
题材：{genre}
基调：{tone}
节奏：{pacing}
简介：{description}
主题：{', '.join(themes)}
角色：暂无（第一章将首次引入主角）
场景：暂无
世界观规则：暂无
伏笔状态：暂无"""

    call1_prompt = f"""你是小说创作的提示词合成器。根据用户指令、当前故事约束清单，选择相关资产并合成一个连贯、无冲突的综合创作提示词。

【用户指令】
请撰写《{title}》的第一章开头（目标字数：2000字，允许±15%）。

【当前故事约束清单（来自 WriteTimeBundle）】
{bundle_prompt}

【任务】
1. 识别用户意图（continue/rewrite/new_scene/polish/plan/other）
2. 从当前故事约束清单中选择所有硬约束资产（标记为 hard_constraint）
3. 把选中资产合成为一个连贯的中文创作提示词，解决段落间冲突，精炼冗余
4. 判断是否需要精修（复合题材/改写/多冲突约束/逾期伏笔多时 needs_refinement=true）

【输出格式】严格输出 JSON，不要 markdown 代码块：
{{"intent":"new_scene","selected_asset_ids":["characters","world_rules"],"synthesized_prompt":"合成后的完整提示词","needs_refinement":false,"refinement_focus":null,"confidence":0.8}}"""

    try:
        result = call_llm(
            api_base, api_key, model_name,
            [{"role": "user", "content": call1_prompt}],
            max_tokens=1024, temperature=0.3, timeout=90,
        )
    except RuntimeError as e:
        print(f"⚠️  Call 1 失败，回退本地拼接: {e}")
        synthesis_prompt = bundle_prompt
        needs_refinement = False
        is_fallback = True
    else:
        print(f"   ✅ 耗时: {result['elapsed']:.1f}s | tokens: {result['tokens']} | 字符: {len(result['content'])}")
        synthesis = extract_json(result["content"])
        if synthesis and synthesis.get("synthesized_prompt", "").strip():
            synthesis_prompt = synthesis["synthesized_prompt"]
            needs_refinement = synthesis.get("needs_refinement", False)
            confidence = synthesis.get("confidence", 0.5)
            intent = synthesis.get("intent", "unknown")
            is_fallback = False
            print(f"   意图: {intent}")
            print(f"   置信度: {confidence}")
            print(f"   需要精修: {needs_refinement}")
            print(f"   合成提示词字符数: {len(synthesis_prompt)}")
            print(f"   合成提示词前150字: {synthesis_prompt[:150]}...")
        else:
            print("⚠️  Call 1 JSON 解析失败，回退本地拼接")
            synthesis_prompt = bundle_prompt
            needs_refinement = False
            is_fallback = True

    elapsed_so_far = time.time() - total_start
    remaining = total_budget - elapsed_so_far
    print(f"\n⏱  已用时: {elapsed_so_far:.1f}s | 剩余预算: {remaining:.1f}s")

    # ===== Step 3: Call 2 — PromptRefiner (精修器，可选) =====
    final_prompt = synthesis_prompt

    if needs_refinement and not is_fallback:
        writer_min_estimate = 60
        if elapsed_so_far + 30 + writer_min_estimate > total_budget:
            print("\n" + "─" * 70)
            print("⏭  Call 2: PromptRefiner — 预算不足，跳过精修")
            print("─" * 70)
        else:
            print("\n" + "─" * 70)
            print("📌 Call 2: PromptRefiner — 精修提示词")
            print("─" * 70)

            refine_prompt = f"""请精修以下创作提示词，解决冲突并精炼冗余。精修重点：一般精修

待精修提示词：
{synthesis_prompt}

直接输出精修后的提示词。"""

            try:
                result = call_llm(
                    api_base, api_key, model_name,
                    [{"role": "user", "content": refine_prompt}],
                    max_tokens=1200, temperature=0.4, timeout=60,
                )
            except RuntimeError as e:
                print(f"⚠️  Call 2 失败，回退原提示词: {e}")
            else:
                refined = result["content"].strip()
                if refined:
                    final_prompt = refined
                    print(f"   ✅ 耗时: {result['elapsed']:.1f}s | tokens: {result['tokens']}")
                    print(f"   精修后字符数: {len(refined)} (原: {len(synthesis_prompt)})")
                else:
                    print("⚠️  Call 2 返回空，回退原提示词")

            elapsed_so_far = time.time() - total_start
            remaining = total_budget - elapsed_so_far
            print(f"\n⏱  已用时: {elapsed_so_far:.1f}s | 剩余预算: {remaining:.1f}s")
    else:
        print("\n" + "─" * 70)
        print("⏭  Call 2: PromptRefiner — 无需精修，跳过")
        print("─" * 70)

    # ===== Step 4: Call 3 — Writer (正文生成) =====
    print("\n" + "─" * 70)
    print("📌 Call 3: Writer — 生成第一章正文")
    print("─" * 70)

    # 计算超时（与 Rust 代码逻辑一致：剩余预算，30-120s）
    call3_timeout = max(30, min(120, int(remaining)))
    print(f"   超时设置: {call3_timeout}s (剩余预算 {remaining:.1f}s)")

    writer_prompt = f"""{final_prompt}

【写作策略】
模式：polish
冲突强度：78/100
叙事节奏：fast
AI自由度：high

【用户原始要求】
写一部异星末世生存的小说

这是故事的开篇，需要：
1. 迅速建立世界观和氛围
2. 引入主角，展示其性格和目标
3. 埋下至少一个伏笔
4. 在第一幕结尾制造一个冲突或悬念

重要：必须严格遵循用户原始要求中的题材设定，不得偏离。"""

    try:
        result = call_llm(
            api_base, api_key, model_name,
            [{"role": "user", "content": writer_prompt}],
            max_tokens=2048, temperature=0.75, timeout=call3_timeout + 10,
        )
    except RuntimeError as e:
        print(f"❌ Call 3 失败: {e}")
        sys.exit(1)

    content = result["content"]
    content_stripped = content.strip()
    char_count = len(content_stripped)
    chinese_chars = sum(1 for c in content_stripped if '\u4e00' <= c <= '\u9fff')

    print(f"   ✅ 耗时: {result['elapsed']:.1f}s | tokens: {result['tokens']} | 字符: {char_count} | 中文字: {chinese_chars}")

    if not content_stripped:
        print("❌ Call 3 返回空内容！")
        sys.exit(1)

    # ===== 汇总 =====
    total_elapsed = time.time() - total_start
    print("\n" + "=" * 70)
    print("📊 TriShot 管线测试汇总")
    print("=" * 70)
    print(f"   总耗时: {total_elapsed:.1f}s (预算 {total_budget}s)")
    print(f"   故事标题: {title}")
    print(f"   题材: {genre}")
    print(f"   第一章字符数: {char_count} (中文字 {chinese_chars})")
    print(f"   使用模型: {model_display} ({model_name})")
    print()

    # 判定
    issues = []
    if total_elapsed > total_budget:
        issues.append(f"超时: {total_elapsed:.1f}s > {total_budget}s 预算")
    if char_count < 100:
        issues.append(f"内容过短: {char_count} 字符")
    if chinese_chars < 50:
        issues.append(f"中文内容过少: {chinese_chars} 字")

    if issues:
        print("⚠️  存在问题:")
        for iss in issues:
            print(f"   - {iss}")
    else:
        print("✅ 全部检查通过！管线正常工作。")

    # 输出正文预览
    print("\n" + "─" * 70)
    print("📝 第一章正文预览（前500字）:")
    print("─" * 70)
    print(content_stripped[:500])
    if char_count > 500:
        print(f"\n... (共 {char_count} 字)")

    print("\n" + "─" * 70)
    print("📝 第一章正文末尾（最后300字）:")
    print("─" * 70)
    print(content_stripped[-300:])

    return 0 if not issues else 1


if __name__ == "__main__":
    sys.exit(main())
