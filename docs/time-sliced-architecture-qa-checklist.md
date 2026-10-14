# 分时介入架构 — QA 验收清单

> ⚠️ **历史快照**：本文记录当时版本的代码状态，非现状。现行流程与资产以 [ARCHITECTURE.md](../ARCHITECTURE.md)（Agency 多代理创作框架）与 [PROJECT_STATUS.md](../PROJECT_STATUS.md) 为准；最新变更见 [CHANGELOG.md](../CHANGELOG.md)。


> 创建日期: 2026-06-14
> 对应设计: [`2026-06-14-time-sliced-intervention-design.md`](./plans/2026-06-14-time-sliced-intervention-design.md)
> 用途: 分时架构实施后的验收检查清单。**所有项必须通过才能标记为已验收。**

---

## A. 自动化测试（必须全绿）

| # | 检查项 | 命令 | 预期 | 状态 |
|---|---|---|---|---|
| A1 | Rust 单元测试 | `cargo test --manifest-path src-tauri/Cargo.toml --lib` | 387 通过，0 失败 | ☐ |
| A2 | Rust clippy | `cargo clippy --manifest-path src-tauri/Cargo.toml` | 零错误（warning 可接受） | ☐ |
| A3 | TypeScript 类型检查 | `npx tsc --noEmit`（src-frontend 下） | 零错误 | ☐ |
| A4 | WriteTimeBundle 测试 | `cargo test --lib creative_engine::write_time_bundle` | 12 通过 | ☐ |
| A5 | QuickPreflightChecker 测试 | `cargo test --lib story_system::preflight` | 3 通过 | ☐ |
| A6 | AuditExecutor 测试 | `cargo test --lib task_system::audit_executor` | 9 通过 | ☐ |
| A7 | InsightExecutor 测试 | `cargo test --lib task_system::insight_executor` | 2 通过 | ☐ |
| A8 | 端到端集成测试 | `cargo test --lib task_system::e2e_tests` | 4 通过 | ☐ |

---

## B. 时间线 1：写作时刻（手动验证）

**前置条件**：配置好至少一个 LLM 模型端点（Settings → 模型管理）。

| # | 检查项 | 操作 | 预期 | 状态 |
|---|---|---|---|---|
| B1 | 普通生成走 TimeSliced | 幕前输入指令生成正文 | 生成耗时显著低于旧版 Full（目标 < 15s） | ☐ |
| B2 | auto_write 走 TimeSliced | 幕前触发文思"自动续写" | 同上，快速返回 | ☐ |
| B3 | QuickPreflight 拦截空角色 | 新建故事（无角色）后生成 | 报错"请先创建至少一个角色"，不触发 auto_contract | ☐ |
| B4 | 红线突出注入生效 | 在有 MASTER_SETTING 合同的故事生成 | 生成的正文遵守世界观红线（无硬伤） | ☐ |
| B5 | 题材自适应：都市类含风格片段 | 都市题材故事生成 | 正文风格细腻度优于纯最小约束 | ☐ |
| B6 | 题材自适应：玄幻类不含风格片段 | 玄幻题材故事生成 | 红线守严，速度不受风格约束拖慢 | ☐ |
| B7 | Full 模式仍可用于向导 | 幕后 NovelCreationWizard 创建故事 | 走 Full 同步路径，质量正常 | ☐ |

---

## C. 时间线 2：审计时刻（手动验证）

| # | 检查项 | 操作 | 预期 | 状态 |
|---|---|---|---|---|
| C1 | 后台审计自动触发 | B1/B2 生成正文后等待 90s | 编辑器内出现 ai_audit 类型标注 | ☐ |
| C2 | annotation 按severity着色 | 观察 C1 产生的标注 | high=红、medium=琥珀、low=蓝 | ☐ |
| C3 | memory 维度优先 | 检查标注顺序 | memory 维度的标注先出现 | ☐ |
| C4 | 标注可处置 | 点击标注处置 | accept/reject/ignore 都能调通后端 | ☐ |
| C5 | 连续生成不堆积 | 快速连续生成 3 次 | 只审计最新版（旧的被取消/覆盖） | ☐ |
| C6 | 审计失败不影响用户 | 故意断开 LLM 端点后生成 | 正文正常返回，后台审计静默失败 | ☐ |

---

## D. 时间线 3：洞察时刻（手动验证）

| # | 检查项 | 操作 | 预期 | 状态 |
|---|---|---|---|---|
| D1 | 每 5 段自动触发 | 连续生成 5 段正文 | story_summaries 出现 deep_insight 记录 | ☐ |
| D2 | NarrativeAnalysis 页展示报告 | 打开幕后"叙事分析"页 | 可见"深度洞察"section（健康度+趋势+债务） | ☐ |
| D3 | 健康度评分合理 | 检查 D2 的健康度 | 分数在 0-100，且有颜色分级（绿/琥珀/红） | ☐ |
| D4 | 条件触发不频繁 | 生成第 3 段后检查 | 不应触发（距上次 < 5 段） | ☐ |

---

## E. 债务指示器与引导（手动验证）

| # | 检查项 | 操作 | 预期 | 状态 |
|---|---|---|---|---|
| E1 | DebtIndicator 显示计数 | 有未处理 annotation 时 | 顶栏显示数字 + 颜色（有 high 则琥珀/红） | ☐ |
| E2 | DebtIndicator 超阈值警告 | 累积 > 10 条 high 或 > 30 条总计 | 指示器变红色 | ☐ |
| E3 | DebtIndicator 点击跳转 | 点击指示器 | 跳转到幕后工作室 | ☐ |
| E4 | 无债务时不显示 | 处置完所有 annotation | 指示器消失 | ☐ |
| E5 | 首次引导 toast | 清除 localStorage 后首次出现 annotation | 显示引导提示（6 秒后消失） | ☐ |
| E6 | 引导只出现一次 | 第二次出现 annotation | 不再显示引导 | ☐ |

---

## F. 回归验证（确保不破坏现有功能）

| # | 检查项 | 操作 | 预期 | 状态 |
|---|---|---|---|---|
| F1 | Full 模式仍正常工作 | 用 / 指令或向导触发 Full | 同步审计 + Rewrite 正常 | ☐ |
| F2 | Ghost Text 仍正常 | 幕前文思活跃模式 | Fast 模式补全正常 | ☐ |
| F3 | 现有 annotation 功能不受影响 | 手动创建 note/todo/warning 标注 | 正常创建和显示 | ☐ |
| F4 | 幕后 SceneAnnotationPanel 正常 | 幕后打开场景编辑器批注面板 | 能查看和处置 ai_audit 标注 | ☐ |
| F5 | Pipeline 三段流程不受影响 | 幕后 SceneEditor 跑 refine/review/finalize | 正常执行 | ☐ |

---

## G. 性能基线（可选但建议）

| # | 检查项 | 操作 | 预期 | 状态 |
|---|---|---|---|---|
| G1 | TimeSliced vs Full 耗时对比 | 同一故事同一指令各生成一次 | TimeSliced 耗时 < Full 的 50% | ☐ |
| G2 | annotation 回流延迟 | 从正文返回到标注出现 | < 120s（含 Inspector LLM 调用） | ☐ |
| G3 | 长篇（50+ 章）内存稳定 | 连续生成 50 段 | 无内存泄漏或持续增长 | ☐ |

---

## 验收签字

- [ ] A 类（自动化）全绿
- [ ] B 类（时间线 1）通过
- [ ] C 类（时间线 2）通过
- [ ] D 类（时间线 3）通过
- [ ] E 类（债务指示器）通过
- [ ] F 类（回归）无破坏

**验收人**：_______________ **日期**：_______________

---

## 已知限制（验收时需知晓，非缺陷）

1. **annotation 为段落级定位**（非字符级精确高亮）。这是 Phase 0 的设计决策——LLM 给字符偏移不可靠。
2. **InsightExecutor 当前不含 KG/向量检索**。本版聚焦追读力+债务+annotation 汇总。KG/向量增强留作后续。
3. **WriteTimeBundle 的 style_slice 暂未接入 StyleDna**。`load_sync` 的 `style_slice_override` 参数当前传 None。接入 StyleDna 是后续优化项。
4. **审计的 scene_id 当前传 None**（orchestrator spawn 处标了 TODO）。annotation 挂到 story 级而非精确 scene。后续接入 scene_id。
5. **Sample size**：Phase 0 实测仅 3 场景，真实用户故事复杂度更高，质量差距可能波动。需关注 Standard 模式的实际质量反馈。
