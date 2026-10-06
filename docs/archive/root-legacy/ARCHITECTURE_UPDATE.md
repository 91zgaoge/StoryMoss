# StoryMoss (草苔) 架构更新说明

## 已完成的两项主要改进

### 1. React + TypeScript 前端架构

#### 技术栈
- **框架**: React 18.2 + TypeScript 5.3
- **构建工具**: Vite 5.1
- **状态管理**: Zustand 4.5 + React Query (TanStack Query) 5.20
- **路由**: React Router DOM 6.22
- **样式**: Tailwind CSS 3.4
- **图标**: Lucide React
- **通知**: React Hot Toast

#### 项目结构
```
src-frontend/
├── src/
│   ├── components/       # UI 组件
│   │   ├── ui/          # 基础组件 (Button, Card)
│   │   └── Sidebar.tsx  # 侧边栏导航
│   ├── pages/           # 页面组件
│   │   ├── Dashboard.tsx
│   │   ├── Stories.tsx
│   │   ├── Characters.tsx
│   │   ├── Chapters.tsx
│   │   ├── Skills.tsx
│   │   ├── Mcp.tsx
│   │   └── Settings.tsx
│   ├── hooks/           # React Query Hooks
│   │   ├── useStories.ts
│   │   ├── useCharacters.ts
│   │   └── useVectorSearch.ts
│   ├── stores/          # Zustand 状态管理
│   │   └── appStore.ts
│   ├── services/        # Tauri API 服务
│   │   └── tauri.ts
│   ├── types/           # TypeScript 类型定义
│   │   └── index.ts
│   ├── utils/           # 工具函数
│   │   ├── cn.ts        # className 合并
│   │   └── format.ts    # 格式化函数
│   ├── main.tsx         # React 入口
│   ├── App.tsx          # 根组件
│   └── index.css        # 全局样式
├── package.json
├── tsconfig.json
├── vite.config.ts
└── tailwind.config.js
```

#### 设计系统
- **色彩**: 电影感暗色主题 (cinema-950 ~ cinema-500)
- **强调色**: 金色 (cinema-gold) #d4af37
- **字体**: 
  - 标题: Cinzel (衬线体)
  - 正文: Crimson Pro (衬线体)
  - 代码: JetBrains Mono
- **特效**: Glass Morphism、Film Grain、渐变边框

### 2. LanceDB 向量数据库集成

#### 当前实现
由于 Rust 版本兼容性问题，暂时使用内存向量存储作为过渡方案：

```rust
pub struct LanceVectorStore {
    storage: Arc<Mutex<HashMap<String, Vec<VectorRecord>>>>,
}
```

#### 数据结构
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorRecord {
    pub id: String,
    pub story_id: String,
    pub chapter_id: String,
    pub chapter_number: i32,
    pub text: String,
    pub record_type: String,
    pub embedding: Vec<f32>,  // 384维向量
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub story_id: String,
    pub chapter_id: String,
    pub chapter_number: i32,
    pub text: String,
    pub score: f32,  // 余弦相似度
}
```

#### 新增 Tauri 命令
- `search_similar`: 向量相似度搜索
- `embed_chapter`: 章节内容向量化

#### 嵌入生成
当前使用简化特征提取（待升级为真实嵌入模型）：
```rust
pub fn embed_text(text: &str) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    // 384维特征向量
    // TODO: 接入真实嵌入模型 (如 all-MiniLM-L6-v2)
}
```

## 前端页面预览

### 仪表盘
- 欢迎区域 + 快速操作
- 统计卡片 (故事数、角色数、章节数)
- 活动时间线

### 故事库
- 电影海报风格卡片网格
- 创建/编辑/删除故事
- 类型标签和搜索

### 角色管理
- 角色头像卡片
- 性格特征展示
- 故事关联

### 章节工坊
- 章节列表侧边栏
- 文本编辑器区域
- 状态管理

### 技能工坊
- 分类标签筛选
- 启用/禁用开关
- 内置技能标识

### MCP 连接
- 服务器配置卡片
- 连接状态显示
- 可用工具列表

### 设置
- LLM 提供商配置
- API Key 管理
- 模型参数调节

## 构建和运行

### 开发模式
```bash
# 1. 安装前端依赖
cd src-frontend
npm install

# 2. 启动前端开发服务器
npm run dev

# 3. 启动 Tauri (新终端)
cd ../src-tauri
cargo tauri dev
```

### 生产构建
```bash
cd src-tauri
cargo tauri build
```

## 下一步计划

### 短期 (v2.1.0)
1. 升级 Rust 到 1.88+ 以支持真正的 LanceDB
2. 集成真实嵌入模型 (fastembed/ort)
3. Monaco Editor 集成
4. ReactFlow 大纲可视化

### 中期 (v2.2.0)
1. WebAssembly 技能支持
2. 协同编辑 (OT 算法)
3. 插件市场
4. 云同步功能

## 文件变更摘要

### 新增文件
- `src-frontend/` - 完整 React 前端项目
- `src-tauri/src/vector/lancedb_store.rs` - 向量存储实现
- `src-tauri/src/hooks/useVectorSearch.ts` - 向量搜索 Hook

### 修改文件
- `src-tauri/Cargo.toml` - 添加 lancedb/arrow 依赖 (暂时注释)
- `src-tauri/tauri.conf.json` - 更新构建配置
- `src-tauri/src/main.rs` - 添加向量搜索命令
- `src-tauri/src/vector/mod.rs` - 导出 LanceDB 模块

### 删除/弃用
- 原 `src/` 目录下的纯 JS 实现 (已迁移到 `src-frontend/`)
- `public/` 目录 (被 `src-frontend/dist/` 替代)

## 性能优化

### 前端
- Vite 快速构建和热更新
- React Query 缓存和乐观更新
- 组件懒加载 (待实现)
- 虚拟滚动 (待实现)

### 后端
- 内存向量存储 (快速原型)
- LanceDB 磁盘存储 (生产就绪后切换)
- 异步命令处理
- 连接池复用

---

**更新日期**: 2025-04-11
**状态**: ✅ 前端架构完成，向量数据库基础框架完成
