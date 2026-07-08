---
name: sf-proof-and-analysis-toolkit
description: StoryForge 领域的第一性原理分析方法（“证明它，别只装它”），每条配本 repo 历史的 worked example。何时加载：要为某个阈值/设计决策找推导依据、要量化验证一个架构假设、要做 A/B 盲测、要证明单写者状态机/竞态修复有效、或被问“怎么证明这个对”时。
---

# StoryForge 证明与分析工具包

> R5：执行是 ground truth。每个“证明它”都配一个本 repo 历史的 worked example。

## 方法 1：阈值推导——8% 自重复闸门

**问题**：为何生成侧自重复阈值是 8% 而非更高/更低？
**推导**：阈值要满足两个约束——(a) 不误杀正常叙事的有意重复（如排比、回环修辞）；(b) 能拦截用户可感知的首尾回环。`trim_self_repetition` 的 KMP 最长 border 比例 ≥8% 且重复长度 ≥30 字时裁剪。8% 是经验校准：低于此的正常文本极少有 ≥30 字的精确 border。
**worked example**：v0.26.14 日志中 LLM 输出 613 字正文首尾段落相同，border 比例远超 8% → 被裁；正常排比段落 border < 30 字 → 不裁。
**怎么复用**：改阈值时跑 `tests/fixtures/trim_golden.json` 7 用例 + 补边界用例（正常修辞不应被裁，整章重复应被裁）。

## 方法 2：A/B 盲测——架构假设验证

**问题**：分时介入架构成立吗（最小约束 vs 全量资产质量差是否 < 30% 阈值）？
**推导**：若差 ≥30%，热路径放弃资产不划算；若 <30%，后台审计可追平，架构成立。
**worked example**：v0.13.0 Phase 0，qwen3.6-35b，3 场景 A/B 盲测 → 平均质量差 **7.9%**（< 30%），prompt 长 160% 仅耗时多 7% → 证实“慢在同步链路而非 Writer 本身”。
**怎么复用**：任何“要不要把 X 移出/移入热路径”的决策，用同题材同创意 A/B 盲测 + 评分器，预设阈值再下结论。`sf-genesis-campaign` 阶段 0 即此法。

## 方法 3：单写者状态机证明——竞态根治

**问题**：如何证明单写者状态机消除了第一章重复，而非又一次“看起来修了”？
**推导**：散布布尔守卫的失败模式是“多个写者各自检查布尔，窗口期内并发写”。单写者状态机的证明：所有写路径都被状态机的 `generating`/`delivered` 态阻塞，状态转移是单点。用状态机端点契约测试覆盖 `idle→generating→delivered` 每个转移的副作用。
**worked example**：v0.26.16 把 `genesisAutoAcceptedRef` 布尔换 `idle→generating→delivered`；v0.26.19 加状态机端点契约测试。9 轮 saga 终结。
**怎么复用**：任何竞态修复，先提取状态机端点契约测试（参考前端 Gap B/C + 状态机测试），再改实现。

## 方法 4：reproduce-before-fix——竞态定位

**问题**：如何不靠盯代码定位竞态？
**推导**：竞态必有可放大的时间窗口。先复现（提高 reproduction rate：循环 100×、加 stress、注入 sleep、缩窄时序），再最小化 repro，再 hypothesise 3-5 个可证伪假设。
**worked example**：v0.26.9 DOM 滞后竞态——`editorRef.getText()` 滞后于 React `content` prop，重复检测失效；改 `latestContentRef` 基准。单元测试覆盖 DOM 滞后竞态。
**怎么复用**：见 `diagnose` 技能 Phase 1-2。

## 方法 5：跨层一致性证明——golden 双跑

**问题**：如何保证 Rust 与 TS 实现同一算法不漂移？
**推导**：单边测试无法发现跨层漂移；共享 golden fixture 双跑锁定。
**worked example**：`tests/fixtures/trim_golden.json` 7 用例，Rust `trim_self_repetition_matches_shared_golden_fixture` + TS `textCleanup.golden.test.ts` 双跑。
**怎么复用**：任何 Rust+TS 共享算法（清洗/分词/评分）都用此模式。

## 方法 6：阻塞定位——行级诊断标记

**问题**：600s 超时卡在哪？
**推导**：在 Ok 分支每步前后插标记，卡点 = 标记断点。
**worked example**：v0.23.18 `execute_generation` Ok 分支 12+ 标记（`record_call.start→try_state→db_write→db_done→emit_completed.start→generate.return_ok`）定位 `db_write` 阻塞。
**怎么复用**：见 `sf-diagnostics-and-tooling` WorkflowLogger 标记法；清理用唯一前缀 `grep`。

## 方法 7：死锁定位——不可重入 Mutex

**问题**：Call3 必死锁但 Call1 不死锁？
**推导**：`std::sync::Mutex` 不可重入；Call1 走 `select_fastest_profile` 不二次 lock，Call3 走 `select_candidates` 在持锁期间再 lock。
**worked example**：v0.23.34 15 个诊断标记定位 health 锁二次 lock；移入嵌套块作用域。
**怎么复用**：任何 Mutex 死锁先排查同线程二次 lock；中毒锁 `unwrap_or_else(|e| e.into_inner())` 恢复。

## 何时 NOT 用本技能

- 通用调试纪律 → `diagnose`。
- 失败编年史 → `sf-failure-archaeology`。
- 开放前沿（Context Rot）→ `sf-research-frontier`。

## 出处与维护

- 重验证命令：
  - `cat tests/fixtures/trim_golden.json | head`（golden 是否在）
  - `rg -n 'compute_trim_ratio|should_retry_self_repetition' src-tauri/src/narrative/genesis.rs`（闸门纯函数）
  - `rg -n '7\.9%|30%' docs/ ARCHITECTURE.md README.md | head`（A/B 结论出处）
- 易漂移项：阈值常量、golden 用例、A/B 结论数字。
- 最后核对：2026-07-07，v0.26.23。
