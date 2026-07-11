# StoryMoss (草苔) 电影感设计系统

基于 Anthropic frontend-design skill 实现的独特视觉设计。

## 设计哲学

### 美学方向
- **主题**: 暗色电影感 + 文学优雅
- **灵感**: 经典好莱坞电影、文学工作室、胶片质感
- **氛围**: 深沉、优雅、专业、创意

## 视觉系统

### 色彩方案

#### 主色调
- `cinema-950`: #050508 - 深邃背景
- `cinema-900`: #0a0a0f - 主背景
- `cinema-850`: #0f0f16 - 卡片背景
- `cinema-800`: #151520 - 次级背景
- `cinema-700`: #1e1e2e - 边框、分隔线

#### 强调色
- `cinema-gold`: #d4af37 - 主要强调（金）
- `cinema-gold-light`: #e8c547 - 高亮
- `cinema-gold-dark`: #b8941f - 深色
- `cinema-velvet`: #4c1d95 - 紫绒色（角色相关）
- `cinema-amber`: #f59e0b - 琥珀色
- `cinema-rust`: #c2410c - 锈色

### 字体系统

#### 字体家族
- **Display**: Cinzel - 标题、品牌
- **Body**: Crimson Pro - 正文、描述
- **Mono**: JetBrains Mono - 代码、标签

#### 字体搭配
```
标题: font-display (Cinzel)
正文: font-body (Crimson Pro)
标签: font-mono (JetBrains Mono)
```

### 组件设计

#### Glass Morphism
```css
.glass-cinema
背景: 半透明深色 + 模糊
边框: 金色/灰色细线
效果: 高端、现代
```

#### 金色边框渐变
```css
.gradient-border
边框: 渐变金色动画
效果: 吸引注意力
用途: 重要卡片、悬停状态
```

#### 按钮样式

**主要按钮**
- 背景: 金色渐变
- 文字: 深色
- 阴影: 金色光晕
- 悬停: 亮度提升

**次要按钮**
- 背景: glass-cinema
- 边框: 细线
- 悬停: 金色边框

#### 卡片设计
- 背景: glass-cinema
- 圆角: rounded-2xl
- 边框: 半透明
- 悬停: 金色边框 + 光晕

### 动画效果

#### 入场动画
- `animate-fade-up`: 淡入上滑
- `animate-fade-in`: 简单淡入
- `animate-slide-left`: 左侧滑入
- stagger 延迟: 0.1s - 0.5s

#### 微交互
- 按钮悬停: 图标旋转、颜色变化
- 卡片悬停: 边框高亮、轻微上浮
- 开关: 平滑滑动

#### 加载动画
- 多层旋转圆环
- 电影胶片图标
- 脉冲效果

### 装饰元素

#### Film Grain
- 覆盖整个页面
- 低透明度 (3%)
- 创造电影质感

#### Gold Lines
- 装饰性分割线
- 渐变透明效果
- 用于标题下方

#### 发光效果
```css
.glow-gold: 金色光晕阴影
.text-glow: 文字发光
```

## 页面设计

### 仪表盘
- Hero 区域 + 引言
- 统计卡片 + 进度条
- 快速操作区
- 活动时间线

### 故事列表
- 电影海报风格卡片
- 类型标签
- 悬停效果

### 角色画廊
- 头像首字母展示
- 性格特征卡片
- 紫绒色主题

### 章节工坊
- 剧本风格布局
- 状态标签
- 编辑器占位

### 技能工坊
- 分类标签
- 开关控制
- 版本信息

---

## 幕前编辑器设计 (Frontstage Editor)

### 纸质平面风格

#### 设计哲学
- **主题**: 温暖纸质质感 + 文学优雅
- **灵感**: 实体书籍、手稿纸张、Claude 界面
- **氛围**: 沉浸、专注、护眼、温暖

#### 色彩方案

##### 纸质色系
- `--parchment`: #f5f4ed - 主背景（羊皮纸）
- `--parchment-dark`: #ebe9e0 - 次级背景
- `--ivory`: #faf9f5 - 高亮背景
- `--warm-sand`: #e8e6dc - 边框、分隔线
- `--border-cream`: #f0eee6 - 细边框

##### 墨水色系
- `--ink`: #2d2c28 - 主文字
- `--charcoal`: #4d4c48 - 次级文字
- `--charcoal-light`: #6b6a65 - 辅助文字
- `--stone-gray`: #87867f - 禁用文字

##### 强调色
- `--terracotta`: #c96442 - 陶土色（主要强调）
- `--terracotta-light`: #d97b5c - 浅陶土
- `--terracotta-dark`: #a85032 - 深陶土
- `--gold`: #c9a86c - 金色点缀

#### 字体系统

##### 字体家族
- **中文正文**: 'Noto Serif SC', 'Source Han Serif CN', 'SimSun', serif
- **英文正文**: 'Crimson Pro', Georgia, serif
- **现代风格**: 'Inter', 'Noto Sans SC', system-ui, sans-serif
- **楷体风格**: 'LXGW WenKai', 'STKaiti', serif

#### 底部工具栏设计

##### 布局
- 位置: 编辑器底部
- 默认状态: 完全隐藏（opacity: 0, translateY: 100%）
- 触发方式: 鼠标悬停编辑器区域
- 动画: 300ms ease-out 平滑滑出

##### 分组卡片设计
```css
.toolbar-group {
  背景: parchment-dark/50
  边框: 1px solid warm-sand
  圆角: rounded-lg
  内边距: px-2 py-1.5
  间距: gap-1
}
```

##### 按钮样式
```css
.toolbar-button {
  背景: parchment
  边框: 1px solid warm-sand
  圆角: rounded
  文字: charcoal, font-serif, text-xs
  内边距: px-2.5 py-1.5
  
  悬停: {
    边框: terracotta/50
    背景: ivory
    阴影: shadow-sm
  }
  
  激活: {
    背景: terracotta/10
    边框: terracotta
    文字: terracotta-dark
    阴影: shadow-inner
  }
}
```

##### 分组标签
- 字体: font-serif italic
- 大小: text-[10px]
- 颜色: stone-gray
- 样式: 全大写 + 字间距
- 文字: "历史"、"格式"、"标题"、"列表"、"其他"

#### 编辑器区域

##### 内容区
- 最大宽度: 900px
- 居中对齐: margin 0 auto
- 内边距: 3rem 2rem
- 最小高度: 80vh

##### ProseMirror 样式
- 字体: 动态 CSS 变量 (--fs-font-family)
- 字号: 动态 CSS 变量 (--fs-font-size)
- 行高: 动态 CSS 变量 (--fs-line-height)
- 段落间距: 1.5em
- 文字对齐: justify

#### 动画效果

##### 工具栏显示/隐藏
```css
.editor-toolbar {
  transition: all 0.3s ease-out;
  opacity: 0;
  transform: translateY(100%);
}

.editor:hover .editor-toolbar {
  opacity: 1;
  transform: translateY(0);
}
```

##### 按钮微交互
- 悬停: 150ms ease 边框颜色变化
- 激活: 内阴影效果
- 禁用: 40% 透明度

### 禅模式

#### 触发方式
- 快捷键: F11
- 退出: F11 或点击底部提示

#### 效果
- 隐藏: frontstage-header, frontstage-sidebar, editor-toolbar
- 编辑器: 全屏展开
- 退出提示: 底部中央，半透明

### 外部连接 (MCP)
- 服务器卡片
- 工具标签
- 操作按钮

### 工作室配置
- 表单布局
- 配置卡片
- 保存按钮

## 响应式设计

### 断点
- 移动端: 默认
- 平板: md (768px)
- 桌面: lg (1024px)

### 适配策略
- 网格: 1 → 2 → 3 列
- 侧边栏: 固定宽度 72
- 字体: 相对大小

## 设计原则

1. **大胆选择**: 独特的金色主题，避免通用暗色模式
2. **层次分明**: 清晰的前景、中景、背景
3. **电影叙事**: 每个页面都有"开场"感觉
4. **细节精致**: 边框、阴影、过渡都精心调校
5. **文学气质**: 字体和引言体现创作工具属性

## 技术实现

### Tailwind 配置
- 自定义颜色
- 自定义字体
- 自定义动画
- 暗色模式

### CSS 特性
- backdrop-filter 模糊
- 渐变背景
- CSS 动画
- 自定义滚动条

### 性能考虑
- CSS 动画使用 GPU 加速
- 半透明层避免重绘
- 图标按需加载

## 文件结构

```
src/
├── views.js          # 视图组件 (电影感设计)
├── main.js           # 应用逻辑
├── mock-tauri.js     # API 模拟
└── index.html        # 入口 (设计系统定义)
```

## 更新日志

### 2025-04-11
- 安装 Anthropic frontend-design skill
- 全面重新设计 UI
- 实现电影感视觉系统
- 添加 Film Grain 效果
- 优化动画和交互

---

*"每一个像素都是故事的一部分"* 🎬
