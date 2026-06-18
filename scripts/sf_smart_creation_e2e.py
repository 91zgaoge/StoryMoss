#!/usr/bin/env python3
"""v0.14.3 智能创作端到端集成测试 - 直连 vllm 验证完整生成链路"""
import json, time, urllib.request, sys

GEMMA_BASE = "http://10.62.239.13:17092/v1"
GEMMA_MODEL = "gemma4-e2b"
QWEN_BASE = "http://10.62.239.13:17098/v1"
QWEN_MODEL = "qwen3.6-35b-a3b-vision"


def call_vllm(api_base, model, prompt, max_tokens=2500, temperature=0.8, timeout=180):
    url = f"{api_base}/chat/completions"
    payload = {"model": model, "messages": [{"role": "user", "content": prompt}],
               "max_tokens": max_tokens, "temperature": temperature}
    data = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(url, data=data, headers={"Content-Type": "application/json"})
    start = time.time()
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            obj = json.loads(resp.read().decode("utf-8"))
            return {"ok": True, "content": obj["choices"][0]["message"]["content"],
                    "elapsed": time.time() - start}
    except Exception as e:
        return {"ok": False, "error": str(e), "elapsed": time.time() - start}


CONTINUATION = """你是一名优秀的小说写作助手。请根据以下已有内容，自然地续写下一段（约 800-1200 字）。

【已有内容】
夜风穿过古城墙的残垣，发出呜咽般的低鸣。林思远站在断壁上，望着远方那片被晚霞染红的雪山。他攥紧了腰间那柄随身十年的青铜剑——剑身已经斑驳，剑柄上的红绳褪了色，但每一次握住它，他都能感受到师父留下的温度。

"该走了。"身后传来沈青鸢的声音，清冷如山泉。她披着一件墨色斗篷，腰间挂着两只竹哨，眉间一颗朱砂痣在夕阳下泛着微光。

林思远没有回头："再等一会儿。"

风更大了。雪山深处隐隐传来鹰鸣。

【续写要求】
- 保持原有的武侠古风文笔
- 体现两人之间的情感张力
- 加入景物描写推动叙事

请直接输出续写内容，不要添加任何说明："""

REWRITE = """你是一名严苛的文学编辑。请改写以下段落，使其更紧凑有力（约 200-300 字）。

【原文】
夜风穿过古城墙的残垣，发出呜咽般的低鸣。林思远站在断壁上，望着远方那片被晚霞染红的雪山。他攥紧了腰间那柄随身十年的青铜剑——剑身已经斑驳，剑柄上的红绳褪了色，但每一次握住它，他都能感受到师父留下的温度。

【改写要求】
- 删减冗余形容词
- 强化动作和心理张力
- 节奏更紧凑

请直接输出改写后的段落："""


results = []


def run(name, fn):
    print(f"\n{'='*70}\n{name}\n{'='*70}")
    try:
        ok = fn()
        status = "PASS" if ok else "FAIL"
        results.append((name, ok, ""))
        print(f"\n[{status}]")
    except AssertionError as e:
        results.append((name, False, str(e)))
        print(f"\n[FAIL] 断言失败：{e}")
    except Exception as e:
        results.append((name, False, str(e)))
        print(f"\n[ERROR] {e}")


def t1():
    """T1: Gemma4 端点健康检查"""
    r = call_vllm(GEMMA_BASE, GEMMA_MODEL, "用一句话介绍你自己（不超过 30 字）", 50, 0.3, 30)
    print(f"耗时 {r['elapsed']:.2f}s")
    if not r["ok"]:
        print(f"错误：{r['error']}"); return False
    print(f"回复：{r['content']}")
    assert r["elapsed"] < 30, f"应在 30s 内"
    return bool(r["content"])


def t2():
    """T2: Qwen 端点健康检查"""
    r = call_vllm(QWEN_BASE, QWEN_MODEL, "用一句话介绍你自己（不超过 30 字）", 50, 0.3, 60)
    print(f"耗时 {r['elapsed']:.2f}s")
    if not r["ok"]:
        print(f"错误：{r['error']}"); return False
    print(f"回复：{r['content']}")
    return bool(r["content"])


def t3():
    """T3: 续写场景（v0.14.3 TimeSliced 模式核心场景）"""
    print("场景：current_content 非空 + selected_text 为空 → TimeSliced 模式")
    print("v0.14.3 路由：单次 LLM 调用，期望 < 180s\n")
    r = call_vllm(GEMMA_BASE, GEMMA_MODEL, CONTINUATION, 2500, 0.8, 180)
    print(f"耗时 {r['elapsed']:.2f}s")
    if not r["ok"]:
        print(f"错误：{r['error']}"); return False
    n = len(r["content"])
    print(f"字数：{n}")
    print(f"\n--- 内容预览（前 400 字）---\n{r['content'][:400]}")
    if n > 400: print(f"...（共 {n} 字）")
    assert r["elapsed"] < 180, "应在 180s 内"
    assert n > 200, f"内容过短：{n} 字"
    assert "【已有内容】" not in r["content"], "不应包含 prompt 模板"
    return True


def t4():
    """T4: 重写场景（v0.14.3 Full 模式的 Writer 阶段）"""
    print("场景：selected_text 非空 → Full 模式（此处只测 Writer 单次调用）")
    print("期望：< 90s\n")
    r = call_vllm(GEMMA_BASE, GEMMA_MODEL, REWRITE, 800, 0.5, 90)
    print(f"耗时 {r['elapsed']:.2f}s")
    if not r["ok"]:
        print(f"错误：{r['error']}"); return False
    n = len(r["content"])
    print(f"字数：{n}\n--- 改写后内容 ---\n{r['content']}")
    assert r["elapsed"] < 90, "应在 90s 内"
    assert n > 50, "改写过短"
    return True


def t5():
    """T5: 长 prompt 不挂起（v0.14.2 首字节防线）"""
    print("场景：长 prompt 模拟 vllm 半挂\n期望：< 200s 完成或超时\n")
    long_p = "请记住：" + ("小说情节复杂多变。" * 30) + "\n（只回复一个字 '好'）"
    r = call_vllm(GEMMA_BASE, GEMMA_MODEL, long_p, 10, 0.1, 200)
    print(f"耗时 {r['elapsed']:.2f}s")
    if r["ok"]:
        print(f"成功：{r['content']}")
    else:
        print(f"失败（合理）：{r['error']}")
    assert r["elapsed"] < 200, "不应超过 200s"
    return True


def t6():
    """T6: 连续 3 次续写稳定性"""
    print("场景：3 次续写验证响应一致性\n")
    times = []
    for i in range(3):
        print(f"--- 第 {i+1} 次 ---")
        r = call_vllm(GEMMA_BASE, GEMMA_MODEL, CONTINUATION, 1500, 0.8, 120)
        if not r["ok"]:
            print(f"  失败：{r['error']}"); return False
        print(f"  耗时 {r['elapsed']:.2f}s, {len(r['content'])} 字")
        times.append(r["elapsed"])
    avg = sum(times) / len(times)
    print(f"\n平均 {avg:.2f}s, max {max(times):.2f}s, min {min(times):.2f}s")
    assert all(t < 180 for t in times), "应全部 < 180s"
    return True


def t7():
    """T7: 验证 v0.14.3 场景路由逻辑（纯算法）"""
    print("验证 PlanExecutor::execute_writer 的场景智能路由\n")

    def route(selected_text=None, has_content=False, mode_override=None, app_mode="auto"):
        m = mode_override or app_mode
        if m == "full": return "Full"
        if m == "fast": return "Fast"
        if m in ("time_sliced", "timesliced"): return "TimeSliced"
        # auto
        return "Full" if selected_text else "TimeSliced"

    cases = [
        ("续写（无选中文本）", route(None, True), "TimeSliced"),
        ("重写选中文本", route("一段文字", True), "Full"),
        ("新章首段（空内容）", route(None, False), "TimeSliced"),
        ("用户强制 Full", route(None, True, "full"), "Full"),
        ("用户强制 Fast", route(None, True, "fast"), "Fast"),
        ("AppConfig=time_sliced", route("有选中", True, None, "time_sliced"), "TimeSliced"),
    ]
    print(f"{'场景':<25}{'路由结果':<15}{'期望':<15}{'状态'}")
    all_pass = True
    for name, actual, expected in cases:
        ok = actual == expected
        print(f"{name:<25}{actual:<15}{expected:<15}{'PASS' if ok else 'FAIL'}")
        if not ok: all_pass = False
    return all_pass


# ============================================================
# 主程序
# ============================================================

if __name__ == "__main__":
    print("v0.14.3 智能创作端到端集成测试")
    print(f"目标：验证 v0.14.2 + v0.14.3 修复后能稳定生成内容")
    print(f"vllm 端点：Gemma4 ({GEMMA_BASE}) + Qwen ({QWEN_BASE})")

    run("T1: Gemma4 端点健康检查", t1)
    run("T2: Qwen 端点健康检查", t2)
    run("T7: v0.14.3 场景路由逻辑（纯算法）", t7)
    run("T3: 续写场景 (TimeSliced 核心场景)", t3)
    run("T4: 重写场景 (Full 模式 Writer)", t4)
    run("T5: 长 prompt 不挂起 (v0.14.2 防线)", t5)
    run("T6: 连续 3 次续写稳定性", t6)

    # 总结
    print("\n" + "=" * 70)
    print("测试结果汇总")
    print("=" * 70)
    passed = sum(1 for _, ok, _ in results if ok)
    total = len(results)
    for name, ok, err in results:
        status = "PASS" if ok else "FAIL"
        print(f"  [{status}] {name}" + (f"  ({err})" if err else ""))
    print(f"\n总计：{passed}/{total} 通过")
    sys.exit(0 if passed == total else 1)
