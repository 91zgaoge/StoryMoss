//! StyleBlend - 风格混合引擎
//!
//! 支持任意 2-5 个 StyleDNA 按权重组合，生成融合风格 prompt。
//! 核心用于 3风格三角框架（Proust + Hemingway + Márquez），但架构通用。

use serde::{Deserialize, Serialize};

use super::dna::StyleDNA;

/// 混合角色
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BlendRole {
    Dominant,  // 主导 50-80%
    Secondary, // 辅助 10-30%
    Tertiary,  // 辅助 5-20%
}

/// 混合组件
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlendComponent {
    pub dna_id: String,
    pub dna_name: String,
    pub weight: f32,
    pub role: BlendRole,
}

impl BlendComponent {
    pub fn new(dna_id: &str, dna_name: &str, weight: f32) -> Self {
        let role = if weight >= 0.5 {
            BlendRole::Dominant
        } else if weight >= 0.2 {
            BlendRole::Secondary
        } else {
            BlendRole::Tertiary
        };
        Self {
            dna_id: dna_id.to_string(),
            dna_name: dna_name.to_string(),
            weight: weight.clamp(0.0, 1.0),
            role,
        }
    }
}

/// 风格混合配置
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StyleBlendConfig {
    pub name: String,
    pub components: Vec<BlendComponent>,
    pub drift_check_enabled: bool,
}

impl Default for StyleBlendConfig {
    fn default() -> Self {
        Self {
            name: "默认混合".to_string(),
            components: vec![],
            drift_check_enabled: true,
        }
    }
}

impl StyleBlendConfig {
    /// 创建新的空混合配置
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Default::default()
        }
    }

    /// 归一化权重（总和为 1.0）
    pub fn normalize(&mut self) {
        let total: f32 = self.components.iter().map(|c| c.weight).sum();
        if total > 0.0 {
            for c in &mut self.components {
                c.weight = (c.weight / total).clamp(0.0, 1.0);
                c.role = if c.weight >= 0.5 {
                    BlendRole::Dominant
                } else if c.weight >= 0.2 {
                    BlendRole::Secondary
                } else {
                    BlendRole::Tertiary
                };
            }
        }
    }

    /// 验证权重合理性
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.components.is_empty() {
            errors.push("混合配置不能为空".to_string());
        }
        if self.components.len() > 5 {
            errors.push("最多支持 5 个风格混合".to_string());
        }

        let total: f32 = self.components.iter().map(|c| c.weight).sum();
        if (total - 1.0).abs() > 0.01 {
            errors.push(format!("权重总和必须为 1.0，当前为 {:.2}", total));
        }

        let dominant_count = self
            .components
            .iter()
            .filter(|c| c.role == BlendRole::Dominant)
            .count();
        if dominant_count == 0 {
            errors.push("必须有一个主导风格（权重 >= 50%）".to_string());
        }
        if dominant_count > 2 {
            errors.push("主导风格不能超过 2 个".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// 获取主导风格
    pub fn dominant(&self) -> Option<&BlendComponent> {
        self.components
            .iter()
            .filter(|c| c.role == BlendRole::Dominant)
            .max_by(|a, b| a.weight.partial_cmp(&b.weight).unwrap())
    }

    /// 获取辅助风格列表
    pub fn secondaries(&self) -> Vec<&BlendComponent> {
        self.components
            .iter()
            .filter(|c| c.role != BlendRole::Dominant)
            .collect()
    }

    /// 获取指定 DNA
    pub fn get_dna<'a>(&self, dna_id: &str, dnas: &'a [StyleDNA]) -> Option<&'a StyleDNA> {
        dnas.iter()
            .find(|d| d.meta.name == dna_id || d.meta.name == dna_id)
    }

    /// 计算加权平均句长
    pub fn weighted_sentence_length(&self, dnas: &[StyleDNA]) -> f32 {
        self.components
            .iter()
            .filter_map(|c| {
                dnas.iter()
                    .find(|d| d.meta.name == c.dna_name || d.meta.name == c.dna_id)
                    .map(|d| d.syntax.avg_sentence_length as f32 * c.weight)
            })
            .sum()
    }

    /// 计算加权平均对话比例
    pub fn weighted_dialogue_ratio(&self, dnas: &[StyleDNA]) -> f32 {
        self.components
            .iter()
            .filter_map(|c| {
                dnas.iter()
                    .find(|d| d.meta.name == c.dna_name || d.meta.name == c.dna_id)
                    .map(|d| d.dialogue.dialogue_ratio * c.weight)
            })
            .sum()
    }

    /// 计算加权平均比喻密度
    pub fn weighted_metaphor_density(&self, dnas: &[StyleDNA]) -> f32 {
        self.components
            .iter()
            .filter_map(|c| {
                dnas.iter()
                    .find(|d| d.meta.name == c.dna_name || d.meta.name == c.dna_id)
                    .map(|d| d.rhetoric.metaphor_density * c.weight)
            })
            .sum()
    }

    /// 计算加权平均内心独白比例
    pub fn weighted_interior_ratio(&self, dnas: &[StyleDNA]) -> f32 {
        self.components
            .iter()
            .filter_map(|c| {
                dnas.iter()
                    .find(|d| d.meta.name == c.dna_name || d.meta.name == c.dna_id)
                    .map(|d| d.perspective.interior_monologue_ratio * c.weight)
            })
            .sum()
    }

    /// 计算加权情感词密度
    pub fn weighted_emotion_density(&self, dnas: &[StyleDNA]) -> f32 {
        self.components
            .iter()
            .filter_map(|c| {
                dnas.iter()
                    .find(|d| d.meta.name == c.dna_name || d.meta.name == c.dna_id)
                    .map(|d| d.emotion.emotion_word_density * c.weight)
            })
            .sum()
    }

    /// 转换为 LLM prompt 扩展文本
    pub fn to_prompt_extension(&self, dnas: &[StyleDNA]) -> String {
        if self.components.is_empty() || dnas.is_empty() {
            return String::new();
        }

        let mut parts = vec![
            "【风格混合指令】".to_string(),
            "本章节采用多风格融合创作法。以下风格基因按权重组合，\
             你必须在每一句话的选择中体现这种融合："
                .to_string(),
            String::new(),
        ];

        // 主导风格：完整注入
        if let Some(dom) = self.dominant() {
            if let Some(dna) = dnas
                .iter()
                .find(|d| d.meta.name == dom.dna_name || d.meta.name == dom.dna_id)
            {
                parts.push(format!(
                    "【主导风格: {} — 权重 {:.0}%】",
                    dom.dna_name,
                    dom.weight * 100.0
                ));
                parts.push(dna.to_prompt_extension());
                parts.push(String::new());
            }
        }

        // 辅助风格：只注入关键差异维度
        for comp in self.secondaries() {
            if let Some(dna) = dnas
                .iter()
                .find(|d| d.meta.name == comp.dna_name || d.meta.name == comp.dna_id)
            {
                if let Some(dom) = self.dominant() {
                    if let Some(dom_dna) = dnas
                        .iter()
                        .find(|d| d.meta.name == dom.dna_name || d.meta.name == dom.dna_id)
                    {
                        parts.push(format!(
                            "【辅助风格: {} — 权重 {:.0}%】",
                            comp.dna_name,
                            comp.weight * 100.0
                        ));
                        parts.push("关键差异维度（相对于主导风格）：".to_string());

                        // 句长差异
                        let sent_diff = dna.syntax.avg_sentence_length as f32
                            - dom_dna.syntax.avg_sentence_length as f32;
                        if sent_diff.abs() > 5.0 {
                            let direction = if sent_diff > 0.0 { "更长" } else { "更短" };
                            parts.push(format!(
                                "- 句法: {} 的句长比主导风格 {}（{} vs {} 字）",
                                comp.dna_name,
                                direction,
                                dna.syntax.avg_sentence_length,
                                dom_dna.syntax.avg_sentence_length
                            ));
                        }

                        // 对话比例差异
                        let dial_diff =
                            dna.dialogue.dialogue_ratio - dom_dna.dialogue.dialogue_ratio;
                        if dial_diff.abs() > 0.1 {
                            let direction = if dial_diff > 0.0 { "更多" } else { "更少" };
                            parts.push(format!(
                                "- 对话: {} 的对话占比 {}（{:.0}% vs {:.0}%）",
                                comp.dna_name,
                                direction,
                                dna.dialogue.dialogue_ratio * 100.0,
                                dom_dna.dialogue.dialogue_ratio * 100.0
                            ));
                        }

                        // 比喻密度差异
                        let meta_diff =
                            dna.rhetoric.metaphor_density - dom_dna.rhetoric.metaphor_density;
                        if meta_diff.abs() > 0.03 {
                            let direction = if meta_diff > 0.0 {
                                "更密集"
                            } else {
                                "更稀疏"
                            };
                            parts.push(format!(
                                "- 修辞: {} 的比喻密度 {}（{:.1} vs {:.1} 个/千字）",
                                comp.dna_name,
                                direction,
                                dna.rhetoric.metaphor_density,
                                dom_dna.rhetoric.metaphor_density
                            ));
                        }

                        // 内心独白差异
                        let int_diff = dna.perspective.interior_monologue_ratio
                            - dom_dna.perspective.interior_monologue_ratio;
                        if int_diff.abs() > 0.1 {
                            let direction = if int_diff > 0.0 { "更多" } else { "更少" };
                            parts.push(format!(
                                "- 视角: {} 的内心独白 {}（{:.0}% vs {:.0}%）",
                                comp.dna_name,
                                direction,
                                dna.perspective.interior_monologue_ratio * 100.0,
                                dom_dna.perspective.interior_monologue_ratio * 100.0
                            ));
                        }

                        // 情感外露差异
                        if dna.emotion.expressiveness != dom_dna.emotion.expressiveness {
                            parts.push(format!(
                                "- 情感: {} 的情感外露为 {}（主导为 {}）",
                                comp.dna_name,
                                dna.emotion.expressiveness,
                                dom_dna.emotion.expressiveness
                            ));
                        }

                        // 标志性特征
                        if !dna.syntax.rhythm_pattern.is_empty() {
                            parts.push(format!("- 节奏: {}", dna.syntax.rhythm_pattern));
                        }
                        if !dna.vocabulary.signature_words.is_empty() {
                            parts.push(format!(
                                "- 标志性词汇: {}",
                                dna.vocabulary.signature_words.join("、")
                            ));
                        }
                        if !dna.dialogue.signature_patterns.is_empty() {
                            parts.push(format!(
                                "- 对话特征: {}",
                                dna.dialogue.signature_patterns.join("、")
                            ));
                        }

                        parts.push(String::new());
                    }
                }
            }
        }

        // 融合规则
        parts.push("【融合规则】".to_string());
        parts.push("1. 主导风格决定整体基调和叙事节奏".to_string());
        parts.push(
            "2. 辅助风格在特定场景中渗透（如对话场景用辅助风格的节奏，心理场景用辅助风格的深度，\
             环境描写用辅助风格的氛围）"
                .to_string(),
        );
        parts.push("3. 避免风格简单拼接，追求有机融合".to_string());
        parts.push(
            "4. 当两种风格在某一维度冲突时（如主导要求长句，辅助要求短句），以主导风格为准，\
             用辅助风格的「精神」而非「形式」渗透"
                .to_string(),
        );
        parts.push("5. 每次段落转换时，检查是否需要切换风格渗透重点".to_string());

        parts.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blend_normalize() {
        let mut blend = StyleBlendConfig::new("测试");
        blend.components = vec![
            BlendComponent::new("a", "风格A", 0.6),
            BlendComponent::new("b", "风格B", 0.3),
            BlendComponent::new("c", "风格C", 0.1),
        ];
        // 已经是归一化的
        blend.normalize();
        let total: f32 = blend.components.iter().map(|c| c.weight).sum();
        assert!((total - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_blend_validate_ok() {
        let mut blend = StyleBlendConfig::new("测试");
        blend.components = vec![
            BlendComponent::new("a", "风格A", 0.65),
            BlendComponent::new("b", "风格B", 0.20),
            BlendComponent::new("c", "风格C", 0.15),
        ];
        assert!(blend.validate().is_ok());
    }

    #[test]
    fn test_blend_validate_no_dominant() {
        let mut blend = StyleBlendConfig::new("测试");
        blend.components = vec![
            BlendComponent::new("a", "风格A", 0.4),
            BlendComponent::new("b", "风格B", 0.3),
            BlendComponent::new("c", "风格C", 0.3),
        ];
        let err = blend.validate().unwrap_err();
        assert!(err.iter().any(|e| e.contains("主导")));
    }

    #[test]
    fn test_blend_dominant_detection() {
        let mut blend = StyleBlendConfig::new("测试");
        blend.components = vec![
            BlendComponent::new("a", "风格A", 0.65),
            BlendComponent::new("b", "风格B", 0.20),
            BlendComponent::new("c", "风格C", 0.15),
        ];
        assert_eq!(blend.dominant().unwrap().dna_name, "风格A");
        assert_eq!(blend.secondaries().len(), 2);
    }

    #[test]
    fn test_blend_role_auto_assignment() {
        let comp = BlendComponent::new("a", "风格A", 0.7);
        assert_eq!(comp.role, BlendRole::Dominant);

        let comp2 = BlendComponent::new("b", "风格B", 0.25);
        assert_eq!(comp2.role, BlendRole::Secondary);

        let comp3 = BlendComponent::new("c", "风格C", 0.05);
        assert_eq!(comp3.role, BlendRole::Tertiary);
    }
}
