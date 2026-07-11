---
name: diagnose
description: Disciplined diagnosis loop for hard bugs and performance regressions. Reproduce → minimise → hypothesise → instrument → fix → regression-test. Use when user says "diagnose this" / "debug this", reports a bug, says something is broken/throwing/failing, or describes a performance regression.
---

# Diagnose

A discipline for hard bugs. Skip phases only when explicitly justified.

When exploring the codebase, use the project's domain glossary to get a clear mental model of the relevant modules, and check ADRs in the area you're touching.

## Phase 1 — Build a feedback loop

**This is the skill.** Everything else is mechanical. If you have a fast, deterministic, agent-runnable pass/fail signal for the bug, you will find the cause — bisection, hypothesis-testing, and instrumentation all just consume that signal. If you don't have one, no amount of staring at code will save you.

Spend disproportionate effort here. **Be aggressive. Be creative. Refuse to give up.**

### Ways to construct one — try them in roughly this order

1. **Failing test** at whatever seam reaches the bug — unit, integration, e2e.
2. **Curl / HTTP script** against a running dev server.
3. **CLI invocation** with a fixture input, diffing stdout against a known-good snapshot.
4. **Headless browser script** (Playwright / Puppeteer) — drives the UI, asserts on DOM/console/network.
5. **Replay a captured trace.** Save a real network request / payload / event log to disk; replay it through the code path in isolation.
6. **Throwaway harness.** Spin up a minimal subset of the system (one service, mocked deps) that exercises the bug code path with a single function call.
7. **Property / fuzz loop.** If the bug is "sometimes wrong output", run 1000 random inputs and look for the failure mode.
8. **Bisection harness.** If the bug appeared between two known states (commit, dataset, version), automate "boot at state X, check, repeat" so you can `git bisect run` it.
9. **Differential loop.** Run the same input through old-version vs new-version (or two configs) and diff outputs.
10. **HITL bash script.** Last resort. If a human must click, drive _them_ with `scripts/hitl-loop.template.sh` so the loop is still structured. Captured output feeds back to you.

Build the right feedback loop, and the bug is 90% fixed.

### Iterate on the loop itself

Treat the loop as a product. Once you have _a_ loop, ask:

- Can I make it faster? (Cache setup, skip unrelated init, narrow the test scope.)
- Can I make the signal sharper? (Assert on the specific symptom, not "didn't crash".)
- Can I make it more deterministic? (Pin time, seed RNG, isolate filesystem, freeze network.)

A 30-second flaky loop is barely better than no loop. A 2-second deterministic loop is a debugging superpower.

### Non-deterministic bugs

The goal is not a clean repro but a **higher reproduction rate**. Loop the trigger 100×, parallelise, add stress, narrow timing windows, inject sleeps. A 50%-flake bug is debuggable; 1% is not — keep raising the rate until it's debuggable.

### When you genuinely cannot build a loop

Stop and say so explicitly. List what you tried. Ask the user for: (a) access to whatever environment reproduces it, (b) a captured artifact (HAR file, log dump, core dump, screen recording with timestamps), or (c) permission to add temporary production instrumentation. Do **not** proceed to hypothesise without a loop.

Do not proceed to Phase 2 until you have a loop you believe in.

## Phase 2 — Reproduce

Run the loop. Watch the bug appear.

Confirm:

- [ ] The loop produces the failure mode the **user** described — not a different failure that happens to be nearby. Wrong bug = wrong fix.
- [ ] The failure is reproducible across multiple runs (or, for non-deterministic bugs, reproducible at a high enough rate to debug against).
- [ ] You have captured the exact symptom (error message, wrong output, slow timing) so later phases can verify the fix actually addresses it.

Do not proceed until you reproduce the bug.

## Phase 3 — Hypothesise

Generate **3–5 ranked hypotheses** before testing any of them. Single-hypothesis generation anchors on the first plausible idea.

Each hypothesis must be **falsifiable**: state the prediction it makes.

> Format: "If <X> is the cause, then <changing Y> will make the bug disappear / <changing Z> will make it worse."

If you cannot state the prediction, the hypothesis is a vibe — discard or sharpen it.

**Show the ranked list to the user before testing.** They often have domain knowledge that re-ranks instantly ("we just deployed a change to #3"), or know hypotheses they've already ruled out. Cheap checkpoint, big time saver. Don't block on it — proceed with your ranking if the user is AFK.

## Phase 4 — Instrument

Each probe must map to a specific prediction from Phase 3. **Change one variable at a time.**

Tool preference:

1. **Debugger / REPL inspection** if the env supports it. One breakpoint beats ten logs.
2. **Targeted logs** at the boundaries that distinguish hypotheses.
3. Never "log everything and grep".

**Tag every debug log** with a unique prefix, e.g. `[DEBUG-a4f2]`. Cleanup at the end becomes a single grep. Untagged logs survive; tagged logs die.

**Perf branch.** For performance regressions, logs are usually wrong. Instead: establish a baseline measurement (timing harness, `performance.now()`, profiler, query plan), then bisect. Measure first, fix second.

## Phase 5 — Fix + regression test

Write the regression test **before the fix** — but only if there is a **correct seam** for it.

A correct seam is one where the test exercises the **real bug pattern** as it occurs at the call site. If the only available seam is too shallow (single-caller test when the bug needs multiple callers, unit test that can't replicate the chain that triggered the bug), a regression test there gives false confidence.

**If no correct seam exists, that itself is the finding.** Note it. The codebase architecture is preventing the bug from being locked down. Flag this for the next phase.

If a correct seam exists:

1. Turn the minimised repro into a failing test at that seam.
2. Watch it fail.
3. Apply the fix.
4. Watch it pass.
5. Re-run the Phase 1 feedback loop against the original (un-minimised) scenario.

## Phase 6 — Cleanup + post-mortem

Required before declaring done:

- [ ] Original repro no longer reproduces (re-run the Phase 1 loop)
- [ ] Regression test passes (or absence of seam is documented)
- [ ] All `[DEBUG-...]` instrumentation removed (`grep` the prefix)
- [ ] Throwaway prototypes deleted (or moved to a clearly-marked debug location)
- [ ] The hypothesis that turned out correct is stated in the commit / PR message — so the next debugger learns

**Then ask: what would have prevented this bug?** If the answer involves architectural change (no good test seam, tangled callers, hidden coupling) hand off to the `/improve-codebase-architecture` skill with the specifics. Make the recommendation **after** the fix is in, not before — you have more information now than when you started.

---

## StoryMoss 专属补充

本项目已有一套成熟的诊断基础设施，优先用它们构造反馈回路，不要从零搭。

### 首选回路：`creative_workflow.log` 时间线对照法

把用户感知的“卡住/异常时刻”对齐到 `<app_data_dir>/logs/creative_workflow.log` 的阶段标记，卡点 = 时间线断点。关键标记：`genesis.first_chapter.generated` / `genesis.chapter_switch.sent` / `genesis.final_content` / `smart_execute.start` / `trishot.call3.done` / `trishot.bgp4.spawn` / `trishot.bgp4.done` / `llm.record_call.spawn`（候选实时探测走标准 `log::debug!` 的 `[Gateway]` 行，不进此日志）。实例：v0.26.23 用此法定位 `auto_contract` 阻塞续写 6 分钟；v0.23.18 用 12+ 行级标记定位 600s 超时卡在 `db_write`。

### 已知“盯代码无效”的失败家族（先排除再下假设）

- **竞态家族**（第一章重复 saga v0.26.7–.16，9 轮）：单基准（`editorRef.getText()`）滞后 DOM + 多写者并发。必须单写者状态机 + 双重基准（`latestContentRef` + DOM 校准）。提取纯函数写契约测试。
- **阻塞家族**（600s / BGP-4 / Ingest / auto_contract）：后台 LLM/DB 调用同步化。查 `is_silent_background` 白名单（`src-tauri/src/llm/service.rs`）+ `spawn_blocking`/`tokio::spawn` fire-and-forget + 连接池 `.connection_timeout`。
- **死锁家族**（v0.23.34/.42）：`std::sync::Mutex` 不可重入；持锁期间再 lock。中毒锁 `unwrap_or_else(|e| e.into_inner())`。
- **JSON 解析家族**（v0.23.48/.49）：推理模型思考链花括号被 `find('{')` 误判；用 `strip_reasoning_blocks` + 括号匹配 `extract_first_json_object`。

完整 symptom→root cause→evidence→status 编年史见 `sf-failure-archaeology`；症状分诊表见 `sf-debugging-playbook`；诊断工具用法见 `sf-diagnostics-and-tooling`。改任何符号前先 `gitnexus_impact`。任何修复在合并前必须走 `sf-change-control`（变更门禁）+ `sf-validation-and-qa`（证据标准）。

## 何时 NOT 用本技能（StoryMoss 场景）

- 已修复战役编年史 → `sf-failure-archaeology`。
- 诊断工具具体用法 → `sf-diagnostics-and-tooling`。
- 架构不变量 → `sf-architecture-contract`。

## 出处与维护（StoryMoss 补充）

- 重验证命令：`rg -n 'genesis\.first_chapter|smart_execute\.start|trishot\.call3\.done|pre_call_probe' src-tauri/src | head`（日志阶段标记是否仍在）
- 易漂移项：日志阶段标记名、`is_silent_background` 白名单、超时常量。
- 最后核对：2026-07-07，v0.26.23。
