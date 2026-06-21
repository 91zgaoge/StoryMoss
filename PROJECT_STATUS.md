# StoryForge (草苔) v0.23.0 项目完成状态

> 最后更新: 2026-06-21（v0.23.0 TriShot 三击生成管线 + 后台智能改写）
> GitHub: https://github.com/91zgaoge/StoryForge

---

## ✅ 最近完成功能

### v0.23.0 — TriShot 三击生成管线：关键路径压缩至最多 3 次 LLM（2026-06-21）

- 🎯 **TriShot 三击管线**：新增 `GenerationMode::TriShot`（三击），Call 1 最快模型选资产+合成提示词 → Call 2(可选) 精修 → Call 3 Writer 生成。质检/改写/入库/洞察全部下沉后台静默执行
- ⚡ **快速模型选取**：`GatewayExecutor::select_fastest_profile()` 按算力档案 TTFB 升序选最快可用模型，`LlmService::generate_with_fastest()` 捷径
- 🧩 **prompt_synthesis 模块**：`AssetManifest` 把 ~17 段资产打包为紧凑清单（4000 字符预算）+ `PromptSynthesizer` JSON 结构输出 + `PromptRefiner` 可选精修（预算守卫跳过）
- 🏎️ **PlanExecutor 快速路径**：TriShot 跳过 SING/PlanGenerator，`PlanStep::long_running` 跳过 90s 步超时
- 🤖 **BGP-2 智能改写**：`auto_rewrite_executor.rs` 按严重度分流——HIGH 自动改写+可撤销，LOW 仅建议
- 📡 **SyncEvent 扩展**：`ContentAutoRevised`（toast 通知）+ `RevisionSuggested`（审阅面板）
- 🖥️ **前端配置**：设置页面新增「三击模式」下拉选项
- ✅ **验证**：`cargo check` 零错误；`cargo test --lib` 486 passed（新增 TriShot 19 测试全部通过，零回归）；`npx tsc --noEmit` 零错误

### v0.22.4 — 「异星球末世生存」智能创作流程优化 + 后台资产审计（2026-06-21）

- 🧩 **GenreResolver 题材解析**：精确/别名/子串/同义词/复合题材解析，解决自然语言题材词断链
- 🗺️ **意图图资产发现增强**：`AssetNode` tags + `discover_assets` 复合题材补充发现
- 🌉 **模型网关资产感知调度**：`asset_tags`/`discovered_asset_ids` 全链路透传，任务类别按标签校准
- ✍️ **TimeSliced 复合题材补强**：`secondary_genre_profile_strategy` 注入次要题材画像
- 📋 **后台资产全面审计**：新增 `docs/CREATIVE_ASSETS_AUDIT_v0.22.4.md`，梳理全部 22 类创作资产、智能创作流程注入点、12 项断链/断环问题与 10 条修复建议
- 🗺️ **项目流程图技术文档**：新增 `docs/PROJECT_PROCESS_FLOWCHARTS_v0.22.4.md`，覆盖创世、拆书、智能创作主路径、79+ 提示词、43 个网文题材模板、40+ 创意资产、Story System 全子系统流程图
- 🏗️ **架构审计报告**：新增 `docs/BROOKS_LINT_ARCHITECTURE_AUDIT_v0.22.4.md`，模块依赖图 + 6 大 decay risks 诊断，Health Score 18/100
- ✅ **验证**：新增 targeted tests 39 passed；`cargo check` 零错误；`npx tsc` 零错误

### v0.22.3 — 钥匙串彻底移除 + 模型健康报告自动刷新（2026-06-21）

- 🔐 **钥匙串彻底移除**：删除 keyring crate、secure_storage 模块、store_api_keys_securely 配置项
- 🧹 **移除 ~260 行钥匙串读写逻辑**：load/save 中全部钥匙串访问已清除
- 📊 **模型健康报告自动刷新**：前端每 30 秒自动刷新，后端改为 async
- ⚡ **冗余 load 消除**：execute_writer 2→1 次、FirstChapterGenerationStep 3→1 次
- ✅ **零回归**：cargo check 零错误，425 passed，tsc 零错误

- GenreProfile 推荐资产种子：7 个题材写入推荐风格/方法论/技能
- 策略选择硬约束：体裁画像有推荐时跳过 LLM 直接使用
- 算力档案默认值修正：capability_score 未测试时默认 0.0

### v0.22.1 — 5 条建设性意见（2026-06-21）

- StrategySelector 题材推荐映射：7 种题材→风格推荐
- StyleDNA 句长偏差检测：>30% 偏差记录建议
- Inspector 方法论动态 prompt：5 种方法论全覆盖
- GenreProfile 推荐字段：4 新列 + Migration 96
- 算力档案质量分权重：HeavyCreation→quality80%

### v0.22.0 — 提示词与后台资产深度结合（2026-06-21）

- Phase A：TimeSliced 路径全资产注入（StyleDNA六维+方法论+题材画像+策略）
- Phase B：Inspector 全资产注入（体裁画像+角色状态+冲突+四元组）
- Phase C：意图感知调度接线（agent_type→intent 自动推导）
- Phase D：算力档案消费闭环（TTFB/TPS 参与候选排序）
- Phase E：资产→生成参数规则映射（asset_params.rs）

### v0.21.0 — 提示词全量可配置化（2026-06-21）

- 79 个提示词全部前端可编辑（21 个分类）
- 假接入修复：15 个 key 改为 resolve_prompt（含 DB 覆盖）
- 旁路接线：40+ 个硬编码提示词全部接入 registry
- 前端 Monaco 编辑器 + 批量导入导出

---

## 🔧 编译状态

| 检查项 | 状态 |
|--------|------|
| `cargo check` | ✅ 零错误 |
| `cargo test --lib intention_graph` | ✅ 21/21 |
| `cargo test --lib adaptive::asset_params` | ✅ 3/3 |
| `cargo test --lib genre_resolver` | ✅ 5/5 |
| `cargo test --lib selector` | ✅ 6/6 |
| `cargo test --lib write_time_bundle` | ✅ 13/13 |
| `cargo test --lib dispatcher` | ✅ 5/5 |
| 真实模型测试（Gemma4-e2b） | ✅ 6/6 |
| `npx tsc --noEmit` | ✅ 零错误 |
| `cargo +nightly fmt -- --check` | ✅ 零差异 |
| `prettier --check` | ✅ 零差异 |
| 后台资产审计 | ✅ 完成，见 `docs/CREATIVE_ASSETS_AUDIT_v0.22.4.md` |
| 已知测试失败 | ⚠️ `test_load_all_assets_integration`（V092 基线）、`test_build_prompt_extension_scene_structure`（注册表与代码 fallback 不一致）|

---

## 📊 提示词覆盖统计

| 类别 | 数量 | 状态 |
|------|------|------|
| Writer/Inspector/Commentator | 5 | ✅ 全部可覆盖 |
| Planner/Analyzer | 4 | ✅ 全部可覆盖 |
| Pipeline（审稿/修稿/后处理） | 4 | ✅ v0.22.0 新增 |
| Audit（质量审计） | 1 | ✅ v0.22.0 新增 |
| Intent（意图解析） | 1 | ✅ v0.22.0 新增 |
| Deconstruction（拆书） | 5 | ✅ v0.22.0 新增 |
| Creation（创世流程） | 14 | ✅ v0.22.0 新增 |
| Strategy（策略选择） | 1 | ✅ v0.22.0 新增 |
| Methodology（方法论） | 19 | ✅ 全部可覆盖 |
| Skill（技能） | 5 | ✅ 全部可覆盖 |
| Memory/Knowledge/Probe | 7 | ✅ 全部可覆盖 |
| Narrative（叙事） | 2 | ✅ 全部可覆盖 |
| World/Character（世界/角色） | 6 | ✅ 全部可覆盖 |
| System/Other | 5 | ✅ 全部可覆盖 |
| **总计** | **79** | ✅ |
