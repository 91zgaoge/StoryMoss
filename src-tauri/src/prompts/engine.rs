//! Prompt Template Engine - 提示词模板引擎
//!
//! 将硬编码的提示词字符串替换为可维护的模板系统。
//! 支持变量替换 {{variable}} 和条件块 {{#if condition}}...{{/if}}

use std::collections::HashMap;

/// 提示词模板
#[derive(Debug, Clone)]
pub struct PromptTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub system_prompt: String,
    pub user_prompt_template: String,
}

/// 模板引擎
pub struct TemplateEngine;

impl TemplateEngine {
    /// 渲染模板，替换 {{key}} 为对应值
    pub fn render(template: &str, variables: &HashMap<String, String>) -> String {
        let mut result = template.to_string();

        // 简单变量替换: {{key}}
        for (key, value) in variables {
            let placeholder = format!("{{{{{}}}}}" , key);
            result = result.replace(&placeholder, value);
        }

        // 清理未替换的变量（保留原样或替换为空）
        // 这里选择保留原样，以便调试

        result
    }

    /// 条件渲染: {{#if key}}...{{/if}}
    pub fn render_with_conditions(template: &str, variables: &HashMap<String, String>) -> String {
        let mut result = template.to_string();

        // 处理条件块
        loop {
            let start_tag = result.find("{{#if ");
            if start_tag.is_none() {
                break;
            }
            let start = start_tag.unwrap();
            let cond_end = result[start..].find("}}").unwrap() + start;
            let condition_key = result[start + 6..cond_end].trim();

            let end_tag = result[cond_end..].find("{{/if}}").unwrap() + cond_end;
            let block_content = result[cond_end + 2..end_tag].to_string();

            let has_value = variables.get(condition_key)
                .map(|v| !v.is_empty() && v != "无" && v != "暂无" && v != "暂无角色信息")
                .unwrap_or(false);

            let replacement = if has_value {
                block_content
            } else {
                String::new()
            };

            result.replace_range(start..end_tag + 7, &replacement);
        }

        // 然后处理普通变量
        Self::render(&result, variables)
    }
}

/// 内置提示词模板库
pub struct PromptLibrary;

impl PromptLibrary {
    /// 获取 Writer Agent 的系统提示词模板
    pub fn writer_system_template() -> &'static str {
        r#"你是一位专业中文小说作家，擅长根据上下文续写和改写内容。

【故事信息】
标题: {{story_title}}
类型: {{genre}}
风格: {{tone}} / 节奏: {{pacing}}

{{#if world_rules}}
【世界观规则】
{{world_rules}}
{{/if}}

{{#if characters}}
【角色信息】
{{characters}}
{{/if}}

{{#if previous_chapters}}
【前文摘要】
{{previous_chapters}}
{{/if}}

{{#if scene_structure}}
【当前场景结构】
{{scene_structure}}
{{/if}}

写作要求：
1. 保持文风一致，情节连贯自然
2. 人物行为符合性格设定
3. 适当加入环境描写和对话
4. 遵守世界观规则
5. 只输出需要的内容，不要添加解释"#
    }

    /// 获取 Writer Agent 的用户提示词模板（续写/创作）
    /// 自动适配开篇（无已有内容）和续写两种场景，不依赖条件块语法。
    pub fn writer_continue_template() -> &'static str {
        r#"请根据以下要求创作内容。

【写作要求】
{{instruction}}

【当前已有内容】
{{current_content}}

说明：如果已有内容为空或"无"，请直接开始创作全新内容；如果已有内容不为空，请在已有内容基础上自然续写。请直接输出正文内容，不要添加解释、总结或重复上下文。"#
    }

    /// 获取 Writer Agent 的用户提示词模板（改写）
    pub fn writer_rewrite_template() -> &'static str {
        r#"请根据以上上下文，对以下文本进行修改。

【修改要求】
{{instruction}}

【需要修改的文本】
{{selected_text}}

【当前章节内容】
{{current_content}}

请只输出修改后的文本，不要添加解释或重复上下文。"#
    }

    /// 获取 Inspector Agent 的系统提示词模板
    pub fn inspector_system_template() -> &'static str {
        r#"你是一位专业的小说质检员，负责检查内容质量、逻辑连贯性和人物一致性。

【故事信息】
标题: {{story_title}}
类型: {{genre}}

{{#if characters}}
【角色设定】
{{characters}}
{{/if}}

检查维度：
1. 逻辑连贯性 - 情节是否通顺，有无矛盾
2. 人物一致性 - 角色行为是否符合设定
3. 文笔质量 - 语言是否流畅，描写是否生动
4. 节奏把控 - 快慢是否得当，有无冗余
5. 世界观一致性 - 是否违反已设定的规则

请按以下 JSON 格式输出质检结果（确保是合法 JSON）：
{
  "score": 85,
  "suggestions": [
    "建议1：具体内容",
    "建议2：具体内容"
  ]
}

score 为 0-100 的整数，suggestions 为改进建议数组。"#
    }

    /// 获取 Outline Planner 的系统提示词模板
    pub fn outline_planner_template() -> &'static str {
        r#"你是一位专业的故事结构顾问，擅长设计故事大纲和章节结构。

【故事创意】
{{premise}}

{{#if characters}}
【角色概要】
{{characters}}
{{/if}}

请使用三幕式结构设计大纲：
1. 第一幕（Setup，25%）：介绍世界、角色、冲突
2. 第二幕（Confrontation，50%）：升级冲突、揭示真相
3. 第三幕（Resolution，25%）：高潮对决、结局收场

每章需要包含：
- 戏剧目标：这章要完成什么叙事使命
- 外部压迫：环境/反派/事件对角色的压迫
- 冲突类型
- 情感弧线

请以清晰的层次结构输出。"#
    }

    /// 获取 Style Checker 的系统提示词模板
    pub fn style_checker_system_template() -> &'static str {
        r#"你是一位专业的文风分析专家，负责对比文本与目标风格的匹配度。

【目标风格 DNA】
{{style_dna}}

【待检查文本】
{{text}}

请从以下维度评估风格匹配度：
1. 平均句长：目标 {{target_sentence_length}} 字，实际如何？
2. 对话比例：目标 {{target_dialogue_ratio}}%，实际如何？
3. 比喻密度：目标 {{target_metaphor_density}}，实际如何？
4. 内心独白比例：目标 {{target_interior_ratio}}%，实际如何？
5. 情感外露程度：目标 {{target_emotion_level}}，实际如何？

请按以下 JSON 格式输出：
{
  "overall_score": 0.85,
  "checks": [
    {"dimension": "句长", "target": 35, "actual": 32, "passed": true, "score": 0.9},
    {"dimension": "对话比", "target": 0.3, "actual": 0.25, "passed": true, "score": 0.8}
  ],
  "issues": ["建议缩短部分长句以匹配目标节奏"]
}

overall_score 为 0.0-1.0，passed 为 true/false。"#
    }

    /// 获取 Commentator（古典评点家）的系统提示词模板
    pub fn commentator_system_template() -> &'static str {
        r#"你是一位博学的古典文学评点家，精通金圣叹式评点。你的任务是为小说段落生成简短精妙的评点。

【故事背景】
标题: {{story_title}}
类型: {{genre}}

【待评点文本】
{{text}}

评点要求：
1. 每条评点 20-40 字，精炼如古人批语
2. 从情节、人物、笔法、意境任一角度切入
3. 使用传统评点语气（如"妙绝！""此处大有深意""笔法顿挫"）
4. 评点前加 ※ 符号
5. 每次生成 1-3 条评点

请只输出评点内容，不要解释。"#
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_render() {
        let template = "Hello, {{name}}!";
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), "World".to_string());
        assert_eq!(TemplateEngine::render(template, &vars), "Hello, World!");
    }

    #[test]
    fn test_conditional_render() {
        let template = "{{#if has_data}}Data: {{data}}{{/if}}End";
        let mut vars = HashMap::new();
        vars.insert("has_data".to_string(), "yes".to_string());
        vars.insert("data".to_string(), "123".to_string());
        assert_eq!(TemplateEngine::render_with_conditions(template, &vars), "Data: 123End");
    }

    #[test]
    fn test_conditional_skip() {
        let template = "{{#if missing}}Data: {{data}}{{/if}}End";
        let mut vars = HashMap::new();
        vars.insert("missing".to_string(), "".to_string());
        assert_eq!(TemplateEngine::render_with_conditions(template, &vars), "End");
    }
}
