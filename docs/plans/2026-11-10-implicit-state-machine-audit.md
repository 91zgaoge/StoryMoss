# StoryMoss 隐式状态机清查与治理方案

> 2026-11-10 · 只读全仓审计（Rust 612 文件 + 前端 stores/hooks/pages/components/frontstage）
> 依据用户三条永久原则：①找全隐式状态机，收敛为更少更深的模块与更小接口；②数据入口校验一次后直接使用；③分析透彻、建议大胆。
> 本文是**决策与实施蓝图**，审计期间未修改任何代码。
>
> **实施状态**：v0.95.0 落地 Phase 0–4 与订阅门控；v0.96.0 收口租约续期/回收、agency waiting/stale、V158 状态触发器、正文 CAS、入口互锁、TrainingStage、auth 死模块、窗口/automation/叙事阶段、布局单源；v0.97.0 完成最后两项——V159 `editor_gate_records.run_id` + `apply_gate_records_to_run` 使 run.result/board/gate record 同源，`navigation/views.ts` + `viewComponents.tsx` 使 Nav/页面/布局同源。详见 CHANGELOG v0.95.0–v0.97.0。

---

## 0. 执行摘要

全仓识别出 **51 个隐式状态机条目**（同一实体/流程的状态散落在多个字段、store、DB 列、事件与日志中），另有 **17 个声明但无写入者/无消费者的死状态机或死字段**。其中：

- **P0（数据丢失/安全/功能永久卡死）约 10 个条目**
- **P1（不可能状态/错误裁决/静默失败）约 24 个条目**
- **P2–P3（展示错位、死状态、歧义）其余**

最重要的结论不是「有 51 个 bug」，而是只有 **8 类系统性反模式**在反复制造它们。逐个修 51 个点会把同一批坑复制到下一处；最佳决定是：

> **建立 4 个深模块 + 删除 17 个死状态机，其余所有状态一律走这 4 个模块的窄接口。**
>
> 1. Rust `lifecycle`：所有持久化实体的唯一状态所有者（任务/run/agent task/聊天消息/导演动作/训练/租约）。
> 2. Rust `cancel`：唯一取消注册表 + RAII 作用域，替换 4 套互不相通的取消机制。
> 3. 前端 `generation` 协议边界：`state/generation.ts` 接线为唯一解释器，一套 reducer 管后台活动。
> 4. 前端 `session`：故事/章节/场景/幽灵文本/保存状态的单一文档会话机，所有切换用 epoch 票据。

这不是把状态机「抽象得更漂亮」，而是把 **唯一所有权、显式转移、终态不可覆盖、取消必可达** 变成类型与 SQL 层面的硬约束，让下一次改动无法再绕过。

---

## 1. 全量清单

### 1.1 Rust 持久化生命周期（P0 集中区）

| ID | 隐式状态机 | 状态载体（证据） | 可达的非法/歧义状态 | 风险 |
|---|---|---|---|---|
| L1 | **Task 生命周期** | `TaskStatus{Pending,Running,Completed,Failed,Cancelled}` `task_system/models.rs:10-18`；DB `status/progress/retry_count/last_heartbeat_at` `db/connection.rs:265-286`；写入 `executor.rs:79-86,136-164`、`repository.rs:308-330`、`service.rs:201-264`、`heartbeat.rs:101-195` | `cancelled→running/completed/failed`（取消只写 DB，执行器继续无条件写终态）；`completed/failed→running`（手动触发只拒绝 Running）；`progress` 回调强制把状态改回 running（`repository.rs:346-350`）；同一任务两个执行实例（scheduler 重建锁）；`pending+progress>0` 永不执行；心跳失败覆盖仍活着的执行器 | **P0** |
| L2 | **Agency Run 生命周期** | `agency_runs.status/phase/lane` 裸 String `db/migrations/V107__agency_tables.sql:6-7`、`V149:26-31`；`models.rs:128-161`；终态守护 `repository.rs:57-86`；取消表 `coordinator.rs:68-105`；收尾 `7643-7655`；命令 `commands.rs:263-300` | `cancelled` 的 run 仍装配场景、发 `completed` 事件与 activity log；`completed` + `result.verdict=revise/failed`；`failed` run 继续跑全批；`update_run_phase/finish_run` 静默 0 行仍返回 Ok；resume 与创建 run 无原子约束；write lane 上限 3 是 TOCTOU | **P0** |
| L3 | **Agency Agent Task 生命周期** | `agency_agent_tasks.status` 无默认/无 CHECK `V149:47-67`；状态域 `models.rs:281-312`；转移 `task_repository.rs:92-164`、`scheduler.rs:258-314` | `cancelled→cancelling`；`completed→cancelling`；`queued` 因信号量/启动失败永久滞留；`waiting`/`stale` 声明但无写入点；`finish` 无终态守卫；重试链无聚合终态 | P1 |
| L4 | **Chat Message + Director Action 生命周期** | `agency_chat_messages.status` `V149:105-119`；`agency_director_actions.status` `V149:123-145`；写入 `chat.rs:101-231`、`director.rs:256-269,536-558,851-889`、`scheduler.rs:922-997`、`chat_commands.rs:221-224` | 占位消息 `acting` 成功后永不转 `done`（成功只追加新消息）；chat=`done` 但 action=`proposed`；原 action=`approved` 与新建 `executing` 两行并存；已 done 的动作可再被 `rejected`；`actions_json` 在 run 结束后仍渲染审批卡；`proposed.run_id` 恒 NULL | **P0/P1** |
| L5 | **租约所有权** | `story_resource_leases.expires_at`（无 status）`V149:75-88`；`lease.rs:43-146`；`LeaseGuard` Drop `coordinator.rs:9426-9437` | 租约过期但 run 仍活着；`failed` task 仍持有租约；`heartbeat`/`reap_expired` 无生产调用点；进程死亡后所有权被第二个 run 直接覆盖 | P1 |
| L6 | **续写训练阶段** | `continuation_training.rs:224-516` 固定里程碑 5/20/30/35/40/75/90/100；报告表无 status/FK `V157:21-38`；判官降级 `1030-1065`；沙盒清理 `1172-1200` | `progress=100` 但资产重建失败/判分 null；报告已落库但 task=running/failed；报告指向已被 cleanup 删除的沙盒；取消返回 `success:false` 后被 TaskService 判 failed | P1 |
| L7 | **Gate/Verdict 生命周期** | `GateOutcome{Passed,RevisionRequired,Failed}` `gate.rs:20-33`；黑板条目 `board.rs:117-128`、`continue_loop.rs:117-253`；`editor_gate_records.outcome` `V146:29-44`；三处裁决 `coordinator.rs:6937-7072,9161-9209` | 首轮 revise gate 永远停在 active；completed run 携带 revise/failed verdict；run.result=pending + gate_record=failed + QC 事件第四份副本；历史 revise 跨 run 注入下一轮 | P1 |
| L8 | **取消机制碎片** | `task_system` CancelMonitor（RAII）`executor.rs:196`；agency `AGENCY_CANCEL_FLAGS` + 附属表；narrative `PIPELINE_CANCEL_FLAGS`；LLM cancel map + 墓碑集合 | 取消只写 DB 不中断执行；批量/导演路径用外部 flag，`cancel_run` 对它们 no-op；pipeline flag 在 abort/panic 后永久残留；死代码路径复用固定 request_id 被误判已取消 | **P0** |

### 1.2 Rust 其他子系统（P1/P2 主体）

| ID | 隐式状态机 | 状态载体（证据） | 可达的非法/歧义状态 | 风险 |
|---|---|---|---|---|
| S1 | **MCP 连接/权限/会话** | DB `enabled` + `MCP_CONNECTIONS` + `CapabilityRegistry` 三源 `commands/mcp.rs:33-70,306-340`；`permission_for_tool` 缺注解→Read `mapping.rs:302-337`；SSE 响应队列无 id 关联 `transport.rs:137,275-310,495-510`；stdio `send` 在 stdin=None 时静默 Ok `transport.rs:79-95` | enabled+无连接+能力空；能力先注册后写连接 map；未声明注解的写工具按只读重试（副作用重复）；SSE 一条多条响应错配给下一请求；stdio 关闭后 send 假成功 | **P0**（安全） |
| S2 | **Model Gateway 健康态** | 内存 `HealthRegistry` + DB `model_capability_profile.status` + `GatewayStatus` + audit 字符串四源；`health.rs:65-301`、`capability_store.rs:23-147`、`executor.rs:397-493,1199-1478` | DB unhealthy 与内存 Healthy 并存；Unknown 被三种“可用性”函数各判一种；`is_probing=false`/`primary_model_id=None` 是伪状态；两个失败计数器语义重叠 | P1 |
| S3 | **LLM 取消与墓碑** | `cancel_senders: Mutex<HashMap<String, Option<Sender>>>`、`cancelled_requests: Mutex<HashSet>` `llm/service.rs:401-407,1885-2745` | 同步生成取消后墓碑永不清理；固定 request_id（探针）复用被误判取消；`Option<Sender>` None 语义未定义 | P1 |
| S4 | **章节投影健康** | `projection_status` JSON `commit_service.rs:360-499`；二次解释 `contract_service.rs:196-208` | `"success (precomputed)"` 被判 unhealthy；`kg/narrative` 键被忽略；JSON 缺失的 unknown 与真实未知不可区分 | P1 |
| S5 | **日志包/上传态** | `PackageInfo.filename/skipped_duplicates/path` `log_report.rs:44-52,185-206`；自动上传时间戳 `539-626`；并发 tmp `.tmp-<pid>` `129-130` | 重复包返回空 filename 但 path 指向未生成文件；并发打包互相覆盖；`skipped_duplicates` 与空 filename/path 三重表达 | P1 |
| S6 | **Updater 流程** | 两源遍历 `updater/mod.rs:160-185`；下载/安装事件 `187-213,324-345`；`CheckUpdateResult` 三字段 | 第一源 `Ok(None)` 立即 return，第二源永不检查；`has_update=true` + `latest_version=null`；下载完成事件早于安装成功 | P1 |
| S7 | **拆书/指导书状态** | `AnalysisStatus` 6 态 `book_deconstruction/models.rs:10-46`；`DistillationStatus` 7 态；去重 `service.rs:69-75,673-678` | 同文件 hash 命中 failed/cancelled 也返回旧 id（无法重试）；`Extracting/Merging` 只在事件中，DB 无写入者；未知状态解析回 Pending | P1 |
| S8 | **LanceDB 初始化态** | `db: RwLock<Option<Connection>>` + `table: RwLock<Option<Table>>` `vector/lancedb_store.rs:56-230` | table Some / db None 的半初始化；并发 init 双开并互 drop；初始化失败永久不可用 | P1 |
| S9 | **Post-process 状态** | `PostProcessStatus{Running,Completed,Failed,Partial}`；解析 `repositories_pipeline.rs:981,1020`；收尾 `finalize.rs:207-263` | 损坏状态字符串→Running；`Partial` 永不产生；`completed_at` 已写而 status 仍 Running | P1 |
| S10 | **自动重算/风格学习运行集合** | `INFLIGHT: Mutex<HashSet>` `recompute.rs:743-825`；`RUNNING` + `PENDING` `style_learning.rs:327-380` | panic/abort 后永久“正在运行”；后台闸门失败路径 `break` 不释放 RUNNING；重启清零但 DB 标记仍在 | **P0**（功能永久失效） |
| S11 | **订阅/身份态** | `SubscriptionStatus.tier/status/expires_at` `subscription/mod.rs:17-98`；`Identity{Account,Device}` `identity.rs:17-41`；死 OAuth `auth/oauth.rs:23-112` | 过期 Pro / `status=cancelled` 仍解锁全部功能（只看 tier）；空 machine_id 生成 user_id="" 订阅；OAuth state 无 TTL | P1（商业） |
| S12 | **窗口/自动化/叙事阶段** | `WindowState` 两 bool `window/mod.rs:9-99`；`is_processing/is_shutdown` `automation/service.rs:34-343`；`NarrativePhase` 启发式 `canonical_state/mod.rs:82-90,477-518` | closed 与 hidden 不可区分；shutdown 后已入队事件未定义；同一 story 两次快照可得不同 phase | P2 |
| S13 | **资产刷新草稿** | `AssetRefreshDraft` 无 id/status/expiry `planner/executor.rs:38-75`；确认前已写 Characters/World `asset_refresh.rs:1054-1086` | 用户取消确认但资产已部分落库；草稿无幂等键，两次执行互相覆盖 | P1 |
| S14 | **创世/Bootstrap 死状态机** | `GenesisRunRepository` 全 `#[allow(dead_code)]` `db/repositories/genesis_run_repository.rs:9-136`；`BootstrapSession` 无写入者 `planner/bootstrap.rs:11-35`；`PipelineProgressEvent.status: String` | UI 读永不变的 pending/running；`current_step=None`、`steps_json="{}"`；真实创世状态在 agency，仪表盘展示另一套 | **P0（展示可信度）** |
| S15 | **Scene.execution_stage** | `Option<String>` 注释声明 5 态 `db/models.rs:51`；写入 `planner/mod.rs:905`、`auto_contract.rs:842`、`planner/executor.rs:2535`（`needs_rewrite` 未声明） | 任意字符串可落库；None→`"unknown"` 双哨兵；UI 与 draft.status/phase 各一套 | P2 |
| S16 | **零散状态** | `pending_review` 非法输入自动 confirmed `pending_review.rs:105-121`；`KbImportResult` 全失败仍 Ok `knowledge_base/mod.rs:101-156`；`ai_operations.status` 创建即 success `repositories_export.rs:196-227`；`SCAN_MEMO` TTL + `-1` 章号 `life_status.rs:150-190` | 拼错状态被当作“已确认”；0 向量仍报成功；失败操作永久 success；撤销后 5 分钟漏扫 | P2 |

### 1.3 前端状态容器与跨窗口（UX 症状最集中）

| ID | 隐式状态机 | 状态载体（证据） | 可达的非法/歧义状态 | 风险 |
|---|---|---|---|---|
| F1 | **生成状态协议 + 后台活动聚合** | 后端 `phase` 发中文 `events.rs:29-60`；hook 用英文终态 `useBackendActivityListener.ts:233-238`；FrontstageApp 用中文 `1902-1908`；唯一解释器 `state/generation.ts` 已写但零生产引用；hook 捕获旧快照 `155` 再读 `store.activities`；两张优先级表 `hook:83-92` vs `backendActivityStore.ts:50-59` | 成功终态被 hook 判 running→底栏永久转圈；`isGenerating=false` 与 primary=running 并存；优先级守卫永不生效；完成/心跳找不到 existing 活动；同一 payload 被两个消费者各写一套 | **P0** |
| F2 | **故事/章节/场景选择会话** | `currentStory/chapters/scenes/currentChapter` `FrontstageApp.tsx:254-259`；`switchSeqRef/lastAppliedStorySeqRef/ignoredStoryEventsRef` `272-276`；一次性旗标 `suppressChapterSwitchFlushRef/skipChapterContentRef/lazyLoadingChapterIdsRef` `284-299`；`selectChapter 2825-3156`、`selectStory 3219-3224`、`switchStory 3284-3412` | 快速点 A 再点 B，谁后完成谁写 `currentChapter`；分章抑制旗标被另一章消费；切书失败后旧故事粘在 ignored；`resetStoryScopedUiState` 不清 `sceneInfo`；跨故事 ContentUpdate 穿透写到旧 scene；同章懒加载在途时点击被静默吞掉 | **P0** |
| F3 | **幽灵文本生命周期** | 父 `generatedText/hideGhostUntil/postAcceptLockRef` `363-369`；子 `isHidingGhost/postAcceptHideUntilRef` `RichTextEditor.tsx:256-265`；打字机 `typewriterFrameRef` `1416`；`editorContainsGeneratedText` 80% 反推 `1195-1233` | Esc 只清文本未取消打字机→下一帧幽灵复活；Tab 在 partial 时接受截断输出；幽灵不可见但操作提示可见；30s 渲染锁与 300s 写入锁表达同一事实 | P1 |
| F4 | **场景保存状态** | `isSaved` store `frontstageStore.ts:27-65`；`saveError/saveFailedRef/saveChainRef/loadGuardRef` `useScenePersistence.ts:81-208`；`justSavedRef` `1418`；顶栏三分支 `FrontstageHeader.tsx:451-469` | `skipContent` 路径不置 isSaved→顶栏长期“保存中”假死；切书后 saveError 残留；失败重试持有旧正文；空串保存被静默跳过；无版本/CAS，幕前旧快照覆盖幕后新稿 | P1（数据丢失） |
| F5 | **生成入口/取消/自动续写** | `isGenerating`（generationStore）+ `isGeneratingRef` + `smartExecuteInFlightRef` + `keepGeneratingForActiveRunRef` + `lastGenerationCancelledRef` + `pendingAutoContinueRef` `FrontstageApp.tsx:1434-1481`；入口 `3884-3945`、取消 `4666-4710`、智能入口 `4741-4969` | 两个 Ctrl+Enter 同周期通过入口→双生成/双创世；取消后成功结果仍追加并覆盖“已取消”；文思 active 取消后立刻复活；冲突路径留下无主 isGenerating=true | P1 |
| F6 | **跨窗口故事选择** | 幕前 `currentStory`、幕后 `appStore.currentStory`、后端 `app_settings` 三源；seq/legacy `useSyncStore.ts:176-182`、`state_sync/service.rs:15-35,112-127`；App 自动切 `267-343` | 后台自动切故事不广播，幕前仍旧故事；`backstage-shown` 先于列表刷新选中错故事；挂载恢复无 seq 覆盖较新选择；legacy seq=0 绕过排序 | P1 |
| F7 | **编辑器配置/主题同步** | localStorage + appStore + 组件 state + Tauri 事件 `useEditorConfig.ts:10-90`；主题 `colorThemes.ts:458-491` | 双通道后到的旧事件回滚新配置；同窗口自我回环重复应用；localStorage 异常时 store/emit 分叉 | P2 |
| F8 | **剧组对话浮层会话** | `sessionId/targetRole/busy` `FrontstageChatOverlay.tsx:45-90`；sessions[0] 推断当前会话 | 切故事后仍用旧 sessionId，发送 story B + session A；浮层内 Ctrl+Enter 仍可触发续写，chat busy 与 isGenerating 可同真 | P2 |
| F9 | **Auth/模型连接/网络** | authStore 五布尔 `useAuthStore.ts:30-47`；`checkAuth` 无生产调用；`modelConnectionStore.checkingIds` 与 `states.isChecking` 双写 `57-141`；网络状态三份 + 后端 health 第四份 | token 存在但未登录；取消后 isLoading+isWaitingOAuth 矛盾；同模型并发检测；离线门禁为死代码 | P2 |
| F10 | **Bootstrap/agency 活动** | `bootstrapProgress` store、`usePipelineProgress`、`novel-bootstrap-progress` 双消费者 `FrontstageApp.tsx:2478-2525`；延迟清空覆盖新生成；`agencyActivityStore.activeRunId` 由“最后一条事件”推断 | 后台步骤写进前台 bootstrap；旧故事的 agency 事件拉回 activeRunId；hydrate 覆盖实时 run；Header 按 role 混合并发 run | P2 |
| F11 | **视图路由/深链瞬态** | `currentView` 字符串 + `FULL_HEIGHT_VIEWS` + `renderView` 三套集合 `App.tsx:53-526`；`selectedGenesisSessionId/tracingFilter/logsSearchQuery` 只写不读 `appStore.ts:58-87`；深链字段无 TTL | 非法 view 注入；`case 'mcp'` 不可达；切故事后旧 ID 静默清理；pendingQuickCreate 意外触发 | P2–P3 |

### 1.4 前端页面/弹窗流程（重复写与卡死）

| ID | 流程 | 状态载体（证据） | 可达非法状态 | 风险 |
|---|---|---|---|---|
| U1 | 拆书分析 | `liveStatus ?? query.data` `useBookDeconstruction.ts:36-244` | 切书不重置 liveStatus→A 书完成态显示在 B 书；完成后不 invalidate，页面永远“加载中” | **P0** |
| U2 | PromptsPanel 草稿 | `edited` map + 单槽 `savingId` `PromptsPanel.tsx:172-322` | 保存 A 后全量 fetch 覆盖 B/C 未保存草稿；连点两保存互相乱序 | P0（丢草稿） |
| U3 | Tasks 行操作 | `isDeleting` + mutation.isPending 双源 `Tasks.tsx:95-212` | 连点 trigger/retry 重复触发；cancelled 任务在“全部”分组永不显示 | P1 |
| U4 | 技能执行 | `isExecuting/progress/taskId` `SkillExecutionPanel.tsx:107-213` | listen 注册竞态丢完成事件→永久 true；取消与失败同形 | P1 |
| U5 | 审批卡 | `actions_json` 非空即 pending `ChatPanel.tsx:224-250`；`AgencyStudio.tsx:564-593` 无锁 | 确认到轮询刷新之间重复派发；escalate 永久保留审批卡；run 已结束仍可确认 | **P0** |
| U6 | 日志上传弹窗 | `uploadModalOpen/packaging/uploading/packaged` `Logs.tsx:109-170,349-408` | 打包中关闭→完成后回写已关闭弹窗；上传中重开看到第一步且可再打包 | P1 |
| U7 | PipelinePanel | `phase` + `postProcessSteps` 两套阶段 `usePipeline.ts:175-245`；`PipelinePanel.tsx:61` | phase=completed 但后处理仍 running，按钮全部可再点 | P1 |
| U8 | TrainingLab | 多个 mutation pending + `findTrainingTaskByBook` 异步读后再 upsert `TrainingLab.tsx:189-357` | 两个“立即跑”都判定不存在→创建两行；statusTask 回退全局最近训练（显示错书）；报告 Modal 无 error 分支 | P1 |
| U9 | 创世向导 | `step` 枚举 + `isGenerating` + index `NovelCreationWizard.tsx:37-192` | 空选项/旧数组/未校验索引；父 `isCreating` 与子 `isGenerating` 双进度；completed 无提交锁 | P1 |
| U10 | 指导书页 | 列表 status + liveStatus 双源 `GuidebookDistillationPanel.tsx:64-81` | live 完成而列表未完成→卡片完成、结果区不出现；selectedId 指向幽灵 id；结果视图忽略 error | P1 |
| U11 | ExportDialog | `step/isReviewing/reviewResult` `ExportDialog.tsx:43-125`；`isReviewing` 只写不读（死） | reviewing 期间 footer 缺失；关闭后异步写回；审查失败与空文本同落 null | P1 |
| U12 | Scenes/SceneEditor | 父 `selectedSceneId/isEditing/previewTab/compareVersions`；子 `activeTab/execution_stage` `Scenes.tsx:28-133,395-423`、`SceneEditor.tsx:86-219` | 双击创建同序号；切场景保留旧对比层；activeTab 与落库阶段不一致 | P1 |
| U13 | TracingPanel | `selectedTrace/detailLoading` 无请求序号 `TracingPanel.tsx:156-217` | 点 A 再点 B，A 后返回覆盖 B；深链无匹配直接丢弃 | P2 |
| U14 | ModelModal | `isSubmitting/isDetectingContext/contextSource` `ModelModal.tsx:133-292` | 探测阶段在提交锁之外→并行两次 create/update；contextSource 跨模型残留 | P2 |
| U15 | Corpus/Maintenance/风格样例 | `busy` 单字符串；`promoteDraft`；`showStyleSampleInput/isAnalyzingStyle` | applyPromotion 无门→重复建角色/伏笔；关闭后异步结果关闭新弹窗；行级无 pending | P2 |
| U16 | AgencyRuns/MCP/AiPromptBar/confirm | `refetchInterval` 仅 running；`addOpen` 表单残留；`active` 索引越界；confirm Promise 卸载不 settle | pending 创世不再轮询；重开弹窗沿用旧表单；键盘选到越界；路由切换时 `await confirmDialog` 永久挂起 | P2 |

### 1.5 死状态机 / 死字段（直接删除清单）

| # | 载体 | 证据 | 处理 |
|---|---|---|---|
| 1 | `GenesisRunRepository` + `genesis_runs` 读路径 | `genesis_run_repository.rs:9-136`，前端仅 `services/api/genesis.ts:134-137` 包装，生产 UI 已由 AgencyRunsPanel 替代 | 删除 repo/IPC/包装；表迁移标记 deprecated |
| 2 | `BootstrapSession` + `BootstrapProgressEvent` | `planner/bootstrap.rs:11-35` 无写入者 | 删除类型；进度走 generation 协议 |
| 3 | `auth/oauth.rs` + `auth/session.rs` | 自述已迁 server 轮询/JWT | 删除死模块，身份只留 `subscription/identity` |
| 4 | `frontstageStore.isGenerating/setGenerating` | 声明+setter，无调用者 `frontstageStore.ts:29,39,52,93` | 删除 |
| 5 | `ExportDialog.isReviewing` | 只 `setIsReviewing`，无读取 `ExportDialog.tsx:54,91,102,114` | 删除 |
| 6 | `useGenerationStore.resetGeneration` | 无调用者 | 删除 |
| 7 | `useBootstrapStore.resetBootstrapProgress` | 无调用者 | 删除 |
| 8 | `useSyncStore.assertUnreachable` | 未使用 | 删除 |
| 9 | `autoSave.isSaving/getIsSaving` | 只写不读 `autoSave.ts:33-121` | 删除；保存态归 session |
| 10 | `selectedGenesisSessionId` | 只写不读 `TracingPanel.tsx:193` | 删除 |
| 11 | `tracingFilter` 的生产写入者缺失 / `logsSearchQuery` 写者缺失 | `TracingPanel.tsx:216`、`Logs.tsx:63` | 删除字段或补唯一入口 |
| 12 | agency `waiting`/`stale`；chat `thinking`/`waiting`/`degraded` | 声明无写入点 `models.rs:281,362`；`reap_stale` 无生产调用 | 删除或实现为显式状态 |
| 13 | `PostProcessStatus::Partial` | 全仓无写入者 | 删除 |
| 14 | `renderView case 'mcp'` | `setCurrentView('mcp')` 先映射 settings，不可达 `appStore.ts:99-102` | 删除 case |
| 15 | `AiOperation.status='success'` 创建即写 | 失败不更新 `repositories_export.rs:196-227` | 改为显式 pending→success/failed |
| 16 | `PIPELINE_CANCEL_FLAGS`（若拆书外无调用） | 仅拆书 executor 注册，abort 后残留 | 并入 cancel core |
| 17 | `GenesisPanel` 类残留 / `PipelineProgressEvent.status` 字符串 | 与 canonical 步骤表两套词汇 | 并入 generation 协议或删除 |

---

## 2. 只有 8 类根因，不是 51 个独立 bug

1. **终态无守卫 + 取消不中断执行**：Task/Run/Chat/Action 的终态在 DB、内存、事件三处各写一份，谁先谁后决定真相。
   - 典型：`TaskExecutionContext::complete/fail` 无条件 UPDATE（`executor.rs:136-164`）；`cancel_task` 只写 DB（`service.rs:230-264`）；`record_progress` 把所有状态改回 Running（`repository.rs:346-350`）。
2. **同一事实多状态源**：DB 列、内存 map、全局 registry、事件、日志各存一份（MCP 三源、Gateway 四源、Run status/activity log/verdict、Task status/progress/log/heartbeat）。
3. **词汇分裂**：Rust 发中文 label，前端 hook 判英文，FrontstageApp 判中文，`state/generation.ts` 统一解释器已写但未接线（`events.rs:29-60` vs `useBackendActivityListener.ts:233-238`）。
4. **全局一次性旗标/无作用域**：`skipChapterContentRef`、`suppressChapterSwitchFlushRef`、`pendingAutoContinueRef`、`ignoredStoryEventsRef` 会被另一调用消费；批量/导演取消用外部 flag，导致 `cancel_run` 无实际目标。
5. **时间戳反推状态**：心跳、租约、退避、seq、去重历史都在用 `Option<String>` + 时间窗口推断“是否活着/已上传/新选择”，解析失败被当作超时/过期/最新。
6. **声明但不可达的状态**：`waiting/stale/thinking/degraded/Partial/Generating(genesis running)` 只在类型里存在，UI 却按它们渲染。
7. **跨层重复推断**：hook + 组件 + store 各有一套阶段识别/优先级/保存判定；同一 payload 被两处解释，终态一次也不闭合。
8. **无 RAII 的运行集合**：`recompute INFLIGHT`、`style_learning RUNNING`、pipeline flags、LLM 墓碑集合在 panic/abort/break 后永久残留；重启不可自愈或行为不一致。

---

## 3. 最佳决定：4 个深模块，不是 51 个补丁

### 3.1 模块 A — Rust `lifecycle`（唯一持久化状态所有者）

**目标**：任何实体的状态只有一个写入口，终态不可覆盖，状态推进与事件/日志同事务。

```rust
// src-tauri/src/lifecycle/mod.rs —— 对外只有三个方法
pub enum EntityRef { Task(String), Run(String), AgentTask(String), ChatMessage(String), DirectorAction(String) }
pub enum StateId { Queued, Running, Waiting, Succeeded, Failed, Cancelled, Stale }
pub enum Event { Start, Progress(i32), Wait, Resume, Succeed(Finalize), Fail(Cause), Cancel(Cause), Retry, Reap }
pub enum Outcome { Applied(StateId), Idempotent(StateId), Rejected(Terminal), Missing }

pub struct LifecycleStore { pool: DbPool, cancel: CancelHub }
impl LifecycleStore {
    pub fn transition(&self, e: EntityRef, ev: Event) -> Result<Outcome, LifecycleError>;
    pub fn snapshot(&self, e: EntityRef) -> Result<Snapshot, LifecycleError>;
    pub fn cancel(&self, e: EntityRef) -> Result<Outcome, LifecycleError>;
}
```

内部深实现（不在公开接口暴露）：

- `transitions.rs`：一张 `(EntityKind, from, Event, to, guards)` 表；非法转移编译期穷举，运行时返回 `Rejected`，不写库。
- SQL 原子化：`UPDATE ... WHERE id=? AND status IN ('queued','running','waiting')`，**检查 affected rows**；0 行 = 终态/缺失，调用方必须按 `Rejected` 处理，不得再写终态。
- 进度单调：`progress = CASE WHEN ?2 >= progress THEN ?2 ELSE progress END`；`Progress` 绝不改状态（不再把 cancelled/completed 改回 running）。
- 心跳：`Progress/Heartbeat` 由 store 写 `last_heartbeat_at`；HeartbeatMonitor 只调用 `transition(Event::Reap)`，不做第二个状态推断器。
- 重试：`Retry` 只允许 `Running|Failed`，写 `next_run_at`；调度器直接读 store 的 next_run（不再有 DB 侧与 cron 侧两套计算）。
- 租约：`Start` 建立 `LeaseScope`，`Succeed/Fail/Cancel` 在同一个事务内释放；心跳顺延租约；终态后不可能仍有租约。
- 事件/日志：`Applied` 时才发一个终态事件；`Rejected` 时只记一次 warn，不发 completed（消除“DB cancelled + 事件 completed”）。
- DB：`V158` 给 `tasks/agency_runs/agency_agent_tasks/agency_chat_messages/agency_director_actions/editor_gate_records` 加 `CHECK`；未知状态字符串在入口解析时**报错**，不回退 Pending/Running/now。

**为什么不是每实体一套枚举**：状态词汇必须跨实体一致（queued/running/waiting/succeeded/failed/cancelled），否则 agent 与 UI 每加一个实体就多一套翻译表，正是当前 51 个条目的来源。**为什么不建通用状态机库**：外部 crate 会把异步事务、租约、事件、SQL 写入口拆散，DX 反而下降；这里的深模块只需一张表和三个方法。

### 3.2 模块 B — Rust `cancel`（唯一取消注册表）

```rust
pub struct CancelHub { /* map<entity_id, Arc<Token>> */ }
pub struct CancelScope { /* RAII: Drop 注销 */ }
pub struct Token { pub fn is_cancelled(&self) -> bool; pub async fn wait(&self); }

impl CancelHub {
    pub fn scope(&self, entity_id: &str) -> CancelScope; // 注册 + Drop 清理
    pub fn cancel(&self, entity_id: &str) -> bool;       // 置位并通知
}
```

- 替换 `AGENCY_CANCEL_FLAGS`、`PIPELINE_CANCEL_FLAGS`、LLM `cancel_senders` + 墓碑集合、Task `CancelMonitor`。
- `cancel()` 先调用 `lifecycle.cancel(entity)`（DB 终态），再置 token；顺序固定，保证“取消已生效”而不是“只发信号”。
- 所有执行器（task/agency batch/chat revision/LLM 生成/拆书/指导书）只消费 `CancelScope.token()`；RAII 保证 abort/panic 后自动注销，不可能出现“集合里永远有故事 ID”。
- 接口只有 3 个方法（scope/cancel/is_cancelled），不提供 4 套注册/注销函数。

### 3.3 模块 C — 前端 `generation` 协议边界（唯一解释器 + 唯一活动 reducer）

- Rust `GenerationStatusEvent`：`phase` 改为 snake_case id（`preparing_context…`），新增 `label`（中文展示），保留 `message`；旧中文仍可解析（兼容层只在一个函数里）。
- `src-frontend/src/state/generation.ts` 接线为全应用唯一入口：所有 `generation-status` 消费方必须先 `parseGenerationEvent`，只读 `outcome/label/progress`。
- `backendActivityStore` 变成唯一活动 reducer：
  - `apply(event: GenerationEvent)`：同一 `taskId` 一条活动；`done/failed/cancelled` 在同一 action 内闭合主活动（不依赖第二个 store/钩子）。
  - 删除 hook 与 FrontstageApp 的两份 `PRECISE_PHASE_PATTERNS`、删除两份 `CATEGORY_PRIORITY`；优先级只在 store 里一份。
  - 修 `flushPrimary` 读旧快照的 bug：在调用点 `useBackendActivityStore.getState()`，而不是 effect 初次捕获的 `storeRef.current`。
- 看门狗只调用同一个 reducer 的 `timeout()`，不再 `failAllRunning` 直接改状态。
- 接口小：`apply`、`primary`、`timeout`；没有 per-event setter。

### 3.4 模块 D — 前端 `session`（单一文档会话机）

```ts
type GhostState =
  | { phase: 'idle' | 'streaming'; text: string; frame: number | null }
  | { phase: 'complete'; text: string }
  | { phase: 'accepting' }
  | { phase: 'accepted' }
  | { phase: 'rejected' };

type SaveState = 'clean' | 'dirty' | 'saving' | 'failed';

interface DocumentSession {
  storyId: string | null;
  chapterId: string | null;
  sceneId: string | null;
  epoch: number;
  ghost: GhostState;
  save: SaveState;
}
```

- 对外小接口：
  ```ts
  beginChapter(storyId, chapterId): Ticket;   // epoch++
  applyChapter(ticket, chapter, scenes): boolean; // 过期票据直接丢弃
  appendContent(ticket, text): boolean;
  setGhost(ticket, state): boolean;
  save(ticket): Promise<'saved'|'failed'>;
  ```
- 所有异步入口（懒加载、ChapterSwitch、pipeline-complete、ContentUpdate、自动分章）都必须拿 ticket；**删除一次性旗标**（`skipChapterContentRef`、`suppressChapterSwitchFlushRef`、`ignoredStoryEventsRef`、5s quiet period），由 epoch + storyId 判断过期。
- 幽灵：打字机帧由 session 拥有；Tab 只在 `complete` 转移；Esc 转移 `rejected` 并取消帧；不存在第二套 `isHidingGhost/postAcceptLock` 锁。
- 保存：顶栏只从 `SaveState` 派生；`resetStoryScopedUiState` 一并重置 save/ghost/sceneInfo；`update_scene` 增加 `expected_updated_at`（CAS），幕前旧快照不能覆盖幕后新稿。
- 跨窗口故事：后端 active story 为唯一权威；幕前自动切故事也写 active story；`seq` 只由一个后端计数器产生，legacy `seq=0` 按最低优先级处理或删除。

### 3.5 UI 流程：不建“向导框架”，只统一三种转移

- **单次异步动作**（导出/日志上传/技能执行/模型探测）：只保留一个 pending 源（TanStack mutation state），删除 `isReviewing/packaging/isSubmitting` 等重复布尔；需要取消/过期时用同一个 `epoch` 票据。
- **多步向导**（创世/导出/风格 DNA）：`Step` 用 union 枚举 + 穷举 `switch`；禁止数字 step/index 直接下标数组；`canAdvance(step, data)` 是纯函数。
- **列表行操作**：每行一个 `pendingKey`（或 mutation 的 `variables`）而不是全局单槽 `savingId`/`deletingId`；后端 mutation 必须返回具体行标识。
- 一次性逻辑就地写：不为「可能复用」提前抽 `useAsyncFlow`；只有导出/日志上传/技能执行三处共享同一 idle→running→done/error→reset 拓扑时，才引入一个 40 行以内的 `useAsyncFlow`。

### 3.6 AX（代理体验）单独交付

- `docs/STATE_MACHINES.md`：从 Rust enum 生成的状态/转移表，agent 不用 grep 5000 行组件。
- `lifecycle` 调试 IPC：`app_lifecycle_snapshot()` 返回所有 active 实体 + 当前状态 + 最近 10 条转移；agent 可一次查询，不再从日志反推。
- CI guard：`architecture_guard.py` 增加规则——禁止在 `lifecycle/` 之外写 `status = '...'` 或 `TaskStatus::`；禁止前端比较 `phase ===`，必须用 `outcome`。
- 编译期穷举：所有状态 union 的 `switch` 必须有 `never` 分支；DB CHECK 让非法值在写入时失败。

---

## 4. 分阶段实施计划（每阶段独立可回滚）

### Phase 0 — 契约与删除（1 个版本，无行为变化）

1. 写 `docs/STATE_MACHINES.md`（状态表 + 合法转移 + 术语表）。
2. Rust `lifecycle` 空壳：枚举、转移表、`Snapshot`，暂不接线。
3. 删除 §1.5 的 17 个死状态机/死字段（纯删除 + 测试调整）。
4. CI guard + TS `never` 检查。
5. **验收**：现有测试全绿；`cargo test --lib` 数量不减；前端 `tsc`/Vitest 基线不变；无 UI 行为变化（截图/视口探针）。

### Phase 1 — Rust P0（任务/agency/聊天/审批/租约/训练/gate）

1. `lifecycle` 接入 Task：`executor.start/complete/fail` → `transition`；`cancel_task` → `cancel`；`record_progress` 不再改状态；HeartbeatMonitor → `Reap`。
2. `cancel` 接入 task + agency batch + chat revision + LLM；删除 4 套旧注册表。
3. agency run/agent task/chat message/director action 全部走 `transition`；`finish_run` 返回 OutCome，拒绝时禁止发 completed 事件。
4. lease 并入 Start/Succeed/Fail/Cancel；`reap_expired` 由 supervisor 调 `Reap`。
5. training/gate：进度只有 `lifecycle.Progress`；资产失败/判分不可用写 `Failed` 或 `Succeeded(Metadata)`，不再用 progress=100 表达成功。
6. DB `V158` CHECK + 数据清洗。

**验收探针**（必须全部显式测试，不接受“日志看起来对”）：

- 五种终态 × 八种事件的转移矩阵：终态不可覆盖；`cancel` 幂等；progress 单调不回退。
- 取消后在途 runner：DB 终态不再被 `completed/failed` 覆盖；事件流只出现一次 terminal；沙盒/场景不被继续写。
- 手动/once/cron 三路径并发同一 task：只允许一个实例（锁在 update 后仍存活）。
- 心跳超时与仍活执行器：不再“先 pending 后 completed”互相覆盖。
- agency 取消：`agency_activity_log` 不出现 completed；run 的 scene 写入数为 0（或明确记录已完成部分）。
- chat 占位消息成功路径转 done；action 从 proposed→approved→executing→done 单链；run 结束后审批卡不可再执行。
- lease：终态后租约为 0；过期租约由 supervisor 收尾。
- 训练：资产失败/判分不可用时不得 progress=100 + completed。

### Phase 2 — 前端 P0（生成协议 + 会话机）

1. Rust 事件加 `label`，`phase` 改 id（保留旧中文解析）。
2. `state/generation.ts` 接线：删两份 `PRECISE_PHASE_PATTERNS`、两份优先级表；修 stale snapshot；终态在一个 reducer 闭合。
3. `session` 机接入 `selectChapter/selectStory/switchStory/onChapterUpdated/content update/append`；删除四个一次性旗标和 quiet period。
4. 幽灵机：Esc 取消帧、Tab 只允许 complete、单锁。
5. 保存态：`SaveState` 替换 `isSaved/saveError` 两布尔；`update_scene` CAS。
6. 生成入口 ref 互锁 + cancel epoch；文思自动续写读取取消快照。

**验收探针**：

- 后端中文/英文/snake_case 三种 `phase` 都能解析；成功终态 100ms 内 primary=done，且只关一次。
- 快速点 5 个章节，最终只应用最后一次 ticket；分章抑制不被另一章消费。
- 切书失败/成功后旧事件不再写新 scene；`resetStoryScopedUiState` 后 sceneId 为空。
- Esc 后打字机不再写幽灵；Tab 在 streaming 时不可接受（或接受完整文本，不截断）。
- 保存失败→顶栏失败→重试成功→已保存；切章/切书后不残留“保存中”。
- 双击 Ctrl+Enter 只生成一次；取消后成功结果不追加；文思 active 取消不复活。

### Phase 3 — Rust P1（MCP/网关/日志/更新/拆书/向量/后处理/运行集合）

1. MCP：连接顺序改为“连接 map 先写、能力后注册”；权限默认 **Unknown→不重试**（只有显式 Read 才重发）；SSE 响应按 JSON-RPC id 配对；stdio 缺 stdin 返回 NotConnected。
2. Gateway：健康态单一 registry；DB 只做镜像；Unknown 统一语义；退避读同一 `last_checked_at`。
3. LogReport：重复包返回 `path: null` 或删除；并发 tmp 用 uuid；自动上传用显式 `AutoUploadState`。
4. Updater：`NoUpdate` 不早退，继续检查第二源；下载/安装合并为显式状态。
5. 拆书/指导书：`Extracting/Merging` 落库；未知状态入口报错；去重只复用非终态或明确返回 failed。
6. 向量/后处理：初始化单次 `OnceCell`；损坏状态解析失败报错，不回退 Running。
7. `recompute/style_learning`：RAII scope；闸门失败也释放；重启扫描遗留标记。

### Phase 4 — 前端 P1 流程

- U1–U16 全部改成“单一 pending + epoch + close 重置”；删除死字段；向导用枚举；行操作用具体 ID。
- 先做 U1（拆书）、U2（Prompts）、U5（审批）、U6（日志上传）、U7（Pipeline）、U8（Training），因为这 6 个直接影响用户数据或主路径。

### Phase 5 — P2/P3

- subscription/identity 单一门控（tier+status+expires_at+remote 优先级）；auth 死模块删除后的身份统一。
- window/automation/canonical phase：窗口显式 `Hidden/Closed`；自动化 `Idle/Processing/Shutdown`；叙事阶段改为存储快照而非每次启发式。
- view routing 集合单一来源；深链字段 TTL 或删除。

---

## 5. UX / DX / AX 收益（可衡量）

### UX（用户可感知）

| 症状 | 当前根因 | 修复后 |
|---|---|---|
| 生成结束底栏仍转圈 | F1 词汇分裂 + stale snapshot | terminal 一次闭合，100ms 内 done |
| 点 A 章最后落到 B 章 | F2 无 epoch | 只应用最后一次点击 |
| 幽灵 Esc 无效/Tab 截断 | F3 六锁 | 单机转移，Esc 必取消 |
| 顶栏“保存中”假死 | F4 两布尔 | 单一 SaveState + CAS |
| 审批卡 run 结束后仍可点 | L4/U5 | 终态不可再执行，卡随 run 结束消失 |
| 训练 progress=100 但资产失败 | L6 | 失败/降级显式终态，不再假成功 |
| 拆书完成永远加载 | U1 liveStatus 泄漏 | 单源 + 完成后 invalidate |
| 日志重复上传/弹窗错位 | U6 | 单一 pending + 关闭取消 |

### DX（开发者）

- 状态修改从“十几个 update_*/set*”收敛到 3 个 lifecycle 方法 + 3 个 cancel 方法。
- 不再需要同时理解 DB/内存/事件/日志四份状态；改一处就改完。
- 51 个条目 → 4 个深模块；新增实体只需注册转移表，不新增一套 status 字符串。
- 删除 17 个死状态机，代码量与搜索面直接下降。

### AX（代理体验）

- 一次 `app_lifecycle_snapshot` + `docs/STATE_MACHINES.md` 即可知道所有 active 状态，不需 grep 5000 行 FrontstageApp。
- 编译期 `never` + CI guard 阻止“又一字符字符串状态”产生；agent 改动前有唯一契约可依。
- 状态/转移命名统一（snake_case id），日志、事件、测试、提示词共用一套词汇。

---

## 6. 为什么不选其他方案

| 方案 | 拒绝理由 |
|---|---|
| 逐条修 51 个隐式条目 | 每个点仍是浅层补丁；同一词汇会继续分裂；下一次改动重新引入三源 |
| 引入通用状态机 crate（statig/smol） | 与 async/SQL/租约/事件分离，DX 下降；跨实体仍需要多套表 |
| 单一巨型 `AppState` | 爆炸半径太大，UI/任务/agency/MCP 全耦合；违反「更小接口」 |
| 每模块一套 Enum，不共享词汇 | 现在 51 个条目就是这种做法的结果；agent 每加实体多一套翻译表 |
| 只加守卫函数（如 `ensure_terminal`） | 调用方仍可绕过；没有原子 SQL/行数校验就无法防“后写覆盖终态” |

---

## 7. 不破坏策略

1. **保留现有 IPC 名称与事件名**；`lifecycle`/`cancel` 先包裹既有 repository，再逐步替换内部实现。
2. **兼容旧词汇**：前端 `parseGenerationEvent` 接受中文/英文/snake_case；Rust 事件新增 `label` 而不是删除 `phase`。
3. **迁移可逆**：V158 先清洗数据再加 CHECK；死状态机删除分两个版本（先停止写/读，再删表）。
4. **开关**：`lifecycle_core_enabled` / `session_epoch_enabled` 配置（默认 true，出问题单个版本可关）。
5. **每阶段独立 tag + 全平台 CI**；P0 阶段不打包本地，只推 GitHub Actions（遵守用户构建规则）。
6. **验收探针先行**：每阶段先写转移矩阵/竞态测试，再做实现；未跑通验收不宣称修复（verify-until-shippable）。

---

## 8. 建议的第一批落地（按 ROI 排序）

1. **Task 终态 + cancel + 调度锁**（L1/L3/L8）——直接消除“取消后复活”“重复执行”“进度 0/75 矛盾”。
2. **Agency run/chat/approval 单链**（L2/L4）——消除 run cancelled 仍写场景、审批卡残留。
3. **前端 generation 协议接线**（F1）——最小改动（模块已存在）换掉底栏长期卡死。
4. **前端 session epoch**（F2/F4）——消除跨章/跨故事数据损坏与保存假死。
5. **幽灵/入口锁**（F3/F5）——消除截断与双击双生成。
6. **MCP 权限默认拒绝 + 连接顺序**（S1）——安全优先，写工具绝不重发。
7. **训练/gate 显式终态**（L6/L7）——不再用 progress=100 伪造成功。

> 以上每一项都有对应的验收探针；建议 Phase 1+2（前 6 组）先作为 v0.95.0 的 P0 收敛，Phase 3+4 作为 v0.96.0，Phase 5 作为 v0.97.0。版本号需在开工前按 `git log origin/master -1` 与 `git tag -l` 重新核对，避免再次被并发会话占用。
