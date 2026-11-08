# MCP 映射契约 v1（标准 MCP / Legado）

> 日期：2026-11-08 ｜ 状态：已实施（v0.91.1） ｜ 契约版本：`mcp-mapping/1.0.0`
> 代码：`src-tauri/src/mcp/mapping.rs`
> 适用：StoryMoss 外部 MCP 服务器（HTTP/stdio）→ 工具面/能力注册表/幕前幕后查询。

## 0. 为什么需要这层契约

v0.88.0 接入栗子读书（Legado）时只做了「字段对不上就静默丢工具」的兼容，
真机验证暴露两个问题：

1. MCP 标准 `tools/list` 返回 `inputSchema`，StoryMoss 内部 `McpTool` 只认
   `parameters`；解析失败被 `if let Ok` 吞掉，连接看似成功但工具数为 0。
2. Legado 的 MCP HTTP 解析对 request header/body 分次到达很敏感，reqwest
   分次写请求会被判为 `400 请求不完整`；回环连接必须直连并合并请求写。

同时，源系统的字段名与权限开关都会变。契约层必须提前定义：

- 什么情况下**整台服务器映射失效**；
- 什么情况下**单个工具失效**；
- 什么情况下**源系统权限撤销**；
- 每一种查询如何报错（稳定错误码，而不是空列表/静默成功）。

## 1. 版本规则

### 1.1 协议版本

客户端 `initialize` 请求固定发送 `protocolVersion = "2024-11-05"`；
服务端返回的版本进入映射契约校验。

| 结果版本 | 处理 |
|---|---|
| `2024-11-05` / `2025-03-26` / `2025-06-18` | 接受 |
| 其他版本 | 连接失败：`MCP_PROTOCOL_UNSUPPORTED` |
| 缺失版本 | 标准 MCP 通用契约允许；Legado 识别为源系统时拒绝（版本规则不完整） |

### 1.2 源系统版本

`serverInfo.name`/`title` 识别源：

- Legado：`legado-macos` / 标题含「栗子读书」；
- 其他源：走 `mcp-standard` 通用契约。

Legado 契约仅支持 **`0.14.x`**（含实测 `0.14.24`）。版本缺失或 major.minor
不是 `0.14` → 连接失败：`MCP_MAPPING_UNSUPPORTED`。

**升级规则**：源系统升级超出范围时，不改旧契约「宽松兼容」，而是新增
`mcp-mapping/1.x.x` 契约与别名；同步更新本文档与测试。契约版本只在字段别名/
权限面变化时递增。

### 1.3 映射版本

`MappingSnapshot.mapping_version` 随连接保存；日志、错误数据、连接摘要都带
`mapping_version`。新版本发布必须能回答「这条连接用的是哪一版映射」。

## 2. 字段规则

### 2.1 工具

| 外部字段 | 内部字段 | 规则 |
|---|---|---|
| `name` | `name` | 必填，空名字 → 该工具 schema drift |
| `description` | `description` | 可选，缺省空串 |
| `inputSchema` | `parameters` | serde alias；缺失或非 object → schema drift |
| `annotations.readOnlyHint` | 权限推断 | `false` → Write 权限 |
| `annotations.openWorldHint` | 权限推断 | `true` → Network 权限 |
| `annotations.destructiveHint` | 权限推断 | `true` → Write 权限 |

`tools/list` 逐项解析：

- 单工具漂移只剔除该工具，其他工具仍可用；
- 列表本身不是 `{tools:[...]}` / 数组 → `MCP_MAPPING_INVALID`；
- raw 列表非空但全部工具漂移 → `MCP_SCHEMA_DRIFT`，连接失败；
- 不再出现「连接成功、0 工具」的静默路径。

### 2.2 资源

`resources/list` 的 `uri` 必填；`name` 可选；`mimeType` 经 alias 映射到内部
`mime_type`；`description` 可选。资源结构损坏按 `MCP_MAPPING_INVALID` 报错。

## 3. 权限规则

Legado 契约声明：

### 3.1 读权限（基础）

10 个基础只读工具必须同时存在：

`list_books`、`get_book`、`list_chapters`、`get_chapter_content`、
`get_reading_progress`、`list_bookmarks`、`list_highlights`、`list_groups`、
`get_reading_stats`、`list_book_sources`

缺任一 → 整台服务器映射失效：`MCP_MAPPING_INVALID`。

### 3.2 联网权限（可选）

契约声明可选工具 `search_online`（Network）。缺失本身不视为映射损坏；
记为「联网权限已撤销」。

- 初始连接：工具列表 = 10 个基础工具；
- 查询 `search_online` → `MCP_PERMISSION_DENIED`；
- 源系统开启联网后重新刷新/连接 → 工具列表 = 11，`search_online` 可用。

### 3.3 写权限（可选）

工具 annotations 中 `readOnlyHint=false` 或 `destructiveHint=true` 归 Write。
刷新时若上次快照有写工具、本次没有 → 写权限撤销。

- 查询被撤销的写工具 → `MCP_PERMISSION_DENIED`；
- 会话中权限切换后调用 → `MCP_PERMISSION_CHANGED`；
- 从未提供过的工具 → `MCP_TOOL_UNAVAILABLE`。

### 3.4 隐私权限（预留）

保留 `Private` 枚举位。Legado 当前工具面无隐私工具；后续源暴露时在契约
`optional_tools` 中补具体工具名，复用同一套撤销/报错规则。

### 3.5 权限快照变化

每次 `tools/list` 生成 `PermissionSnapshot`：

| 变化 | 处理 |
|---|---|
| 基础工具缺失 | 整机失效 `MCP_MAPPING_INVALID` |
| 可选权限工具缺失 | 权限撤销，不使整机失效 |
| 工具 annotations/权限类别变化 | 更新快照；调用时重新校验 |
| 工具名保留但 schema 漂移 | 单工具 `MCP_SCHEMA_DRIFT` |

工具调用返回 `-32601` 或 tool-not-found 时，客户端刷新一次工具面后再判定：
确认源系统权限变化 → `MCP_PERMISSION_CHANGED`；否则返回原 RPC 错误。

## 4. 查询错误码（IPC `AppError` 稳定 code）

| code | 触发 | 恢复 |
|---|---|---|
| `MCP_PROTOCOL_UNSUPPORTED` | 协议版本不在白名单 | 升级 StoryMoss / 检查服务端版本 |
| `MCP_MAPPING_UNSUPPORTED` | 源系统版本超出契约范围 | 更新映射契约 |
| `MCP_MAPPING_INVALID` | 基础工具缺失/列表结构损坏 | 检查源系统版本与工具面 |
| `MCP_SCHEMA_DRIFT` | 工具缺 name/inputSchema；查询 drift 工具 | 更新 alias 或契约版本 |
| `MCP_PERMISSION_DENIED` | 查询已撤销权限对应工具 | 在源系统开启权限后重试 |
| `MCP_PERMISSION_CHANGED` | 调用时发现权限快照变化 | 刷新工具列表/重新连接 |
| `MCP_TOOL_UNAVAILABLE` | 工具从未提供或已移除 | 刷新工具列表 |

错误消息必须包含：server、tool、permission、mapping_version、支持版本/字段；
`ErrorResponse.data` 带结构化诊断。

## 5. HTTP 传输补充规则（Legado 实测）

Legado macOS 的 MCP HTTP 服务器对「header 与 body 分两次 TCP 到达」的请求返回
`400 请求不完整`。StoryMoss 对 `http://` 回环 URL（localhost / 127.0.0.0/8 /
::1）使用原生 HTTP/1 请求：

- header + body 拼成一个 buffer，一次 `write_all` 发出；
- 仍支持 JSON 与 SSE 响应、Content-Length 与 chunked；
- 回环地址绕过系统代理（reqwest 路径也调用 `no_proxy()`）；
- 非回环地址继续走 reqwest，保留用户的代理策略。

## 6. 升级步骤（字段名/权限面变化）

1. 在 `mapping.rs` 新增 alias/权限分类；不覆盖旧契约。
2. 递增 `MCP_MAPPING_VERSION`（如 `1.1.0`）并在本文档登记兼容矩阵。
3. 更新 `parse_tools_payload` / `build_snapshot` 的单测与 Legado live probe。
4. 若源系统版本门变化，调整 `resolve_contract` 的 major.minor 范围。
5. 发布后真机验证：工具数、`get_chapter_content`、权限撤销错误码。

## 7. 验收探针

```bash
# 纯函数/契约
cargo test --lib mcp::

# 真机（token 从环境变量读取，不写仓库）
LEGADO_MCP_TOKEN=<token> cargo test --lib live_legado_mcp_tools_list -- --ignored --nocapture
```

验收点：initialize 识协议 + Legado 0.14.x；tools/list 解析 10/11 个工具；
`get_chapter_content.parameters` 有 JSON Schema；`list_groups` 可调用；
联网关闭时 `search_online` 报 `MCP_PERMISSION_DENIED`。
