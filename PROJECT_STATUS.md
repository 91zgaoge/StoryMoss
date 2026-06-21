# StoryForge (草苔) v0.22.2 项目完成状态

> 最后更新: 2026-06-21（v0.22.2 提示词与后台资产深度结合 + 全面文档更新）
> GitHub: https://github.com/91zgaoge/StoryForge

---

## ✅ 最近完成功能

### v0.22.2 — 素材推荐种子 + 策略硬约束（2026-06-21）

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
| `cargo test --lib intention_graph` | ✅ 18/18 |
| `cargo test --lib adaptive::asset_params` | ✅ 3/3 |
| 真实模型测试（Gemma4-e2b） | ✅ 6/6 |
| `npx tsc --noEmit` | ✅ 零错误 |
| `cargo +nightly fmt -- --check` | ✅ 零差异 |
| `prettier --check` | ✅ 零差异 |

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
