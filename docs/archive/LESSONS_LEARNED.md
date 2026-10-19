# StoryMoss 经验教训档案

> 本文件记录项目修复过程中积累的深层经验与反模式，供后续 AI 助手与开发者参考。
> 最后更新：2026-10-19（基于 v0.73.0 发布镜像并发写损坏——签名校验失败根治）

---

## 经验 1：CI 构建失败必须先用证据定位根因

**来源**：2026-07-05 GitHub Actions Run #875 失败

**现象**：`Build and Test` 工作流全平台失败。

**根因**：不是编译错误或测试失败，而是 `cargo +nightly fmt -- --check` 与 `npm run format:check` 未通过。部分 Rust 测试代码和前端代码未按项目配置折行。

**经验**：
- 遇到 CI 失败，第一步应读取失败日志（`gh run view <id> --log-failed`），不要假设是编译或功能问题。
- 代码格式化失败就是构建失败，必须像编译错误一样零容忍。
- 修复后必须在本地重新运行相同的检查命令，确认通过后再推送。

---

## 经验 2：布尔守卫扩散是架构腐烂的早期信号

**来源**：v0.26.7–v0.26.14 的 `genesisAutoAcceptedRef` 演进

**现象**：`genesisAutoAcceptedRef` 从 v0.26.7 的 7 处使用增长到 v0.26.14 的 13 处使用，散布在 `setGeneratedText`、`ChapterSwitch`、`pipeline-complete`、`handleSmartGeneration`、`Tab 接受` 等多个位置。

**根因**：每次发现新的竞态路径，就通过增加布尔判断来"堵住"，而不是重构状态模型。布尔值只有 `true/false`，无法表达 Genesis 流程实际的三态语义（未开始 / 进行中 / 已投递）。

**经验**：
- 当同一个 `useRef<boolean>` 或标志位在 3 处以上被读写，且判断条件开始依赖调用顺序时，应考虑重构为数态/状态机。
- 状态机能显式表达生命周期阶段，消除"根据多个变量推断当前阶段"的脆弱逻辑。
- 不要让"最小改动"的短期压力把系统推向"散布布尔守卫"反模式。

---

## 经验 3：多写者竞态的根治方案是"单写者 + 状态闸门"

**来源**：Genesis 第一章重复问题

**现象**：`ChapterSwitch`、`pipeline-complete`、`onChapterUpdated`、`smart_execute`、`ContentUpdate`、`AppendContent` 等多个通道都可能把 DB 正文或生成内容写入编辑器，导致同一内容叠加。

**修复**：v0.26.16 引入 `idle → generating → delivered` 三态状态机：
- `generating` 态硬闸门阻塞所有外部 DB 正文加载通道；
- `delivered` 态硬闸门阻塞 `generatedText` 幽灵文本恢复；
- 只有 `smart_execute.final_content` 路径是唯一写者，完成 `generating → delivered` 转换。

**经验**：
- 当同一资源被多个异步通道修改时，优先设计"唯一写者 + 状态闸门"。
- 不要在每个通道加 `if (!flag)`，而要让非法状态转换在架构上不可能发生。

---

## 经验 4：LLM 输出质量问题需要"生成侧 + 消费侧"双层防御

**来源**：v0.26.14 发现 LLM 自身会生成首尾段落重复的模型级循环

**现象**：v0.26.7–v0.26.13 一直认为重复是前端写入了两次；v0.26.14 才通过日志确认是 LLM 输出自身重复，并增加消费侧清理 `trimSelfRepetition`。

**局限**：后处理只能被动裁剪，阈值保守，无法提升模型输出质量。

**修复**：v0.26.16 在消费侧清理基础上，增加生成侧验证闸门：
- 检测 LLM 输出自重复比例；
- 自重复 ≥8% 时用更强 anti-repeat 指令重试一次；
- prompt 模板新增「结构纪律」段，明确禁止首尾回环与整章重复。

**经验**：
- 对于模型输出异常（重复、空对象、JSON 解析失败、思考链污染等），要同时评估：
  1. 消费侧清理/容错
  2. 生成侧验证与重试
  3. prompt 中更明确的约束与反例
- 不要把"模型不会犯错"作为隐含假设。

---

## 经验 5：症状归因错误会让修复越来越偏

**来源**：v0.26.7–v0.26.14 连续多个版本围绕"前端竞态"修复，但问题反复出现

**现象**：每次修复后，用户仍报告第一章重复。每一版都在"DOM 滞后"、"store-editor 失步"、"幽灵容器残留"等假设上加补丁。

**转折**：v0.26.14 通过详细日志分析发现：`append_ai_done` 只触发一次、`append_text_check.occurrences=1`，重复来自 LLM 自身输出。

**经验**：
- 反复复发的 bug，首先要质疑核心假设："真的是 X 导致的吗？"
- 用日志、数据流、最小复现来验证假设，而不是在旧假设上继续堆补丁。
- 区分"前端没有写两次"和"内容本身重复"是定位此类问题的关键。

---

## 经验 6：紧急止血后必须偿还技术债务

**来源**：v0.26.7–v0.26.14 的连续补丁模式

**现象**：生产 bug 的压力下，每个版本都选择最小改动（加一个布尔判断、加一个 ref、加一个去重工具），系统复杂度持续上升。

**结果**：到 v0.26.14 时，`genesisAutoAcceptedRef` 已经难以维护，调用顺序和赋值时机变得脆弱。

**修复**：v0.26.16 停下来做结构性重构，用状态机替代散布布尔守卫，用生成侧验证闸门替代纯后处理。

**经验**：
- 止血后要记录"需要结构性修复的 TODO"。
- 当同一问题反复出现 3 次以上时，应停下来质疑现有架构，而不是继续补丁。
- 结构性修复虽然改动大，但会大幅降低后续维护成本和复发概率。

---

## 经验 7：推送前必须同步版本号与强制文档

**来源**：本次修复发现 `Cargo.toml`、`tauri.conf.json`、`package.json` 版本号停留在 v0.26.14，而 git log 中已有 v0.26.16 提交

**影响**：版本号与 tag、提交消息不一致，会导致 CI 产物版本混乱、发布流程出错。

**经验**：
- 每次发布前检查：`Git tag`、`Cargo.toml`、`src-tauri/tauri.conf.json`、`src-frontend/package.json` 必须一致。
- 按 `AGENTS.md` 要求更新：`README.md`、`CHANGELOG.md`、`AGENTS.md`、`PROJECT_STATUS.md`、`ROADMAP.md`、`ARCHITECTURE.md`、`TESTING.md`、`docs/USER_GUIDE.md`。
- 版本号混乱往往是之前某次推送未严格执行规则的遗留问题，发现后应立即同步。

---

## 经验 8：验证是声明完成的唯一依据

**来源**：本次修复流程

**实践**：
- 修复格式问题后，本地运行 `cargo +nightly fmt -- --check` 与 `npm run format:check`；
- 文档与代码变更后，运行 `cargo check` 与 `npx tsc --noEmit`；
- 推送后通过 `gh run list` 与 `gh run view` 观察 CI 实际状态。

**经验**：
- 不要在没有运行验证命令的情况下说"应该通过了"。
- 任何"已完成"的声明都应附带最近一次运行的命令输出。
- CI 日志中的失败信息往往直接指出解决方案，要完整阅读而不是 skim。

---

## 经验 9：tauri setup 建窗顺序竞态——State 必须先于任何窗口/WebView 创建 manage

**来源**：2026-08-09 Windows 启动闪退根治（v0.33.1–v0.33.5，最终修复 commit `01f5662`）

**现象**：Windows 上双击启动即闪退，无任何日志文件；WER 报 `BEX64` / 异常码 `c0000409` / `P9=7`（`__fastfail`）。macOS 与部分 Windows 机器完全不复发。

**根因**：tauri 2.11.5 的 `app::setup()`（`src-tauri/src/app.rs`）**先创建 `tauri.conf.json` 里声明的配置窗口、后调用用户 `.setup()` 闭包**。Windows 上 wry 创建 WebView2 环境时会泵 Win32 消息循环（故障机实测约 2.3 秒），前端页面加载完成后立即发 IPC 命令，`State<DbPool>` 提取发生在 `.manage()` 之前 → `state() called before manage()` panic（tauri-2.11.5/src/lib.rs:734）→ 该 panic 发生在 WebView2 COM 回调（`extern "C"` 边界）内无法解退 → `panic_cannot_unwind` → 进程直接 abort。macOS WebView 初始化快，竞态窗口几乎为零，因此从不触发。

**修复**：frontstage/backstage 窗口在 `tauri.conf.json` 中设 `create: false`，全部状态 `manage()` 完成后，在 setup 末尾用 `WebviewWindowBuilder::from_config` 显式建窗。

**经验**：
- **铁律：任何 `State` 必须在第一个窗口/WebView 创建之前完成 `manage()`**。tauri 会先建 config 窗口再调 setup 闭包，因此配置窗口必须 `create: false`，由 setup 末尾在所有状态就绪后显式创建。
- **`extern "C"` 边界内 panic = 无日志进程 abort**。COM 回调、WNDPROC、WebView IPC 处理函数等路径上的代码必须保证不 panic（这些边界无法解退，panic 直接 `__fastfail`，连 panic hook 都不一定来得及写盘）。
- **GUI 子系统下 stderr 不可见**。Windows 诊断 GUI 应用崩溃时，临时去掉 `windows_subsystem = "windows"` 切控制台子系统，可从终端直接看到 panic 消息——本次正是靠这一击命中根因（此前两轮修复都在没有 panic 消息的情况下盲猜）。
- **无日志崩溃的取证工具链**：启动面包屑（`startup_trace.rs`，写 `%TEMP%`）+ main 入口早期 panic hook（`install_early_diag()`）+ WER LocalDumps 全量转储 + `minidump-stackwalk` 分析 dmp。
- **Windows setup 阶段触碰 WebView2 异步回调历来高危**：`init_windows`（`src-tauri/src/lib.rs:602`）注释记载此前 setup 里用 WebView2 COM 禁用右键菜单同样触发 BEX64/c0000409——同族问题第二次出现，今后 setup 阶段应避免任何依赖 WebView 异步回调的操作。
- **平台不对称的竞态最容易漏网**："mac 从不复发"不等于"没问题"，时序窗口在慢初始化平台（Windows WebView2）上才会暴露。

---

## 经验 10：发布镜像必须"单通道 + 不可变路径 + 字节级验收"

**来源**：2026-10-15 v0.73.0 用户更新报 `The signature verification failed`

**现象**：0.72.x 用户检查更新到 0.73.0 时下载成功，但安装阶段签名校验失败；GitHub Release 上的稳定版产物本身是好的。

**根因**：同一提交同时触发了 `master` nightly 构建与 `v0.73.0` tag 稳定构建。两个 workflow run 的
`upload-to-website` job 时间完全重叠（02:33:56–02:34:38），向 FTP 同一 `/releases` 目录上传**同名文件**：
- macOS 更新包 `StoryMoss_aarch64.app.tar.gz` / `latest.json` 不含版本号，两条上传流同时写同一路径；
- 官网最终二进制是稳定版长度，但内容被并发写坏，稳定版签名与 nightly 签名**都验不过**；
- 官网 `latest.json` 最后留下的是 nightly 清单（`notes` 为「Nightly 自动构建版本」）。

**修复**：
- `upload-to-website` 只在稳定 tag 上运行（nightly 不再写官网，仍留 GitHub Release）；
- job 级 `concurrency: storymoss-release-upload` 串行化任何剩余上传；
- macOS 更新包在镜像侧改用版本化文件名（`StoryMoss_0.73.0_aarch64.app.tar.gz`），不再有共享可变路径；
- 上传前用 Node 内置 Ed25519 + BLAKE2b 逐产物验 minisign 签名，上传后重新从官网下载再验一遍；
- 新增 `Sync Release to Website` 手工工作流用于镜像救火（拒绝 prerelease、拒绝版本降级）。

**经验**：
- **"构建成功"不等于"发布成功"**。发布链路的正确性必须用客户端同款校验（签名）端到端验证，不能只看 HTTP 200 / 文件大小。
- **同名文件 + 并发写 = 静默损坏**。任何会被多个 run 写的共享路径，要么加互斥，要么改成不可变（带版本/带 run id）路径。
- **nightly 与 stable 不能共用发布通道**，即使版本号相同也不行；通道隔离应做在 workflow 触发条件上，而不是靠约定。
- **签名验证报错先验产物，不要先怀疑密钥**：本次公钥/私钥完全正常，坏的是镜像上的二进制；用同一签名分别校验 GitHub 原始产物与官网镜像即可一步定位。

---

## 反模式清单

| 反模式 | 症状 | 推荐方案 |
|---|---|---|
| 散布布尔守卫 | 同一个布尔 ref 在 3 处以上被读写，判断条件依赖调用顺序 | 重构为数态/状态机 |
| 多写者协调 | 多个异步通道修改同一资源，靠各处加 `if` 保护 | 明确唯一写者 + 状态闸门 |
| 症状驱动补丁 | 同一问题反复出现，每次在旧假设上加新补丁 | 停下来质疑核心假设，做根因分析 |
| 单层模型防御 | 只做消费侧清理或只做 prompt 约束 | 消费侧 + 生成侧 + prompt 约束多层防御 |
| 忽视格式化检查 | 认为 `fmt`/`prettier` 失败是"小问题" | 把格式检查当作编译错误 |
| 状态后于窗口注册 | setup 闭包里 `manage()` 发生在 config 窗口创建之后，前端 IPC 抢跑 | 配置窗口 `create: false`，全部状态 manage 完后 setup 末尾显式建窗 |
| 无日志盲修 | GUI 应用崩溃没有 panic 消息，凭猜测连续打补丁 | 先切控制台子系统/加面包屑拿到 panic 消息，再动手修 |
| 共享可变发布路径 | 多个构建/上传任务写同一文件名，产物静默损坏 | 发布通道隔离 + 版本化文件名 + 上传前后签名校验 |

---

_最后更新: 2026-10-19_
