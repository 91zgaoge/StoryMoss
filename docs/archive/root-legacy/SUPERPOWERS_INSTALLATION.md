# Superpowers 技能安装记录

## 安装时间
2025-04-11

## 技能来源
https://github.com/obra/superpowers

## 作者
Jesse Vincent (jesse@fsck.com)

## 许可证
MIT

## 版本
5.0.7

## 安装位置
`~/.claude/skills/superpowers`

---

## 什么是 Superpowers?

Superpowers 是一个完整的软件开发工作流技能集，适用于 Claude Code。它基于一组可组合的"技能"和初始指令构建，确保 AI Agent 遵循经过验证的最佳实践。

### 核心理念

不同于传统的"直接开始写代码"方式，Superpowers 采用以下流程：

1. **需求澄清** - 首先了解用户真正想做什么
2. **规格说明** - 将设计分块展示，足够短以便阅读和消化
3. **实现计划** - 制定清晰的实施计划
4. **子代理驱动开发** - 让子代理处理每个工程任务，审查工作并持续前进
5. **TDD** - 强调真正的红/绿测试驱动开发

---

## 包含的技能 (14个)

### 🧠 规划与思考
| 技能 | 描述 |
|------|------|
| **using-superpowers** | 每次对话开始时使用 - 建立如何查找和使用技能的机制 |
| **brainstorming** | 任何创意工作之前必须使用的技能 - 探索用户意图、需求和设计 |
| **writing-plans** | 当你有规格或需求时，在触碰代码之前使用 |

### 📝 开发与实现
| 技能 | 描述 |
|------|------|
| **test-driven-development** | 实现任何功能或修复错误之前使用 - 先写测试 |
| **subagent-driven-development** | 使用子代理在当前会话中执行实施计划 |
| **dispatching-parallel-agents** | 面对2+个可并行处理的独立任务时使用 |
| **executing-plans** | 当你有书面实施计划在单独会话中执行时使用 |

### 🔧 工具与工作流
| 技能 | 描述 |
|------|------|
| **using-git-worktrees** | 开始需要隔离的特性工作时使用 - 创建隔离的 git worktree |
| **systematic-debugging** | 遇到任何 bug、测试失败或意外行为时使用 |
| **writing-skills** | 创建新技能、编辑现有技能或验证技能时使用 |

### ✅ 验证与完成
| 技能 | 描述 |
|------|------|
| **verification-before-completion** | 在声称工作完成、修复或通过之前使用 - 需要运行验证命令 |
| **requesting-code-review** | 完成任务、实现主要功能或合并前使用 |
| **receiving-code-review** | 收到代码审查反馈时使用，特别是在反馈不清楚时 |
| **finishing-a-development-branch** | 实施完成、所有测试通过后使用 - 指导如何集成工作 |

---

## 核心原则

### 测试驱动开发 (TDD)
```
没有先失败的测试，就没有生产代码
```

**红-绿-重构循环：**
1. 🔴 **红** - 写一个失败的测试
2. 🟢 **绿** - 写最少的代码让测试通过
3. ♻️ **重构** - 清理代码，保持测试通过

**铁律：**
- 如果你没看测试失败，你就不知道它是否测试了正确的东西
- 违反规则的字母就是违反规则的精神
- 在测试之前写了代码？删除它。重新开始。

### YAGNI (You Aren't Gonna Need It)
不要实现你不需要的东西。

### DRY (Don't Repeat Yourself)
不要重复自己。

### 证据先于断言
在声称成功之前，先运行验证命令并确认输出。

---

## 工作流程

### 典型开发会话

```
1. 使用 using-superpowers 建立上下文
2. 使用 brainstorming 探索需求和设计
3. 使用 writing-plans 制定实施计划
4. 使用 test-driven-development 开始开发
5. 使用 subagent-driven-development 或 executing-plans 执行
6. 使用 verification-before-completion 验证
7. 使用 requesting-code-review 请求审查
8. 使用 finishing-a-development-branch 完成
```

---

## 与 Claude Code 集成

Superpowers 可通过以下方式安装：

### 官方插件市场
```bash
/plugin install superpowers@claude-plugins-official
```

### 手动安装 (当前方式)
通过符号链接将技能仓库链接到 Claude 技能目录：
```bash
ln -sf /path/to/superpowers ~/.claude/skills/superpowers
```

---

## 使用示例

### 开始新项目
1. 使用 `using-superpowers` 建立上下文
2. 使用 `brainstorming` 探索项目愿景
3. 使用 `writing-plans` 制定开发计划

### 开发新功能
1. 使用 `using-git-worktrees` 创建隔离的工作区
2. 使用 `test-driven-development` 编写测试
3. 使用 `subagent-driven-development` 实现功能
4. 使用 `verification-before-completion` 验证

### 调试问题
1. 使用 `systematic-debugging` 系统化地定位问题
2. 使用 `test-driven-development` 编写回归测试
3. 使用 `verification-before-completion` 确认修复

---

## 验证安装

```bash
# 检查安装
ls -la ~/.claude/skills/superpowers/

# 查看技能列表
ls ~/.claude/skills/superpowers/skills/

# 查看特定技能
cat ~/.claude/skills/superpowers/skills/test-driven-development/SKILL.md
```

---

## 相关链接

- **GitHub**: https://github.com/obra/superpowers
- **官方市场**: https://claude.com/plugins/superpowers
- **文档**: https://github.com/obra/superpowers/tree/main/docs

---

## 赞助

如果 Superpowers 帮助了你，请考虑赞助作者的 open source 工作：
https://github.com/sponsors/obra

---

*Superpowers 已安装，Claude 现在拥有专业的软件开发超能力！* 🚀
