# StoryMoss 故障排除指南

## 常见问题

### 1. 无法连接本地服务端口

**症状**: 应用窗口显示"无法连接到本地服务"或空白页面

**原因**:
- 前端无法连接到 Tauri 后端
- 端口被占用
- 防火墙阻止连接

**解决方案**:
```powershell
# 检查端口占用
netstat -an | findstr "5173"

# 清理占用端口的进程
taskkill /F /IM node.exe
taskkill /F /IM storymoss.exe

# 重新启动应用
cd src-frontend && npm run dev
cd src-tauri && cargo tauri dev
```

### 2. React 无限循环错误

**症状**: "Maximum update depth exceeded" 错误

**原因**:
- React Query 重试与 useEffect 形成循环
- 状态更新触发重新渲染

**解决方案**:
- 已修复：数据加载逻辑已移至独立组件
- 限制 React Query 重试次数
- 使用 `useRef` 确保初始化只执行一次

### 3. 编译失败

**症状**: `cargo tauri dev` 编译失败

**解决方案**:
```powershell
# 清理缓存
cargo clean
cd src-frontend && rm -r node_modules && npm install

# 重新编译
cargo build
```

### 4. Tauri CLI 未找到

**症状**: 提示 "cargo tauri" 命令未找到

**解决方案**:
```powershell
cargo install tauri-cli
```

### 5. 续写时看不到「【前文相关原文（检索）】」块

**症状**: 续写正常，但 prompt/日志里没有检索摘录块。

**这是设计行为，不一定是故障**：
- 没有命中相关前文（或向量库还没有该故事的索引）时整块不注入；
- 向量库不可用、embedding 失败、检索出错都会**静默降级**，续写照常进行；
- 开关：配置项 `continue_retrieval_enabled`（默认开），关掉则不检索。

**排查**：确认该故事已有已提交章节（章摘要会投影进向量库）；配置了语义 embedding（Ollama / OpenAI 兼容）时召回更好，本地哈希只是兜底。

### 6. 卷纲生成失败

**症状**: 「运行维护 → 卷纲」点「规划第 N 卷」后报错。

**排查**：卷纲用当前激活模型一次 JSON 调用生成——先确认模型可用（设置 → 模型）；故事尚无全书大纲时卷纲质量会差，建议先补齐大纲；短篇不需要卷纲，不规划不影响任何现有流程。

### 7. 出现「章纲待重规划」提示

**这是预期行为**：编辑过较早的章节后，下游章纲与新正文可能脱节。点「重规划章纲」查看新旧对照——**默认全部不勾选**，只有你勾选的条目会落库（每轮最多 3 章，可多次）；不想处理就点「忽略」。

### 8. 「判官一致率」卡片为空 / 「章法偏好」没有数据

- **判官一致率**：只有编辑判过 blocking（且带原文引用）的章节才会记录；新书或件件通过时为空属正常。可判条数还需要引用 ≥6 字且有成稿可比。
- **章法偏好**：每条手法需要至少 5 次「接受 + 放弃」样本才会影响选择；统计来自主续写路径的 Tab / Esc，样本不足时按原轮换，不影响写作。

---

## 开发模式运行步骤

### 方式一：使用 Tauri Dev（推荐）

**终端 1 - 启动前端开发服务器**:
```powershell
cd src-frontend
npm run dev
```

**终端 2 - 启动 Tauri 应用**:
```powershell
cd src-tauri
cargo tauri dev
```

### 方式二：一键启动

```powershell
cd src-tauri
cargo tauri dev
```
（Tauri 会自动启动前端开发服务器）

---

## 环境要求

- **Node.js**: 18+ (推荐 20+)
- **Rust**: 1.95.0（仓库 `rust-toolchain.toml` 固定，`rustup` 自动安装）
- **Tauri CLI**: 最新版本
- **Windows**: Windows 10 或更高版本

---

## 相关文档

- [项目架构](ARCHITECTURE.md)
- [测试与验收](TESTING.md)
- [用户指南](docs/USER_GUIDE.md)
- [更新日志](CHANGELOG.md)
