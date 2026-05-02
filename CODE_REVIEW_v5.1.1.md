# Code Review: v5.1.1 设计-实现对齐修复

> **Reviewer**: Kimi Code CLI
> **Branch**: `005-align-v5.1`
> **Commits**: `20c6ea3` + `e69354d`
> **Scope**: Rust backend + TypeScript frontend + documentation

---

## Context

本次变更旨在消除 AUDIT_GAP_REPORT_v5.1.md 中识别的 11 项设计-实现差距，重点是 P0 功能断裂修复。

---

## Five-Axis Review

### 1. Correctness ✅ (with comments)

| 文件 | 评估 | 备注 |
|------|------|------|
| `lib.rs` — `auto_ingest_chapter` | ✅ 正确 | 异步后台执行，不阻塞 save 响应；错误处理完整 |
| `lib.rs` — `update_chapter` Ingest 触发 | ✅ 正确 | `tokio::spawn` 包裹，只在 `result.is_ok()` 后触发 |
| `lib.rs` — `state_sync` story_id 修复 | ✅ 正确 | 先查询后操作，删除前保存 story_id |
| `db/repositories_v3.rs` — batch save | ⚠️ 逻辑冗余 | `ON CONFLICT(id)` 永远不会触发（Ingest 每次都生成新 UUID），但无害 |
| `workflow/scheduler.rs` — `run_instance` | ⚠️ 空实现 | 返回 `Ok(())` 但不执行任何节点，调用方可能误解 |

**发现的问题：**

**Important**: `WorkflowScheduler::run_instance` 是空壳。它 `log::info!` 然后直接 `Ok(())`，没有真正遍历和执行 workflow 节点。虽然这是一个渐进式修复（从"完全不做"到"加入队列"），但调用 `execute_next` 的代码会得到一个看似成功的结果，实际上什么都没执行。建议在 `run_instance` 中添加 `todo!("Real node execution not yet implemented")` 或至少返回一个明确的错误信息。

---

### 2. Readability & Simplicity ✅

| 评估项 | 结论 |
|--------|------|
| 命名 | ✅ `auto_ingest_chapter`、`save_entities_batch` 等名称清晰自描述 |
| 注释 | ✅ 关键修改都有 `v5.1.1:` 前缀注释，说明修改原因 |
| 参数顺序 | ⚠️ `emit_*_created` 和 `emit_*_updated` 的 story_id 位置不一致（created 在第二，updated 在最后） |

**发现的问题：**

**Nit**: `FrontstageToolbar.tsx` 第 8 行导入了 `Sparkles` 和 `Settings` 但未使用。这是删除 AI 续写按钮后的残留导入。

**Nit**: `workflow/scheduler.rs` 第 1-2 行导入了 `Workflow` 和 `NodeType`，但在当前实现中未使用。

---

### 3. Architecture ⚠️ (with concerns)

| 评估项 | 结论 |
|--------|------|
| 模块边界 | ⚠️ `auto_ingest_chapter` 放在 `lib.rs` 中，`lib.rs` 已 1700+ 行，增加耦合 |
| 复用性 | ✅ `save_entities_batch` / `save_relations_batch` 是通用的，可被其他地方复用 |
| 设计模式 | ✅ `state_sync` 的修复遵循了"先查询再发射"的已有模式 |

**发现的问题：**

**Important**: `auto_ingest_chapter` 放在 `lib.rs` 中是一个短期务实的选择，但长期应该迁移到 `memory/mod.rs` 或 `creative_engine/` 中。`lib.rs` 应该只保留 Tauri 命令注册和模块导入，业务逻辑应该在专门的模块中。

**Consider**: `WorkflowScheduler` 实现了队列但没有后台 worker。如果未来有代码调用 `schedule_execution`，需要有一个地方定期调用 `execute_next`。当前没有在 `lib.rs` setup 中启动这样的循环。

---

### 4. Security ✅

| 评估项 | 结论 |
|--------|------|
| SQL 注入 | ✅ 所有查询使用参数化 `params![]` |
| 输入验证 | ✅ `auto_ingest_chapter` 有 `content_text.len() < 20` 的短内容过滤 |
|  secrets | ✅ 无硬编码密钥 |

**发现的问题：**

**FYI**: `save_entities_batch` 中 `entity.attributes.to_string()` 直接序列化 LLM 生成的 JSON 存入数据库。这不是 SQL 注入风险（参数化查询防护），但如果 LLM 输出异常大的 JSON 对象，可能导致存储问题。目前 `kg_entities.attributes` 列类型为 TEXT，SQLite 的 TEXT 上限约为 1GB，实际风险极低。

---

### 5. Performance ⚠️ (needs attention)

| 评估项 | 结论 |
|--------|------|
| 异步执行 | ✅ Ingest 在后台 `tokio::spawn` 中执行，不阻塞 save |
| 事务 | ✅ `save_entities_batch` / `save_relations_batch` 使用事务 |
| LLM 调用频率 | ❌ 每次保存都触发，无防抖/去重/速率限制 |

**发现的问题：**

**Critical (性能/成本)**: `auto_ingest_chapter` 每次 `update_chapter` 调用都会触发一次完整的 LLM Ingest Pipeline。如果用户启用自动保存（每 30-60 秒）或频繁手动保存，会产生大量 LLM 调用，导致：
1. **API 成本激增**：每次 Ingest 至少 1-2 次 LLM 调用
2. **用户体验下降**：后台 LLM 队列堆积可能影响其他 AI 功能响应
3. **数据库压力**：频繁写入 `kg_entities` / `kg_relations`

**建议修复**：
```rust
// 在 auto_ingest_chapter 开头添加内容哈希去重或时间冷却
static LAST_INGEST: Mutex<HashMap<String, (String, Instant)>> = Mutex::new(HashMap::new());
// 如果内容哈希未变化，或距离上次 Ingest 不足 5 分钟，跳过
```

**Nit**: `update_character` / `delete_character` 引入了 N+1 查询（先 `get_by_id` 再 `update/delete`）。对于单机 SQLite 应用影响可忽略，但不符合最佳实践。

---

## Dead Code

| 位置 | 说明 | 建议 |
|------|------|------|
| `FrontstageToolbar.tsx:8` | `Sparkles, Settings` 导入未使用 | 删除 |
| `workflow/scheduler.rs:1-2` | `Workflow, NodeType` 导入未使用 | 删除 |

---

## Verification

- [x] `cargo check` — 零错误 ✅
- [x] `cargo test` — 193/193 通过 ✅
- [x] `npm run build` — 通过 ✅
- [x] `cargo tauri build` — Windows `.exe` + `.msi` + `-setup.exe` 生成 ✅
- [ ] 缺少针对 `auto_ingest_chapter` 的单元测试
- [ ] 缺少针对 `save_entities_batch` / `save_relations_batch` 的单元测试

---

## Summary & Verdict

### 改进之处
1. **P0 断裂修复扎实**：`update_chapter` Ingest 触发、`state_sync` story_id、FrontstageToolbar story_id 传递都是正确且必要的修复
2. **事务安全**：批量保存使用 SQLite 事务，保证原子性
3. **错误处理**：`auto_ingest_chapter` 的所有失败路径都有 `log::warn!` 记录，不会 panic
4. **最小侵入**：修改遵循已有代码模式，没有引入新依赖或新架构

### 需要处理的问题

| 优先级 | 问题 | 建议行动 |
|--------|------|----------|
| **P1** | `auto_ingest_chapter` 每次 save 都触发 LLM，无频率控制 | 添加内容哈希去重或 5 分钟冷却期 |
| **P2** | `WorkflowScheduler::run_instance` 空实现 | 返回 `Err("Node execution not yet implemented")` 而非 `Ok(())` |
| **P2** | `lib.rs` 越来越臃肿 | 创建 `memory/auto_ingest.rs` 迁移 `auto_ingest_chapter` |
| **Nit** | 未使用的导入 | 删除 `Sparkles`、`Settings`、`Workflow`、`NodeType` |

### 裁决

**Approve with comments** — 代码正确且改善了系统健康度，可以合并到 `master`。但强烈建议在合并前或合并后的下一个迭代中解决 **P1 的 LLM 频率控制问题**，否则生产环境中频繁保存会导致 API 成本失控。
