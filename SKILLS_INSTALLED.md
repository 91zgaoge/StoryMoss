# 已加载的技能

## 本机技能 (~/.claude/skills/)

### 核心开发技能

#### 1. brainstorming
- **用途**: 需求分析和头脑风暴
- **能力**: 创造性思维、功能规划、用户场景分析
- **位置**: `~/.claude/skills/brainstorming/`

#### 2. find-skills
- **用途**: 技能发现和推荐
- **能力**: 根据需求推荐合适的技能
- **位置**: `~/.claude/skills/find-skills/`

#### 3. skill-creator
- **用途**: 创建自定义技能
- **能力**: 指导如何编写 SKILL.md 和创建技能包
- **位置**: `~/.claude/skills/skill-creator/`

#### 4. systematic-debugging
- **用途**: 系统化的调试方法
- **能力**: Bug 诊断、根因分析、修复验证
- **位置**: `~/.claude/skills/systematic-debugging/`

---

### 前端开发技能

#### 5. frontend-design
- **用途**: 高质量前端界面设计
- **能力**:
  - 独特、高设计质量的 UI 创建
  - 避免"AI 生成感"的通用设计
  - 精致的排版、色彩和动效
  - 大胆的视觉方向选择（极简/极繁/复古未来/编辑风格等）
- **关键原则**:
  - 避免通用字体（Inter, Roboto, Arial）
  - 避免陈词滥调（紫色渐变白底）
  - 每个设计都独一无二
- **位置**: `~/.claude/skills/frontend-design/`

#### 6. react-components
- **用途**: Stitch 设计转 React 组件
- **能力**:
  - 将 Stitch 设计转换为 Vite + React 组件
  - AST 验证和代码质量检查
  - 模块化组件架构
- **依赖**: Stitch MCP 工具
- **位置**: `~/.claude/skills/react-components/`

#### 7. vercel-react-native-skills
- **用途**: React Native 最佳实践
- **能力**: 移动应用开发、性能优化、原生模块
- **位置**: `~/.claude/skills/vercel-react-native-skills/`

---

### 文档和内容技能

#### 8. obsidian-markdown
- **用途**: Obsidian 风格 Markdown 编辑
- **能力**:
  - Wikilinks 内部链接 `[[Note]]`
  - Callouts 标注块 `[!note]`
  - Embeds 嵌入 `![[file]]`
  - Properties/Frontmatter YAML
  - Mermaid 图表
  - LaTeX 数学公式
- **适用场景**: 知识库、技术文档、双链笔记
- **位置**: `~/.claude/skills/obsidian-markdown/`

#### 9. obsidian-bases
- **用途**: Obsidian Bases 数据库视图
- **能力**:
  - 创建 `.base` 文件定义动态视图
  - Table/Cards/List/Map 视图
  - 过滤器、公式、属性配置
  - 类似 Notion 数据库功能
- **适用场景**: 任务追踪、阅读列表、项目管理
- **位置**: `~/.claude/skills/obsidian-bases/`

---

### 文档生成技能

#### 10. office
- **用途**: Office 文档生成 (DOCX/XLSX/PDF/PPTX)
- **能力**:
  - Word 文档生成 (docx)
  - Excel 表格生成 (xlsx)
  - PDF 文档生成 (pdf-lib)
  - PowerPoint 演示文稿 (pptxgenjs)
  - **GB/T 9704-2012 中国公文格式**
- **适用场景**: 
  - 导出小说为 Word/PDF
  - 生成报表和统计
  - 创建官方文档
- **位置**: `~/.claude/skills/office/`

#### 11. json-canvas
- **用途**: Obsidian Canvas 可视化
- **能力**:
  - 创建节点（text/file/link/group）
  - 添加边/连接
  - 思维导图、流程图、项目板
- **适用场景**:
  - 故事大纲可视化
  - 角色关系图
  - 情节流程图
- **位置**: `~/.claude/skills/json-canvas/`

---

## 全局技能 (~/.agents/skills/)

- brainstorming
- find-skills
- office
- react-components
- skill-creator
- systematic-debugging
- vercel-react-native-skills

---

## StoryMoss 项目技能矩阵

| 任务类型 | 推荐技能 | 说明 |
|---------|---------|------|
| **新功能设计** | brainstorming + frontend-design | 头脑风暴后创建精致 UI |
| **UI 改进** | frontend-design | 高质量界面设计 |
| **Bug 修复** | systematic-debugging | 系统化调试方法 |
| **组件开发** | react-components | React 组件最佳实践 |
| **导出功能** | office | DOCX/PDF/EPUB 生成 |
| **技术文档** | obsidian-markdown | 知识库风格文档 |
| **故事可视化** | json-canvas | 情节/角色关系图 |
| **数据管理** | obsidian-bases | 故事/角色数据库视图 |
| **技能扩展** | skill-creator | 创建自定义技能 |

---

## 技能加载记录

- **首次加载**: 2026-04-12
- **最新更新**: 2026-04-12
- **新增技能**: frontend-design, obsidian-markdown, obsidian-bases, office, json-canvas
- **加载者**: 用户授权
- **总计**: 11 个技能已加载

---

## 使用建议

### 写作场景
```
头脑风暴 → brainstorming
UI 设计 → frontend-design
组件实现 → react-components
导出文档 → office
```

### 知识管理场景
```
文档编写 → obsidian-markdown
数据库视图 → obsidian-bases
可视化图表 → json-canvas
```

### 开发场景
```
Bug 修复 → systematic-debugging
功能设计 → brainstorming + frontend-design
技能扩展 → skill-creator
```
