# 幕前幕后双界面架构实施计划

> **注意**：本计划文档已同步保存到项目目录 `PLAN_Frontstage_Backstage.md`

## 项目背景

StoryMoss (草苔) v2.0 需要实现一个创新的双界面架构：
- **幕前 (Frontstage)**：极简阅读写作界面，类似阅读小说的沉浸体验，使用 Claude 读书感设计系统（暖色调纸张质感）
- **幕后 (Backstage)**：完整工作界面，包含所有创作功能、智能处理与生成等后台技能

## 现状分析

### 已有实现
- ✅ 后端窗口管理模块 (`window/mod.rs`) - 窗口显示/隐藏/切换、事件通信
- ✅ Tauri 配置中定义了 main 和 backstage 两个窗口
- ✅ 幕前界面基础组件 (FrontstageApp, ReaderWriter, AiHintOverlay)
- ✅ 暖色调设计系统颜色定义

### 现有问题
1. **窗口启动逻辑错误**：应用启动时显示 main 窗口而非 frontstage 窗口
2. **窗口 Label 不匹配**：代码查找 "frontstage"/"main"，但配置中是 "main"/"backstage"
3. **样式文件缺失**：`frontstage.css` 被引用但不存在
4. **FrontstageApp 使用 inline styles**：需要迁移到 Tailwind CSS
5. **窗口间切换不完善**：缺少侧边栏快速切换按钮
6. **AI 提示动效未完全实现**：浮动提示和浮现动效需要完善

---

## 实施阶段

### Phase 1: 窗口架构重构
**目标**：修复窗口启动逻辑，确保应用启动进入幕前界面

#### 1.1 修复 Tauri 窗口配置
- **文件**：`src-tauri/tauri.conf.json`
- **修改**：
  - 将 main 窗口改为 frontstage（幕前），启动时可见
  - 将 backstage 窗口改为后台，启动时隐藏
  - 调整窗口尺寸：幕前 1400x900（沉浸式），幕后 1200x800（工作界面）

#### 1.2 修复窗口 Label 引用
- **文件**：`src-tauri/src/window/mod.rs`
- **修改**：
  - `get_frontstage()` 查找 "main" → "frontstage"
  - `get_backstage()` 查找 "main" → "backstage"

#### 1.3 添加窗口初始化逻辑
- **文件**：`src-tauri/src/lib.rs`
- **添加**：在 setup 钩子中：
  - 获取 frontstage 窗口并显示
  - 获取 backstage 窗口并隐藏
  - 确保 frontstage 获得焦点

---

### Phase 2: 幕前界面设计系统实现
**目标**：基于 Claude 读书感设计系统完善幕前界面

#### 2.1 创建 frontstage.css 样式文件
- **文件**：`src-frontend/src/frontstage/styles/frontstage.css`
- **设计系统**：
  ```
  背景：Parchment (#f5f4ed)
  主色调：Terracotta (#c96442)
  文字：Charcoal Warm (#4d4c48) / Olive Gray (#5e5d59)
  字体：Crimson Pro (serif) + Noto Serif SC
  行高：1.8（阅读舒适）
  段落间距：1.5em
  最大宽度：720px（阅读最优宽度）
  ```

#### 2.2 重构 FrontstageApp.tsx
- **文件**：`src-frontend/src/frontstage/FrontstageApp.tsx`
- **重构内容**：
  - 移除 inline styles，使用 Tailwind + frontstage.css
  - 实现三栏布局：侧边栏（可收起）+ 主写作区 + AI 提示区
  - 添加纸张质感背景纹理
  - 优化禅模式（Zen Mode）：全屏无干扰写作

#### 2.3 实现 AI 提示浮现动效
- **文件**：`src-frontend/src/frontstage/components/AiHintOverlay.tsx`
- **动效设计**：
  - 灰色小字（Olive Gray #5e5d59）
  - 如"文思泉涌"般浮现：渐变出现（2s）→ 停留（4s）→ 渐变消失（2s）
  - 随机位置在右侧留白区域
  - 半透明背景模糊效果

#### 2.4 完善 ReaderWriter 组件
- **文件**：`src-frontend/src/frontstage/components/ReaderWriter.tsx`
- **功能**：
  - 类似书籍排版的文本区域
  - 自动保存指示器
  - 字数统计（中文 + 英文）
  - 快捷键提示（Ctrl+Space AI续写，F11 禅模式）

---

### Phase 3: 幕后界面侧边栏集成
**目标**：在幕后界面添加快速切换回幕前的入口

#### 3.1 更新 Sidebar 组件
- **文件**：`src-frontend/src/components/Sidebar.tsx`
- **添加**：
  - 底部 "返回幕前" 按钮
  - 使用 BookOpen 图标
  - 暖色调样式（Terracotta 强调色）

#### 3.2 添加窗口切换逻辑
- **文件**：`src-frontend/src/components/FrontstageLauncher.tsx`
- **更新**：
  - 修复 `open_backstage` 命令调用
  - 添加窗口状态监听
  - 按钮状态同步

---

### Phase 4: 窗口间通信完善
**目标**：实现幕前幕后的实时数据同步

#### 4.1 完善后端事件系统
- **文件**：`src-tauri/src/window/mod.rs`（已存在，需验证）
- **事件类型**：
  - `FrontstageEvent::ContentUpdate` - 内容更新
  - `FrontstageEvent::AiHint` - AI 提示
  - `BackstageEvent::ContentChanged` - 幕前内容变更通知
  - `BackstageEvent::GenerationRequested` - 请求生成内容

#### 4.2 前端事件监听
- **文件**：`src-frontend/src/frontstage/FrontstageApp.tsx`
- **添加**：
  - 监听 `frontstage-update` 事件
  - 监听 `backstage-update` 事件
  - 内容同步逻辑

- **文件**：`src-frontend/src/App.tsx`（幕后界面）
- **添加**：
  - 监听 `backstage-update` 事件
  - 当幕前内容变更时更新状态

---

### Phase 5: AI 流式生成动态效果（核心特色）
**目标**：实现幕后智能生成文字持续流式输出到幕前界面的文思泉涌效果

#### 5.1 流式文字渲染组件
- **文件**：`src-frontend/src/frontstage/components/StreamingText.tsx` - 新建
- **功能设计**：
  - 双状态文本编辑器：用户正文 + AI 生成预览
  - AI 生成文字实时流式显示在用户光标位置
  - 视觉区分：
    - 用户文字：18px, `#4d4c48` (Charcoal Warm)，正常字重
    - AI 生成中：14px, `#87867f` (Stone Gray)，斜体，带淡入动画
    - AI 生成完成：保持淡色，等待用户确认

#### 5.2 流式输出动效
- **打字机效果**：文字逐字出现，间隔 30-80ms（随机模拟真实打字）
- **光标闪烁**：AI 生成区域右侧显示闪烁光标（Terracotta 色）
- **呼吸光晕**：生成区域周围有微弱的 Terracotta 光晕脉冲
- **粒子效果**：文字出现时有微小的墨滴扩散动画（CSS keyframes）

#### 5.3 用户交互控制
- **接受生成**：Tab 键或点击 "采纳" 按钮，AI 文字转正（字号变大，颜色变深）
- **拒绝生成**：Esc 键或点击 "弃用" 按钮，AI 文字淡出消失
- **重新生成**：Ctrl+Shift+Space，清除当前生成，重新流式输出
- **暂停/继续**：Space 键暂停生成，再次按继续

#### 5.4 AI 提示意见系统
- **文件**：`src-frontend/src/frontstage/components/AiSuggestionBubble.tsx` - 新建
- **设计**：
  - 位置：文本右侧留白区域浮动显示
  - 样式：12px, `#5e5d59` (Olive Gray)，圆角气泡，半透明背景
  - 动效：如萤火虫般随机位置浮现，停留 6-10 秒后淡出
  - 类型：情节建议、人物心理、环境描写、节奏提醒

#### 5.5 后端流式 API 支持
- **文件**：`src-tauri/src/llm/mod.rs`（现有模块扩展）
- **添加**：
  - `stream_generate` 命令：返回 Stream/SSE 格式
  - 支持中途取消生成
  - 生成进度事件发送到前端

---

### Phase 6: 细节优化与测试
**目标**：完善细节，确保流畅体验

#### 6.1 启动体验优化
- 添加启动画面或加载指示
- 确保 frontstage 窗口优先显示

#### 6.2 动画与过渡
- 侧边栏展开/收起动画
- 禅模式切换动画
- AI 提示浮现动画
- 流式文字生成动画

#### 6.3 快捷键完善
- Ctrl+Space：触发 AI 续写
- F11：切换禅模式
- Ctrl+Shift+B：切换幕后界面
- ESC：退出禅模式 / 拒绝 AI 生成
- Tab：接受 AI 生成

#### 6.4 测试验证
- 应用启动进入幕前界面 ✓
- 幕前可正常写作 ✓
- AI 提示正常浮现 ✓
- 可从幕前切换到幕后 ✓
- 可从幕后切换回幕前 ✓
- 禅模式工作正常 ✓
- **AI 流式生成效果流畅 ✓**
- **用户可接受/拒绝 AI 生成 ✓**

---

## 设计规范参考

### Claude 读书感设计系统
| 元素 | 值 |
|------|-----|
| 背景色 | `#f5f4ed` (Parchment) |
| 主色调 | `#c96442` (Terracotta) |
| 主文字 | `#4d4c48` (Charcoal Warm) |
| 次要文字 | `#5e5d59` (Olive Gray) |
| 边框 | `#e8e6dc` (Warm Sand) |
| 强调背景 | `#faf9f5` (Ivory) |
| 字体 | Crimson Pro, Noto Serif SC, Georgia |
| 正文字号 | 18px |
| 行高 | 1.8 |
| 段落间距 | 1.5em |
| 内容最大宽度 | 720px |

### 窗口配置
| 窗口 | Label | 尺寸 | 初始状态 |
|------|-------|------|----------|
| 幕前 | frontstage | 1400x900 | 可见 |
| 幕后 | backstage | 1200x800 | 隐藏 |

---

## 文件变更清单

### 后端 (Rust)
1. `src-tauri/tauri.conf.json` - 窗口配置
2. `src-tauri/src/window/mod.rs` - Label 修复
3. `src-tauri/src/lib.rs` - 窗口初始化

### 前端 (幕前)
1. `src-frontend/src/frontstage/styles/frontstage.css` - 新建
2. `src-frontend/src/frontstage/FrontstageApp.tsx` - 重构
3. `src-frontend/src/frontstage/components/ReaderWriter.tsx` - 完善
4. `src-frontend/src/frontstage/components/AiHintOverlay.tsx` - 完善
5. `src-frontend/src/frontstage/components/FrontstageToolbar.tsx` - 完善

### 前端 (幕前 - AI 流式生成)
1. `src-frontend/src/frontstage/components/StreamingText.tsx` - 新建（核心组件）
2. `src-frontend/src/frontstage/components/AiSuggestionBubble.tsx` - 新建（提示气泡）
3. `src-frontend/src/frontstage/hooks/useStreamingGeneration.ts` - 新建（流式生成 Hook）

### 前端 (幕后)
1. `src-frontend/src/components/Sidebar.tsx` - 添加切换按钮
2. `src-frontend/src/components/FrontstageLauncher.tsx` - 修复
3. `src-frontend/src/App.tsx` - 添加事件监听

---

## 实施建议

这是一个**单一方案**，因为架构已经确定，主要是完善实现细节。关键决策：

1. **窗口启动方式**：应用启动直接显示幕前界面（而非先幕后再切换）
2. **设计风格**：严格遵循 Claude 读书感设计系统，与幕后深色界面形成对比
3. **交互模式**：幕前极简（沉浸式），幕后功能完整（工作流）

预计工作量：2-3 个阶段可以并行进行，整体约需 4-6 小时完成。
