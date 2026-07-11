# StoryMoss (草苔) 运行指南

## 项目结构

```
v2-rust/
├── src-frontend/       # React + TypeScript 前端
├── src-tauri/          # Tauri + Rust 后端
├── src-core/           # 核心 Rust 库
└── Cargo.toml          # Workspace 配置
```

## 环境要求

- **Rust** 1.70+
- **Node.js** 18+
- **npm** 或 **yarn**

## 快速开始

### 1. 安装依赖

```bash
# 后端依赖
cargo fetch

# 前端依赖
cd src-frontend
npm install
cd ..
```

### 2. 开发模式

**方式一：分别启动（推荐用于开发）**

终端1 - 启动前端开发服务器：
```bash
cd src-frontend
npm run dev
```

终端2 - 启动 Tauri 应用：
```bash
cd src-tauri
cargo tauri dev
```

**方式二：一键启动（Tauri 自动管理前端）**
```bash
cd src-tauri
cargo tauri dev
```

### 3. 构建发布版本

```bash
cd src-tauri
cargo tauri build
```

构建完成后，安装包位于 `src-tauri/target/release/bundle/`。

## 新功能

### React + TypeScript 前端

- **框架**: React 18 + TypeScript 5
- **构建工具**: Vite 5
- **状态管理**: Zustand + React Query
- **样式**: Tailwind CSS
- **UI组件**: 自定义电影感设计系统
- **编辑器**: Monaco Editor (即将集成)
- **图表**: ReactFlow (即将集成)

### LanceDB 向量数据库

- **存储**: 本地 LanceDB 向量数据库
- **嵌入模型**: all-MiniLM-L6-v2 (384维)
- **功能**: 语义搜索、相似度检索

## 开发说明

### 前端开发

```bash
cd src-frontend

# 运行开发服务器
npm run dev

# 类型检查
npm run type-check

# 代码检查
npm run lint

# 构建
npm run build
```

### 后端开发

```bash
# 编译检查
cargo check

# 运行测试
cargo test

# 代码格式化
cargo fmt

# Clippy 检查
cargo clippy -- -D warnings
```

## 配置说明

### LLM 配置

配置文件位置: `~/.config/storymoss/config.json`

```json
{
  "llm": {
    "provider": "openai",
    "api_key": "your-api-key",
    "model": "gpt-4",
    "temperature": 0.7,
    "max_tokens": 4096
  }
}
```

### 向量数据库

向量数据存储在应用数据目录: `~/.config/storymoss/vector_db/`

## 故障排除

### 前端端口被占用

**症状**: 启动时提示 "Port 5173 is already in use"

**解决方案**:
```powershell
# 查找占用端口的进程
netstat -ano | findstr "5173"

# 终止进程 (使用找到的 PID)
taskkill /PID <PID> /F

# 或终止所有相关进程
taskkill /F /IM node.exe
taskkill /F /IM storymoss.exe
```

### 无法连接本地服务端口

**症状**: 应用窗口显示"无法连接到本地服务"或空白页面

**原因**: Windows 上 `localhost` 解析问题

**解决方案**:
- 确保前端开发服务器已启动 (`npm run dev`)
- 检查 `src-tauri/tauri.conf.json` 中的 `devUrl` 配置为 `http://127.0.0.1:5173`
- 参考 [详细修复记录](docs/FIXES_2025_04_11.md)

### React 无限循环错误

**症状**: "Maximum update depth exceeded" 错误

**原因**: 数据加载与状态更新形成循环

**解决方案**:
- 已修复：使用独立的数据加载组件
- 限制 React Query 重试次数
- 参考 [详细修复记录](docs/FIXES_2025_04_11.md)

### Rust 编译失败

```bash
# 清理缓存
cargo clean

# 重新构建
cargo build
```

### Tauri 命令未找到

```bash
# 安装 Tauri CLI
cargo install tauri-cli

# 或使用 npx
npx @tauri-apps/cli dev
```

### 更多问题

参考 [TROUBLESHOOTING.md](TROUBLESHOOTING.md) 获取完整的故障排除指南。

## API 参考

### Tauri 命令

| 命令 | 描述 |
|------|------|
| `list_stories` | 获取故事列表 |
| `create_story` | 创建故事 |
| `update_story` | 更新故事 |
| `delete_story` | 删除故事 |
| `get_story_characters` | 获取角色列表 |
| `create_character` | 创建角色 |
| `get_story_chapters` | 获取章节列表 |
| `update_chapter` | 更新章节 |
| `search_similar` | 向量相似度搜索 (LanceDB) |
| `embed_chapter` | 章节向量化 |
| `get_skills` | 获取技能列表 |
| `enable_skill` | 启用技能 |
| `connect_mcp_server` | 连接 MCP 服务器 |

## 贡献指南

1. Fork 项目
2. 创建功能分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'Add amazing feature'`)
4. 推送分支 (`git push origin feature/amazing-feature`)
5. 创建 Pull Request
