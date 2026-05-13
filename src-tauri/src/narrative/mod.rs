//! Narrative Element Model — 统一叙事元素模型
//!
//! 核心理念：无论正向生成（Bootstrap/创世）还是逆向分析（拆书），
//! 操作的叙事元素是同一套抽象。
//!
//! 模块结构：
//! - elements: 统一数据模型（CharacterElement, SceneElement 等）
//! - pipeline: Pipeline trait 和通用基础设施
//! - prompts: 统一 Prompt 模板（生成/提取两用）
//! - genesis: GenesisPipeline — 正向/创世流程
//! - analysis: AnalysisPipeline — 逆向/分析流程
//! - progress: 统一进度事件系统

pub mod elements;
pub mod pipeline;
pub mod prompts;
pub mod genesis;
pub mod analysis;
pub mod progress;
pub mod audit;
pub mod health;

/// 从 LLM 响应中提取 JSON 对象，并修复常见语法错误（尾随逗号、空值、markdown 围栏等）
pub fn extract_and_sanitize_json(content: &str) -> Result<String, String> {
    // 1. 基础提取：找第一个 { 和最后一个 }
    let raw = if let (Some(start), Some(end)) = (content.find('{'), content.rfind('}')) {
        &content[start..=end]
    } else {
        return Err("No JSON object found in response".to_string());
    };

    // 2. 移除 markdown 代码围栏标记（```json ... ```）
    let mut s = raw.to_string();
    for fence in ["```json", "```JSON", "```", "`"] {
        s = s.replace(fence, "");
    }

    // 3. 移除 UTF-8 BOM 和控制字符
    s = s.trim().to_string();
    s = s.replace('\u{feff}', "");

    // 4. 修复尾随逗号：`,]` → `]` 和 `,}` → `}`
    // 使用正则风格的替换：在行尾或空白后的逗号后面紧跟 ] 或 }
    let mut prev;
    loop {
        prev = s.clone();
        s = s.replace(",]", "]");
        s = s.replace(",}", "}");
        s = s.replace(", ]", "]");
        s = s.replace(", }", "}");
        if s == prev {
            break;
        }
    }

    // 5. 修复空值：`: ,` → `: null,`，`: ]` → `: null]`，`: }` → `: null}`
    // 注意：要处理多种空白变体
    for (bad, good) in [
        (": ,", ": null,"),
        (":,", ": null,"),
        (": ]", ": null]"),
        (": ]", ": null]"),
        (": }", ": null}"),
        (":}", ": null}"),
    ] {
        s = s.replace(bad, good);
    }

    // 6. 修复中文智能引号 " " 为 ASCII 引号
    s = s.replace('"', "\"").replace('"', "\"");

    Ok(s)
}

// pub use elements::*;
// pub use pipeline::*;
// pub use progress::*;
