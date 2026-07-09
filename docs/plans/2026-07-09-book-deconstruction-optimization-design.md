# 拆书完善与优化 — 设计决策（实施方案前置）

> **状态：** 草案待实施（2026-07-09）  
> **依据：** [`docs/audits/2026-07-09-book-deconstruction-audit.md`](../audits/2026-07-09-book-deconstruction-audit.md)  
> **目标：** 让拆书结果可靠落库，并显著提升「转故事 / 续写」质量增益；不堆 UI 图表优先。

## 成功标准（可证伪）

| ID | 标准 |
|----|------|
| S1 | 主路径拆书完成后，`reference_books.story_arc` 非空（有有效 LLM 输出时）≥90% |
| S2 | `author` 在元数据可解析时写入 DB，详情页可展示 |
| S3 | 伏笔写入 `foreshadowing_tracker`（story_id=book_id），转故事后可激活/可见 |
| S4 | 绑定 `reference_book_id` 的续写，few-shots 优先向量检索；无向量时降级 Jaccard |
| S5 | 删除或硬失败封印 `BookAnalyzer` 主路径；fallback 仅显式开关或移除 |
| S6 | `pipeline-progress` 按 `book_id` 过滤；USER_GUIDE 承诺与能力对齐 |
| S7 | 新增契约测试 ≥8；`cargo test --lib` 全绿；不破坏 scene-first / Pro 门控 |

## 非目标

- 幕前入口拆书；热路径 quality_gate；把整本参考书塞进 Writer
- 完整「出场频率/高潮曲线」图表（可记 ROADMAP 债务，本方案不做）
- 拆书时跑 StrategySelector / StyleDNA 全量选择（续写侧消费即可）

## 版本切片

| 版本 | 内容 |
|------|------|
| **v0.26.46** | Phase A：持久化闭环（故事线 / 作者 / 伏笔）+ 进度过滤 + 文档降级 |
| **v0.26.47** | Phase B：向量 few-shots + 观测 run 表雏形 |
| **v0.26.48** | Phase C：统一管线（去 legacy）+ 测试加固 + 可选 KG 继承 |

## 不变量

Scene-first；Reference→Active 语义；Pro 门控；architecture_guard；取消不半提交；拆书 LLM 不伪装 silent 淹没幕前（进度按 book 过滤即可）。
