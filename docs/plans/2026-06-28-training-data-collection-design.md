# 创作训练数据采集与回流方案设计

> 状态: 设计完成，待实施  
> 日期: 2026-06-28  
> 版本: v1.0

## 一、背景与目标

StoryMoss 作为免费 AI 辅助小说创作桌面应用，每次创作产生的模型参数、提示词与生成内容成果是珍贵的训练数据，对专业创作模型的微调与优化具有不可替代的价值。

### 目标

建立从用户桌面到出品方服务器的完整训练数据采集与回流管道：

1. **全面采集**：模型参数 · 提示词 · 生成正文 · 用户行为反馈
2. **隐私内建**：正文在本地脱敏加密，原文不出设备
3. **合规先行**：满足《个人信息保护法》告知-同意、最小必要、删除权等要求
4. **零摩擦体验**：授权后全链路自动化，不影响生成速度
5. **授权换免费**：用户同意数据授权即可免费使用 AI 功能

### 约束

- 桌面应用（Tauri 2.4 + Rust），纯本地运行，数据在本地
- 中国大陆合规主体，需遵守《个人信息保护法》《数据安全法》
- 用户创作内容高度敏感，脱敏策略必须在本地执行
- 零新增 LLM 调用（不加任何成本）

---

## 二、架构全景

```
┌────────────────────── 用户桌面 (StoryMoss Tauri App) ──────────────────────┐
│                                                                               │
│  ┌──────────┐   ┌─────────────┐   ┌──────────┐   ┌──────────────┐            │
│  │ 授权协议 │──▶│ 试用计数器  │──▶│ 数据采集 │──▶│ 本地预处理   │            │
│  │ 首次弹窗 │   │ N次生成上限 │   │Training  │   │ 脱敏+加密    │            │
│  └──────────┘   └─────────────┘   │Tuple     │   └──────┬───────┘            │
│                                    └──────────┘          │                    │
│                                                          ▼                    │
│                                                  ┌──────────────┐            │
│                                                  │  上传队列    │            │
│                                                  │  断点续传    │            │
│                                                  │  指数退避    │            │
│                                                  └──────┬───────┘            │
│                                                         │                    │
└─────────────────────────────────────────────────────────│────────────────────┘
                                                          │
                                              HTTPS + AES-256-GCM
                                                          │
┌────────────────────── 出品方服务器 ─────────────────────────────────────────┐
│                                                                               │
│  ┌──────────┐   ┌─────────────┐   ┌────────────────────────────────────┐     │
│  │ 接收网关 │──▶│ 用户授权服务│──▶│ 数据湖 (S3/OSS + PostgreSQL)       │     │
│  │ /api/v1/ │   │ JWT+授权范围│   │ 正文密文不解密，训练管道离线解密    │     │
│  └──────────┘   └─────────────┘   └────────────────────────────────────┘     │
│                                                                               │
└───────────────────────────────────────────────────────────────────────────────┘
```

### 设计原则

| 原则 | 实现 |
|------|------|
| **木桶不短板** | 七环节缺一不可，但开发量有轻有重 |
| **隐私设计内建** | 脱敏在本地完成，正文出设备前已经过处理加密 |
| **用户体验零摩擦** | 授权后完全静默，不弹窗、不阻塞、不影响生成速度 |
| **合规先行** | 每个环节嵌入《个人信息保护法》要求 |
| **零新增 LLM 调用** | 全程不改动现有 LLM 调用链路，仅在完成后挂钩采集 |

---

## 三、数据模型：TrainingTuple

一次 AI 生成产生的完整训练记录。

### 3.1 完整结构

```rust
/// 一次 AI 生成产生的完整训练数据元组
struct TrainingTuple {
    // ========== 标识层 ==========
    tuple_id: String,              // UUID v7 (时间排序)
    user_hash: String,             // SHA256(user_id + salt), 不可逆匿名化
    story_hash: String,            // SHA256(story_id + salt), 同一用户内可关联
    created_at: DateTime<Utc>,
    app_version: String,           // 如 "v0.23.73"

    // ========== 模型参数层 ==========
    model_params: ModelParams {
        provider: String,
        model_name: String,
        model_role: String,        // "creative" | "tool" | "background"
        is_local_model: bool,
        temperature: f32,
        max_tokens: i32,
        top_p: Option<f32>,
        frequency_penalty: Option<f32>,
        presence_penalty: Option<f32>,
        capabilities: Vec<String>,  // ["chat","reasoning","json_mode",...]
        quality_tier: String,
        speed_tier: String,
        max_context_length: u32,
    },

    // ========== 提示词层 ==========
    prompt_layer: PromptLayer {
        system_prompt_full: String,
        prompt_template_id: Option<String>,
        prompt_template_category: Option<String>,
        user_context_summary: String,       // ≤500字上下文摘要
        selected_asset_ids: Vec<String>,
        asset_compact_guidance: String,     // 注入提示词的资产指导文本
        framework_selections: Option<Value>,
        intent_verb: Option<String>,
        intent_object: Option<String>,
    },

    // ========== 生成内容层 (脱敏处理) ==========
    content_layer: ContentLayer {
        generated_text_hash: String,       // SHA256(原文) 用于去重
        generated_text_length: u32,
        generated_text_preview: String,    // 前200字 (已脱敏)
        // 完整正文加密存储: data/training_blobs/{tuple_id}.enc
        // AES-256-GCM, 服务端不解密
    },

    // ========== 行为反馈层 ==========
    behavior_layer: BehaviorLayer {
        generation_mode: String,           // "tri_shot" | "time_sliced" | "full"
        generation_phase: String,          // "续写" | "创世第1章" | ...
        duration_ms: u64,
        prompt_tokens: u32,
        completion_tokens: u32,
        route_decision: Option<String>,
        llm_call_success: bool,
        error_type: Option<String>,

        user_action: UserAction,
        // AcceptedAsIs         — 直接接受 → 高质量
        // AcceptedWithEdits    — 编辑后接受 → 部分满意
        // Rejected             — 删除 → 低质量
        // ImplicitlyContinued  — 后面继续写 → 满意
        // Abandoned            — 离开该章 → 弱信号
    },

    // ========== 元数据层 ==========
    metadata: TrainingMetadata {
        generation_request_id: String,
        previous_tuple_id: Option<String>,
        creative_asset_versions: HashMap<String, String>,
        prompt_registry_version: String,
    },
}
```

### 3.2 存储方式

| 数据 | 存储位置 | 格式 |
|------|----------|------|
| 元数据（除正文外所有字段） | SQLite `training_tuples` 表 | 结构化 |
| 完整正文 | 本地文件 `data/training_blobs/{tuple_id}.enc` | AES-256-GCM 密文 |
| 加密方式 | 信封加密：AES-256-GCM 会话密钥 + 服务端 RSA 公钥加密 | — |

### 3.3 本地 SQLite 表

```sql
CREATE TABLE training_tuples (
    id TEXT PRIMARY KEY,
    user_hash TEXT NOT NULL,
    story_hash TEXT NOT NULL,
    created_at TEXT NOT NULL,
    app_version TEXT NOT NULL,

    -- model_params
    provider TEXT NOT NULL,
    model_name TEXT NOT NULL,
    model_role TEXT NOT NULL DEFAULT 'creative',
    is_local_model INTEGER NOT NULL DEFAULT 0,
    temperature REAL,
    max_tokens INTEGER,
    top_p REAL,
    capabilities TEXT,  -- JSON array

    -- prompt_layer
    system_prompt_full TEXT,
    prompt_template_id TEXT,
    prompt_template_category TEXT,
    user_context_summary TEXT,
    selected_asset_ids TEXT,  -- JSON array
    asset_compact_guidance TEXT,
    intent_verb TEXT,
    intent_object TEXT,

    -- content_layer
    generated_text_hash TEXT,
    generated_text_length INTEGER,
    generated_text_preview TEXT,  -- 已脱敏前200字
    content_blob_path TEXT,       -- 加密正文文件路径

    -- behavior_layer
    generation_mode TEXT,
    generation_phase TEXT,
    duration_ms INTEGER,
    prompt_tokens INTEGER,
    completion_tokens INTEGER,
    route_decision TEXT,
    llm_call_success INTEGER,
    error_type TEXT,
    user_action TEXT,

    -- metadata
    generation_request_id TEXT,
    previous_tuple_id TEXT,
    creative_asset_versions TEXT,  -- JSON object
    prompt_registry_version TEXT,

    -- sync
    upload_status TEXT NOT NULL DEFAULT 'pending',  -- pending|uploading|synced|blocked
    uploaded_at TEXT,
    upload_attempts INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_training_tuples_upload ON training_tuples(upload_status, created_at);
CREATE INDEX idx_training_tuples_user ON training_tuples(user_hash, created_at);
```

---

## 四、本地采集与预处理

### 4.1 三个挂钩点

不新建采集通道，在现有 LLM 调用完成后挂钩补采。

| 挂钩点 | 位置 | 触发时机 | 采集内容 |
|--------|------|----------|----------|
| **Hook A** | `llm/service.rs` `execute_generation` 返回后 | 每次 LLM 调用完成 | `model_params` + LLM 性能数据 |
| **Hook B** | `agents/orchestrator.rs` / `commands/orchestrator.rs` `smart_execute` 返回后 | 一次创作生成完成 | `prompt_layer` + `content_layer` + `metadata` |
| **Hook C** | 前端 `FrontstageApp.tsx` | 用户对生成正文操作后 | `behavior_layer.user_action` |

### 4.2 数据流

```
现有链路                        新增挂钩
──────────                      ────────
execute_generation()  ───────▶  Hook A: INSERT training_tuples (model_params)
        │
        ▼
smart_execute()       ───────▶  Hook B: UPDATE 补齐 prompt_layer + content_layer
        │                        └─ 正文脱敏 → 加密 → 写 blob 文件
        ▼
前端 [正文已展示]      ───────▶  Hook C: log_frontend_event → UPDATE user_action
        │                        30s 无编辑 → AcceptedAsIs
        │                        有编辑保存 → AcceptedWithEdits
        │                        删除/离开 → Rejected/Abandoned
```

### 4.3 Hook A 实现要点

```rust
// llm/service.rs execute_generation() 返回 Ok(response) 后
async fn collect_hook_a(
    pool: &DbPool,
    config: &AppConfig,
    request: &GatewayRequest,
    profile: &LlmProfile,
    response: &GenerateResponse,
    duration_ms: u64,
    success: bool,
    error_type: Option<&str>,
) {
    if !config.data_consent_granted { return; }

    let tuple = TrainingTuple {
        tuple_id: Uuid::now_v7().to_string(),
        user_hash: config.user_hash(),
        story_hash: config.story_hash(),
        model_params: ModelParams {
            provider: profile.provider.to_api_string(),
            model_name: profile.model.clone(),
            model_role: request.model_role.map(|r| r.as_str()).unwrap_or("creative"),
            is_local_model: profile.is_local_model,
            temperature: profile.temperature,
            max_tokens: profile.max_tokens,
            top_p: profile.top_p,
            frequency_penalty: profile.frequency_penalty,
            presence_penalty: profile.presence_penalty,
            capabilities: profile.capabilities.iter().map(|c| c.as_str()).collect(),
            quality_tier: profile.quality_tier.as_str(),
            speed_tier: profile.speed_tier.as_str(),
            max_context_length: profile.max_context_length,
        },
        behavior_layer: BehaviorLayer {
            duration_ms,
            prompt_tokens: response.tokens_used as u32,
            completion_tokens: 0, // Hook B 补齐
            route_decision: None,
            llm_call_success: success,
            error_type: error_type.map(String::from),
            user_action: UserAction::Pending,
            ..Default::default()
        },
        ..TrainingTuple::new_empty()
    };

    TrainingTupleRepo::insert(pool, &tuple)?;
}
```

### 4.4 Hook B 实现要点（关键）

```rust
// 在 generate_for_request_with_request_id() 成功返回后
async fn collect_hook_b(
    pool: &DbPool,
    config: &AppConfig,
    tuple_id: &str,
    request: &GatewayRequest,
    final_content: &str,
    context: &GenerationContext,
) -> Result<()> {
    if !config.data_consent_granted { return Ok(()); }

    let mut tuple = TrainingTupleRepo::find_by_id(pool, tuple_id)?;

    // 提示词层
    tuple.prompt_layer = Some(PromptLayer {
        system_prompt_full: request.system_prompt.clone().unwrap_or_default(),
        prompt_template_id: resolve_template_id(&request.context_label),
        prompt_template_category: resolve_category(&request.context_label),
        user_context_summary: build_context_summary(context),
        selected_asset_ids: request.discovered_asset_ids.clone(),
        asset_compact_guidance: render_selected_asset_guidance(
            &request.discovered_asset_ids
        )?,
        framework_selections: load_framework_selections(&request.request_id),
        intent_verb: request.intent_verb.clone(),
        intent_object: request.intent_object.clone(),
    });

    // 内容层 (正文脱敏 + 加密)
    if !final_content.is_empty() {
        let sanitized = sanitize_pii(final_content);
        let encrypted = encrypt_content_blob(
            &sanitized,
            &config.server_public_key
        )?;
        let blob_path = write_content_blob(&tuple.tuple_id, &encrypted)?;

        tuple.content_layer = Some(ContentLayer {
            generated_text_hash: sha256(final_content),
            generated_text_length: final_content.chars().count() as u32,
            generated_text_preview: sanitized.chars().take(200).collect(),
        });
        tuple.content_blob_path = Some(blob_path);
    }

    TrainingTupleRepo::update(pool, &tuple)?;
    Ok(())
}
```

### 4.5 PII 脱敏策略 (`sanitize_pii`)

正文在离开设备前执行脱敏，人名/地名/联系方式替换为通用占位符。

| 类型 | 检测方式 | 替换为 |
|------|----------|--------|
| 中文人名 | 启发式 (常见姓氏 + 1-2 汉字) | `[CHAR_1]`, `[CHAR_2]` … |
| 地名 | 行政后缀 (省/市/县/区/镇/村) + 高频地名表 | `[LOC_1]`, `[LOC_2]` … |
| 手机号 | 正则 `1[3-9]\d{9}` | `[PHONE]` |
| 邮箱 | RFC 5322 子集正则 | `[EMAIL]` |
| 身份证号 | 18 位校验正则 | `[ID]` |
| 英文人名 | NER 启发式 (首字母大写连续词) | `[PERSON_1]` … |

**权衡说明：** 脱敏会损失角色名称一致性等微调信号，但这是《个人信息保护法》第 6 条数据最小化要求的必要代价。服务端不解密原始正文，脱敏版本已足够做写作风格、结构规划等维度的训练。

### 4.6 Hook C 实现要点

```typescript
// 利用已有的 log_frontend_event 通道, 新增 user_action 事件
// 后端 log_frontend_event 处理中新增对以下 phase 的识别:

// phase: "training.user_action"
// message: "accepted_as_is" | "accepted_with_edits" | "rejected" | "continued"
// details: { tuple_id, previous_tuple_id? }
```

后端 `llm/commands.rs` 的 `log_frontend_event` 处理中增加匹配逻辑，更新 `training_tuples.user_action`。

---

## 五、上传队列与容错

### 5.1 上传 Worker

后台 tokio task，每 5 分钟唤醒，静默工作。

```
┌────────────────── 本地上传队列 ──────────────────┐
│                                                    │
│  training_tuples (SQLite)                          │
│  ├─ upload_status: "pending" | "uploading" |       │
│  │                  "synced" | "blocked"           │
│  │                                                │
│  ▼                                                │
│  UploadWorker (后台 tokio task, 每 5min 唤醒)      │
│  ├─ 检查: data_consent_granted? + 网络可达?        │
│  ├─ 取 ≤50 条 pending → 标记 uploading            │
│  ├─ 批量打包 JSON + 正文 blob (gzip)               │
│  ├─ POST /api/v1/training-data                    │
│  │   ├─ 200 → 标记 synced                         │
│  │   ├─ 429/503 → 指数退避 1s→5s→25s→125s→625s   │
│  │   ├─ 401/403 → 标记 blocked (授权已撤销)        │
│  │   └─ 其他网络错误 → 回退为 pending              │
│  └─ 本地保留 synced 数据 30 天，过期自动清理       │
│                                                    │
│  断网/关机场景:                                     │
│  - 数据先写 SQLite, 上传是异步的 → 永不丢失        │
│  - 服务端按 tuple_id 幂等去重                      │
│  - 重启后 Worker 自动恢复队列                       │
└────────────────────────────────────────────────────┘
```

### 5.2 上传协议

```
POST /api/v1/training-data
Authorization: Bearer <JWT>
Content-Type: multipart/mixed
X-Client-Version: v0.23.73
X-Upload-Batch-Id: <UUID v7>
X-Content-Digest: <SHA256 of entire payload>

--boundary
Content-Type: application/json

{
  "batch_id": "uuid",
  "user_hash": "sha256...",
  "app_version": "v0.23.73",
  "tuples": [
    {
      "tuple_id": "...",
      "model_params": { ... },
      "prompt_layer": { ... },
      "content_layer": {
        "generated_text_hash": "...",
        "generated_text_length": 1234,
        "generated_text_preview": "..."
      },
      "behavior_layer": { ... },
      "metadata": { ... }
    }
  ]
}

--boundary
Content-Type: application/octet-stream
Content-Disposition: attachment; filename="blob_{tuple_id}.enc"

<AES-256-GCM ciphertext>

--boundary--

Response 200:
{ "accepted": 47, "duplicates": 3, "rejected": 0 }
```

### 5.3 安全设计

| 层面 | 措施 |
|------|------|
| **传输** | HTTPS + TLS 1.3 + 证书锁定 (pin 服务端证书指纹) |
| **认证** | JWT 由客户端用 `user_hash + device_fingerprint` 签名，服务端验签 |
| **正文加密** | 信封加密：AES-256-GCM 会话密钥 + 服务端 RSA-4096 公钥加密。服务端不解密存储，仅离线训练管道中解密 |
| **防重放** | `tuple_id` UUID v7 + 服务端幂等性检查 |
| **防篡改** | `X-Content-Digest` 头部携带整个 payload 的 SHA256 哈希 |

### 5.4 服务端最小实现骨架

```rust
// 服务端: Axum (约 400 行)
struct AppState {
    db: PgPool,                       // PostgreSQL: 训练元数据
    blob_store: S3Client,             // S3/OSS: 加密正文
    jwt_keys: JwtKeys,
    rate_limiter: RateLimiter,        // 按 user_hash 限流
}

async fn upload_training_data(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<BatchResponse>, AppError> {
    // 1. 验证 JWT
    // 2. 限流: 每用户每分钟 ≤ 1 次上传
    // 3. 逐 tuple_id 检查幂等 → 跳过已存在
    // 4. 正文 blob → 直接写 S3 (不解密)
    // 5. 元数据 → INSERT INTO training_tuples
    // 6. 返回 { accepted, duplicates, rejected }
}
```

### 5.5 服务端存储结构

```
S3/OSS: storymoss-training-data/
└── blobs/
    └── {user_hash[:2]}/
        └── {user_hash}/
            └── {tuple_id}.enc       (AES-256-GCM 密文)

PostgreSQL: training_db
├── training_tuples (
│     tuple_id, user_hash, story_hash, app_version,
│     model_params JSONB, prompt_layer JSONB,
│     content_layer JSONB, behavior_layer JSONB,
│     metadata JSONB, received_at, blob_s3_key
│   )
├── user_consents (
│     user_hash, consented_at, revoked_at,
│     consent_version
│   )
├── upload_batches (
│     batch_id, user_hash, tuple_count, received_at
│   )
└── audit_log (
│     action, user_hash, target, details, created_at
│   )
```

---

## 六、前端授权流程

### 6.1 AppConfig 新增字段

```rust
// config/settings.rs AppConfig 新增
#[serde(default)]
pub data_consent_granted: bool,         // 全或无授权
#[serde(default)]
pub consent_version: Option<String>,    // 同意的协议版本号
#[serde(default)]
pub consent_granted_at: Option<String>,
#[serde(default)]
pub trial_generations_used: u32,
#[serde(default = "default_trial_limit")]
pub trial_generations_limit: u32,       // 默认 15
#[serde(default)]
pub server_public_key_hash: Option<String>,
```

### 6.2 首次启动授权弹窗

```
┌─ 首次启动 ──────────────────────────────────────────────────────┐
│                                                                   │
│  ┌─────────────────────────────────────────────────────────┐     │
│  │                   🌿 StoryMoss 草苔                      │     │
│  │                                                          │     │
│  │  本软件对创作者完全免费。作为交换，我们希望将您的创作     │     │
│  │  数据（模型参数、提示词、AI 生成内容）用于 AI 写作模型    │     │
│  │  的研究训练，持续提升生成质量。                           │     │
│  │                                                          │     │
│  │  ┌─ 📋 数据授权说明 ────────────────────────────────┐    │     │
│  │  │                                                    │    │     │
│  │  │  采集范围：模型配置、调用参数、完整提示词、        │    │     │
│  │  │  AI 生成正文、用户行为反馈                         │    │     │
│  │  │                                                    │    │     │
│  │  │  ⚠️ 原文不出设备：正文经本地脱敏后上传，           │    │     │
│  │  │     人名/地名/联系方式替换为占位符                 │    │     │
│  │  │                                                    │    │     │
│  │  │  随时可撤销：设置页可关闭授权并请求删除已上传数据  │    │     │
│  │  │                                                    │    │     │
│  │  └────────────────────────────────────────────────────┘    │     │
│  │                                                          │     │
│  │  ┌─ 试用选项 ───────────────────────────────────────┐    │     │
│  │  │                                                    │    │     │
│  │  │  暂不同意？可试用 15 次 AI 生成，之后需授权继续    │    │     │
│  │  │  （基础写作、手动管理功能始终可用）                │    │     │
│  │  │                                                    │    │     │
│  │  └────────────────────────────────────────────────────┘    │     │
│  │                                                          │     │
│  │        [ 同意授权，开始使用 ]    [ 试用 15 次 ]           │     │
│  │                                                          │     │
│  │                     📄 完整协议                           │     │
│  └──────────────────────────────────────────────────────────┘     │
│                                                                   │
└───────────────────────────────────────────────────────────────────┘
```

### 6.3 试用计数器与生成守卫

```typescript
// 前端生成入口守卫
function checkGenerationAllowed(): boolean {
  const { dataConsentGranted, trialUsed, trialLimit } = settings;

  if (dataConsentGranted) return true;
  if (trialUsed < trialLimit) {
    incrementTrial();        // 调后端 trial_generations_used += 1
    return true;
  }
  showConsentModal();        // 试用耗尽，弹授权协议窗
  return false;
}
```

### 6.4 设置页隐私卡片

```
┌─ 数据与隐私 ──────────────────────────────────────┐
│                                                     │
│  创作数据授权                         [ 已授权 ✓ ] │
│  模型参数 · 提示词 · AI 生成内容 · 使用行为         │
│                                                     │
│  已贡献训练数据: 127 条                             │
│  最近上传: 2026-06-28 15:32                         │
│                                                     │
│  [ 撤销授权 ]                                       │
│  [ 请求删除已上传数据 ]                             │
│  [ 导出我的全部数据 ]                               │
│                                                     │
└─────────────────────────────────────────────────────┘
```

---

## 七、合规协议

### 7.1 授权协议文本

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
        StoryMoss 创作数据授权协议 v1.0
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

感谢使用 StoryMoss（以下简称"本软件"）。本软件由
[出品方名称]（以下简称"我们"）开发，对创作者完全免费。

作为免费使用的交换条件，您同意将使用本软件过程中产生的
以下数据授权我们用于 AI 写作模型的研究与训练（以下简称
"研究目的"）：

一、授权数据范围
  (1) 模型调用参数（模型名称、温度、token 数等配置）
  (2) 发给 AI 的创作提示词（含系统提示词和上下文摘要）
  (3) AI 生成的正文内容
  (4) 您对生成内容的操作行为（接受、修改、删除等）

二、数据保护措施
  (1) 本地脱敏：正文中的人名、地名、联系方式等在离开您的
      设备前会被替换为通用占位符（如 [CHAR_1]、[LOC_1]）。
      您的原始创作不会以可识别形式离开本设备。
  (2) 加密传输：所有数据通过加密通道上传，正文额外使用
      AES-256-GCM 加密，密钥由我们持有，服务端不解密存储。
  (3) 数据最小化：仅采集上述四类数据，不采集您的设备信息、
      浏览记录、地理位置等非创作相关数据。

三、您的权利
  (1) 撤销权：您可随时在设置中撤销本授权，撤销后不再上传
      新数据。
  (2) 删除权：您可请求删除已上传的数据。我们将在 15 个
      工作日内从在线存储中删除，但已用于训练且无法从模型
      中剥离的数据除外。
  (3) 知情权：您可在设置中查看已上传数据的条数和最近上传
      时间。
  (4) 携带权：您可导出本软件中属于自己的全部创作数据。

四、数据出境
  您的数据将上传至我们在 [服务器所在地] 的服务器。
  如服务器位于中国大陆境外，我们将依法进行数据出境安全
  评估，确保数据接收方提供与中国法律同等水平的保护。

五、协议更新
  如本协议发生重大变更，本软件将在启动时提示您重新确认。
  继续使用即表示同意更新后的协议。

六、免费使用条件
  同意本协议即可免费使用 AI 生成功能。不同意可试用 15 次，
  试用结束后仅可使用基础写作和管理功能。

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

### 7.2 《个人信息保护法》合规映射

| 法定要求 | 出处 | 本方案满足方式 |
|----------|------|---------------|
| **告知-同意** | 第13-17条 | 首次弹窗完整告知 + 主动勾选同意 + 可随时撤回 |
| **最小必要** | 第6条 | 仅采集四类创作数据，本地脱敏后人名地名已不可识别 |
| **单独同意** | 第23条 | 授权为独立弹窗，不与用户协议捆绑 |
| **删除权** | 第47条 | 设置页一键请求删除，15 工作日内执行 |
| **数据出境** | 第38-40条 | 协议明确告知服务器所在地，如需出境依法评估 |
| **影响评估** | 第55-56条 | 出具数据处理影响评估报告（内部文档） |
| **自动化决策** | 第24条 | 训练模型非面向个人的自动化决策，不适用 |

### 7.3 技术合规落地

| 合规动作 | 实现位置 | 机制 |
|----------|----------|------|
| 协议版本追踪 | `AppConfig.consent_version` | 升级协议版本 → 启动时弹重新确认窗 |
| 授权撤销 | `AppConfig.data_consent_granted = false` | 上传队列停止 + 服务端 `/api/v1/revoke/{user_hash}` |
| 删除请求 | 隐私设置页 | 调 `/api/v1/delete/{user_hash}` → 清理 S3 + PG |
| 数据导出 | `export_training_data` 新命令 | 打包 training_tuples + 全部创作内容 → JSON/ZIP |
| 用量透明 | 隐私设置页展示 | `get_training_data_stats` 命令返回已上传条数/时间 |
| 处理记录 | 服务端 `audit_log` 表 | 每个数据操作写审计日志，留存 ≥3 年 |

---

## 八、实施路线图

### Phase 1 (Week 1-2): 数据底座

- 服务端: 接收 API + PostgreSQL + S3/OSS 存储
- 数据库迁移: `training_tuples` 表 + `AppConfig` 新字段
- Hook A + Hook B: 采集写入 training_tuples
- 脱敏实现: `sanitize_pii` + 单元测试
- 正文加密: 信封加密 (AES-256-GCM + RSA)

### Phase 2 (Week 3): 前端体验

- 首次启动授权弹窗 (`ConsentModal.tsx`)
- 试用计数器 + 生成守卫 (`useTrialGuard`)
- 设置页隐私卡片 (`PrivacySettings.tsx`)
- Hook C: 用户行为信号采集

### Phase 3 (Week 4): 传输闭环

- 上传队列 Worker + 批量打包 + 加密传输
- 服务端幂等接收 + 用户撤销/删除 API
- 证书锁定 + JWT 双向认证
- 端到端集成测试 (单用户全链路)

### Phase 4 (Week 5): 合规收尾

- 协议文本最终化（填入 [出品方名称] 和 [服务器所在地]）
- 数据出境安全评估（如服务器在中国大陆境外）
- 个人信息保护影响评估报告
- 服务端审计日志 + 数据留存策略

### Phase 5 (Week 6+): 训练管道（出品方内部，非 StoryMoss 代码）

- 离线解密正文 blob → 构建训练数据集
- 按质量信号分层采样 (Accepted > Edited > Rejected)
- DPO 偏好对构建 (Rejected vs Accepted 同 prompt 对)
- 模型微调 → 评测 → 回灌到 StoryMoss 模型池

### 新增代码量估算

| 组件 | 预估行数 |
|------|----------|
| `src-tauri/src/training/` (新模块: collector + uploader + sanitizer + commands) | ~800 |
| 数据库迁移 (V100__training_tuples.sql) | ~30 |
| 数据仓库层 (repositories_training.rs) | ~150 |
| AppConfig 扩展 | ~40 |
| 前端 ConsentModal | ~200 |
| 前端 PrivacySettings | ~150 |
| 前端 useTrialGuard | ~60 |
| 服务端 (Axum: handlers + models + auth) | ~400 |
| **总计** | **~1830 行** |

### 零新增 LLM 调用

全程不改动现有 LLM 调用链路，仅在生成完成后挂钩采集。勾子函数为 fire-and-forget 或 `spawn_blocking`，不增加任何额外推理成本或响应延迟。

---

## 九、训练数据价值说明

### 训练数据产出预估

假设 1000 名活跃创作者，每人每天生成 20 次：

| 指标 | 日产出 | 月产出 | 年产出 |
|------|--------|--------|--------|
| 训练元组 | 20,000 | 600,000 | 7,200,000 |
| 完整提示词对 | 20,000 | 600,000 | 7,200,000 |
| 正文对（可做 DPO） | 20,000 | 600,000 | 7,200,000 |

### 训练价值维度

| 数据类型 | 可做的训练 | 价值 |
|----------|-----------|------|
| 完整 system_prompt + user_prompt + response 三元组 | SFT 监督微调 | ⭐⭐⭐ |
| 同 prompt 下 Accepted vs Rejected 对 | DPO 偏好对齐 | ⭐⭐⭐ |
| user_action 信号 + 编辑差量 | RLHF 奖励建模 | ⭐⭐ |
| 资产组合 → 生成结果映射 | 策略路由优化 | ⭐⭐ |
| 跨题材/跨风格的生成模式 | 多风格模型训练 | ⭐⭐ |

---

## 附录 A: 现有基础设施复用清单

| 现有组件 | 复用方式 |
|----------|----------|
| `WorkflowLogger` | 关联 `generation_request_id` |
| `llm_calls` 表 | 关联已有 `purpose`/`task_type` 字段 |
| `DiagnosticStore` | Hook B 从中读取系统提示词全文 |
| `GatewayRequest` | Hook A 直接抽取 `model_role`/`intent_verb`/`asset_tags` |
| `LlmProfile` | Hook A 直接序列化模型参数 |
| `PromptRegistry` | Hook B 反查 `prompt_template_id` |
| `log_frontend_event` | Hook C 复用现有通道 |
| `feature_usage_logs` | 关联用户行为时间线 |
| OAuth 用户体系 | `user_hash` 来源 |

## 附录 B: AppConfig 完整新增字段

```rust
// config/settings.rs

#[serde(default)]
pub data_consent_granted: bool,

#[serde(default)]
pub consent_version: Option<String>,

#[serde(default)]
pub consent_granted_at: Option<String>,

#[serde(default)]
pub trial_generations_used: u32,

#[serde(default = "default_trial_limit")]
pub trial_generations_limit: u32,

#[serde(default)]
pub server_public_key: Option<String>,       // PEM 格式 RSA 公钥

#[serde(default)]
pub server_public_key_hash: Option<String>,  // SHA256 指纹

#[serde(default)]
pub server_endpoint: Option<String>,          // 如 "https://data.storymoss.app"

fn default_trial_limit() -> u32 { 15 }
```

## 附录 C: 新增 Tauri IPC 命令

```rust
// 数据统计
get_training_data_stats() -> TrainingDataStats

// 数据导出
export_training_data() -> String  // 返回文件路径

// 授权管理
revoke_data_consent() -> ()
request_data_deletion() -> ()    // 调服务端 + 标记本地 blocked

// 服务端通信（调试用）
check_server_connectivity() -> bool
```

## 附录 D: 服务端 API 完整列表

| 端点 | 方法 | 说明 |
|------|------|------|
| `/api/v1/training-data` | POST | 批量上传训练元组 + 正文 blob |
| `/api/v1/consent/{user_hash}` | GET | 查询用户授权状态 |
| `/api/v1/consent/{user_hash}` | PUT | 更新授权（授权/撤销） |
| `/api/v1/data/{user_hash}` | DELETE | 删除用户全部训练数据 |
| `/api/v1/health` | GET | 服务健康检查 |
| `/api/v1/public-key` | GET | 获取服务端 RSA 公钥 |
