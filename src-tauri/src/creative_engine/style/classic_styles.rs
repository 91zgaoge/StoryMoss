//! 经典风格库
//!
//! 内置多位经典作家的 StyleDNA，用户可在幕后一键选择。
//! 这些 DNA 是人工精心构建的参考基线，后续可用 StyleAnalyzer 自动校准。

use super::{classic_styles_extended::*, dna::*};

/// 获取所有内置经典风格（共 52 种）
pub fn get_builtin_styles() -> Vec<StyleDNA> {
    let mut styles = vec![
        jin_yong(),
        zhang_ailing(),
        hemingway(),
        murakami(),
        mo_yan(),
        classical_prose(),
        modern_minimal(),
        noir_detective(),
        wuxia_poetic(),
        romance_flowery(),
        proust(),
        marquez(),
    ];
    styles.extend(get_extended_styles());
    styles
}

/// 金庸风格
/// 特征：武侠术语密集、古典白话对话、四字格+长短交替、武打比喻丰富
pub fn jin_yong() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "金庸".to_string(),
            author: Some("金庸".to_string()),
            description: "新派武侠小说宗师，语言典雅蕴藉，武打描写如诗如画，人物刻画入木三分"
                .to_string(),
            genre_association: Some("武侠".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "high".to_string(),
            abstraction: "balanced".to_string(),
            temporal_quality: "archaic".to_string(),
            preferred_categories: vec![
                "武侠术语".to_string(),
                "古典诗词".to_string(),
                "色彩词汇".to_string(),
                "动词前置".to_string(),
                "兵器名称".to_string(),
            ],
            signature_words: vec![
                "掌风".to_string(),
                "剑气".to_string(),
                "内力".to_string(),
                "轻功".to_string(),
            ],
            avoided_patterns: vec!["现代俚语".to_string(), "网络用语".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 35,
            clause_complexity: "complex".to_string(),
            rhythm_pattern: "四字格+长短交替".to_string(),
            preferred_structures: vec![
                "四字格".to_string(),
                "对偶".to_string(),
                "排比".to_string(),
            ],
            opening_variety: "varied".to_string(),
            punctuation_style: "传统标点，善用顿号、分号".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.08,
            preferred_devices: vec!["比喻".to_string(), "拟人".to_string(), "对仗".to_string()],
            imagery_preference: vec![
                "自然意象".to_string(),
                "武侠意象".to_string(),
                "色彩意象".to_string(),
            ],
            parallelism_frequency: "frequent".to_string(),
            irony_usage: "subtle".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "omniscient".to_string(),
            narrative_distance: "moderate".to_string(),
            interior_monologue_ratio: 0.15,
            omniscience_level: 0.9,
            temporal_handling: "linear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "balanced".to_string(),
            emotion_word_density: 0.06,
            dominant_mood: "侠义豪情".to_string(),
            emotional_arc_pattern: "gradual".to_string(),
            humor_style: "witty".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.35,
            dialogue_length: "moderate".to_string(),
            subtext_ratio: 0.3,
            signature_patterns: vec![
                "说话前先动作描写".to_string(),
                "古典白话".to_string(),
                "江湖切口".to_string(),
            ],
            tag_style: "action_beats".to_string(),
        },
    }
}

/// 张爱玲风格
/// 特征：色彩与触觉词汇密集、比喻精巧、冷漠叙事距离、细节描写极致
pub fn zhang_ailing() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "张爱玲".to_string(),
            author: Some("张爱玲".to_string()),
            description: "华语文坛独特声音，以冷酷笔触描绘人情冷暖，比喻精妙绝伦，色彩触觉通感丰富"
                .to_string(),
            genre_association: Some("都市言情".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "high".to_string(),
            abstraction: "abstract".to_string(),
            temporal_quality: "modern".to_string(),
            preferred_categories: vec![
                "色彩词汇".to_string(),
                "触觉词汇".to_string(),
                "服饰细节".to_string(),
                "器物描写".to_string(),
            ],
            signature_words: vec![
                "苍凉".to_string(),
                "华丽".to_string(),
                "细碎".to_string(),
                "暗红".to_string(),
            ],
            avoided_patterns: vec!["直白抒情".to_string(), "宏大叙事".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 28,
            clause_complexity: "moderate".to_string(),
            rhythm_pattern: "长短错落，短句收尾".to_string(),
            preferred_structures: vec!["意象叠加".to_string(), "倒装".to_string()],
            opening_variety: "varied".to_string(),
            punctuation_style: "善用破折号、省略号".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.12,
            preferred_devices: vec!["比喻".to_string(), "通感".to_string(), "象征".to_string()],
            imagery_preference: vec![
                "色彩意象".to_string(),
                "触觉意象".to_string(),
                "都市意象".to_string(),
            ],
            parallelism_frequency: "rare".to_string(),
            irony_usage: "subtle".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "close_third".to_string(),
            narrative_distance: "distant".to_string(),
            interior_monologue_ratio: 0.25,
            omniscience_level: 0.3,
            temporal_handling: "flashback".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "restrained".to_string(),
            emotion_word_density: 0.04,
            dominant_mood: "苍凉疏离".to_string(),
            emotional_arc_pattern: "gradual".to_string(),
            humor_style: "dark".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.25,
            dialogue_length: "terse".to_string(),
            subtext_ratio: 0.7,
            signature_patterns: vec![
                "对话简短有力".to_string(),
                "潜台词丰富".to_string(),
                "暗含机锋".to_string(),
            ],
            tag_style: "action_beats".to_string(),
        },
    }
}

/// 海明威风格（冰山理论）
/// 特征：短句、省略、极简、只展示不讲述
pub fn hemingway() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "海明威".to_string(),
            author: Some("海明威".to_string()),
            description: "冰山理论创始人，极简主义文风标杆，短句、省略、动作驱动叙事".to_string(),
            genre_association: Some("现实主义".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "low".to_string(),
            abstraction: "concrete".to_string(),
            temporal_quality: "modern".to_string(),
            preferred_categories: vec![
                "动作动词".to_string(),
                "具体名词".to_string(),
                "自然词汇".to_string(),
            ],
            signature_words: vec!["很好".to_string(), "确实".to_string()], // 中文翻译保留的简洁感
            avoided_patterns: vec![
                "形容词堆砌".to_string(),
                "副词".to_string(),
                "心理描写".to_string(),
            ],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 15,
            clause_complexity: "simple".to_string(),
            rhythm_pattern: "短句为主，句号密集".to_string(),
            preferred_structures: vec!["并列短句".to_string(), "省略主语".to_string()],
            opening_variety: "repetitive".to_string(),
            punctuation_style: "极简，少用修饰性标点".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.02,
            preferred_devices: vec!["象征".to_string()],
            imagery_preference: vec!["自然意象".to_string()],
            parallelism_frequency: "rare".to_string(),
            irony_usage: "subtle".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "close_third".to_string(),
            narrative_distance: "intimate".to_string(),
            interior_monologue_ratio: 0.05,
            omniscience_level: 0.1,
            temporal_handling: "linear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "restrained".to_string(),
            emotion_word_density: 0.02,
            dominant_mood: "隐忍克制".to_string(),
            emotional_arc_pattern: "static".to_string(),
            humor_style: "dry".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.45,
            dialogue_length: "terse".to_string(),
            subtext_ratio: 0.6,
            signature_patterns: vec![
                "简短有力".to_string(),
                "重复确认".to_string(),
                "省略含蓄".to_string(),
            ],
            tag_style: "said_only".to_string(),
        },
    }
}

/// 村上春树风格
/// 特征：超现实比喻、孤独感、爵士乐节奏、第一人称疏离
pub fn murakami() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "村上春树".to_string(),
            author: Some("村上春树".to_string()),
            description: "超现实主义与日常感的奇妙融合，孤独主题，音乐性叙事，独特的比喻世界"
                .to_string(),
            genre_association: Some("都市奇幻".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "medium".to_string(),
            abstraction: "balanced".to_string(),
            temporal_quality: "modern".to_string(),
            preferred_categories: vec![
                "音乐术语".to_string(),
                "食物描写".to_string(),
                "身体感知".to_string(),
                "西方文化引用".to_string(),
            ],
            signature_words: vec![
                "井".to_string(),
                "猫".to_string(),
                "爵士".to_string(),
                "孤独".to_string(),
            ],
            avoided_patterns: vec!["宏大叙事".to_string(), "集体主义词汇".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 25,
            clause_complexity: "moderate".to_string(),
            rhythm_pattern: "舒缓流畅，如爵士乐即兴".to_string(),
            preferred_structures: vec!["长句铺陈".to_string(), "突如其来的短句".to_string()],
            opening_variety: "varied".to_string(),
            punctuation_style: "流畅的逗号连接".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.15,
            preferred_devices: vec![
                "比喻".to_string(),
                "拟人".to_string(),
                "超现实意象".to_string(),
            ],
            imagery_preference: vec![
                "都市意象".to_string(),
                "音乐意象".to_string(),
                "自然意象".to_string(),
            ],
            parallelism_frequency: "moderate".to_string(),
            irony_usage: "subtle".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "first_person".to_string(),
            narrative_distance: "close".to_string(),
            interior_monologue_ratio: 0.4,
            omniscience_level: 0.0,
            temporal_handling: "nonlinear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "restrained".to_string(),
            emotion_word_density: 0.03,
            dominant_mood: "孤独疏离".to_string(),
            emotional_arc_pattern: "cyclical".to_string(),
            humor_style: "dry".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.3,
            dialogue_length: "moderate".to_string(),
            subtext_ratio: 0.5,
            signature_patterns: vec![
                "怪诞的日常对话".to_string(),
                "自言自语".to_string(),
                "超现实交流".to_string(),
            ],
            tag_style: "minimal".to_string(),
        },
    }
}

/// 莫言风格
/// 特征：乡土气息浓郁、感官爆炸、魔幻现实主义、粗粝有力
pub fn mo_yan() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "莫言".to_string(),
            author: Some("莫言".to_string()),
            description: "乡土魔幻现实主义，感官描写极致丰富，语言粗粝有力，想象力狂野".to_string(),
            genre_association: Some("乡土文学".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "high".to_string(),
            abstraction: "concrete".to_string(),
            temporal_quality: "mixed".to_string(),
            preferred_categories: vec![
                "乡土方言".to_string(),
                "身体词汇".to_string(),
                "色彩词汇".to_string(),
                "动物隐喻".to_string(),
            ],
            signature_words: vec!["红".to_string(), "血".to_string(), "高粱".to_string()],
            avoided_patterns: vec!["文人雅词".to_string(), "书面语".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 40,
            clause_complexity: "complex".to_string(),
            rhythm_pattern: "汹涌澎湃，一口气到底".to_string(),
            preferred_structures: vec![
                "长句铺排".to_string(),
                "排比".to_string(),
                "倒装".to_string(),
            ],
            opening_variety: "varied".to_string(),
            punctuation_style: "长句密集，感叹号".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.18,
            preferred_devices: vec!["比喻".to_string(), "夸张".to_string(), "通感".to_string()],
            imagery_preference: vec![
                "身体意象".to_string(),
                "乡土意象".to_string(),
                "色彩意象".to_string(),
            ],
            parallelism_frequency: "frequent".to_string(),
            irony_usage: "overt".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "multiple".to_string(),
            narrative_distance: "close".to_string(),
            interior_monologue_ratio: 0.2,
            omniscience_level: 0.8,
            temporal_handling: "nonlinear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "expressive".to_string(),
            emotion_word_density: 0.1,
            dominant_mood: "热烈悲怆".to_string(),
            emotional_arc_pattern: "sudden".to_string(),
            humor_style: "dark".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.2,
            dialogue_length: "verbose".to_string(),
            subtext_ratio: 0.1,
            signature_patterns: vec![
                "方言土语".to_string(),
                "粗口谩骂".to_string(),
                "民间俚语".to_string(),
            ],
            tag_style: "varied_tags".to_string(),
        },
    }
}

/// 古典散文风格
pub fn classical_prose() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "古典散文".to_string(),
            author: None,
            description: "中国传统散文风格，含蓄蕴藉，借景抒情，骈散结合".to_string(),
            genre_association: Some("散文".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "high".to_string(),
            abstraction: "abstract".to_string(),
            temporal_quality: "archaic".to_string(),
            preferred_categories: vec!["自然意象".to_string(), "古典词汇".to_string()],
            signature_words: vec!["之".to_string(), "乎".to_string(), "者".to_string()],
            avoided_patterns: vec!["现代词汇".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 20,
            clause_complexity: "moderate".to_string(),
            rhythm_pattern: "骈散结合".to_string(),
            preferred_structures: vec!["对偶".to_string(), "排比".to_string()],
            opening_variety: "varied".to_string(),
            punctuation_style: "传统".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.1,
            preferred_devices: vec!["比喻".to_string(), "对仗".to_string()],
            imagery_preference: vec!["自然意象".to_string()],
            parallelism_frequency: "frequent".to_string(),
            irony_usage: "subtle".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "first_person".to_string(),
            narrative_distance: "distant".to_string(),
            interior_monologue_ratio: 0.3,
            omniscience_level: 0.0,
            temporal_handling: "linear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "restrained".to_string(),
            emotion_word_density: 0.05,
            dominant_mood: "淡泊宁静".to_string(),
            emotional_arc_pattern: "gradual".to_string(),
            humor_style: "none".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.1,
            dialogue_length: "terse".to_string(),
            subtext_ratio: 0.8,
            signature_patterns: vec!["古语".to_string()],
            tag_style: "minimal".to_string(),
        },
    }
}

/// 现代极简风格
pub fn modern_minimal() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "现代极简".to_string(),
            author: None,
            description: "干净利落的现代文风，去修饰化，信息密度高".to_string(),
            genre_association: Some("现代小说".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "low".to_string(),
            abstraction: "concrete".to_string(),
            temporal_quality: "modern".to_string(),
            preferred_categories: vec!["日常词汇".to_string()],
            signature_words: vec![],
            avoided_patterns: vec!["成语".to_string(), "古典引用".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 18,
            clause_complexity: "simple".to_string(),
            rhythm_pattern: "简洁直接".to_string(),
            preferred_structures: vec!["主谓宾".to_string()],
            opening_variety: "moderate".to_string(),
            punctuation_style: "极简".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.01,
            preferred_devices: vec![],
            imagery_preference: vec![],
            parallelism_frequency: "rare".to_string(),
            irony_usage: "none".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "close_third".to_string(),
            narrative_distance: "close".to_string(),
            interior_monologue_ratio: 0.1,
            omniscience_level: 0.2,
            temporal_handling: "linear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "restrained".to_string(),
            emotion_word_density: 0.02,
            dominant_mood: "冷静客观".to_string(),
            emotional_arc_pattern: "gradual".to_string(),
            humor_style: "none".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.2,
            dialogue_length: "terse".to_string(),
            subtext_ratio: 0.2,
            signature_patterns: vec!["直白".to_string()],
            tag_style: "said_only".to_string(),
        },
    }
}

/// 黑色侦探风格
pub fn noir_detective() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "黑色侦探".to_string(),
            author: None,
            description: "冷硬派侦探小说风格， cynical, 雨夜霓虹，内心独白密集".to_string(),
            genre_association: Some("侦探/ noir".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "medium".to_string(),
            abstraction: "concrete".to_string(),
            temporal_quality: "modern".to_string(),
            preferred_categories: vec!["城市词汇".to_string(), "犯罪术语".to_string()],
            signature_words: vec!["雨".to_string(), "夜".to_string(), "烟".to_string()],
            avoided_patterns: vec![],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 22,
            clause_complexity: "moderate".to_string(),
            rhythm_pattern: "断续，碎片感".to_string(),
            preferred_structures: vec!["短句".to_string(), "省略".to_string()],
            opening_variety: "varied".to_string(),
            punctuation_style: "碎片化".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.06,
            preferred_devices: vec!["比喻".to_string(), "象征".to_string()],
            imagery_preference: vec!["都市意象".to_string(), "阴暗意象".to_string()],
            parallelism_frequency: "rare".to_string(),
            irony_usage: "overt".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "first_person".to_string(),
            narrative_distance: "intimate".to_string(),
            interior_monologue_ratio: 0.5,
            omniscience_level: 0.0,
            temporal_handling: "flashback".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "restrained".to_string(),
            emotion_word_density: 0.03,
            dominant_mood: " cynical 疏离".to_string(),
            emotional_arc_pattern: "static".to_string(),
            humor_style: "dark".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.4,
            dialogue_length: "terse".to_string(),
            subtext_ratio: 0.6,
            signature_patterns: vec!["硬邦邦".to_string(), " cynical 旁白".to_string()],
            tag_style: "action_beats".to_string(),
        },
    }
}

/// 武侠诗意风格
pub fn wuxia_poetic() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "武侠诗意".to_string(),
            author: None,
            description: "古龙式武侠，诗意化叙事，留白多，意境优先于动作".to_string(),
            genre_association: Some("武侠".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "medium".to_string(),
            abstraction: "abstract".to_string(),
            temporal_quality: "archaic".to_string(),
            preferred_categories: vec!["诗词化词汇".to_string(), "江湖用语".to_string()],
            signature_words: vec!["刀".to_string(), "酒".to_string(), "月".to_string()],
            avoided_patterns: vec!["冗长描写".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 16,
            clause_complexity: "simple".to_string(),
            rhythm_pattern: "诗化断句".to_string(),
            preferred_structures: vec!["短句".to_string(), "分行".to_string()],
            opening_variety: "varied".to_string(),
            punctuation_style: "极简".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.1,
            preferred_devices: vec!["比喻".to_string(), "象征".to_string()],
            imagery_preference: vec!["自然意象".to_string(), "孤独意象".to_string()],
            parallelism_frequency: "moderate".to_string(),
            irony_usage: "subtle".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "close_third".to_string(),
            narrative_distance: "distant".to_string(),
            interior_monologue_ratio: 0.1,
            omniscience_level: 0.3,
            temporal_handling: "linear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "restrained".to_string(),
            emotion_word_density: 0.03,
            dominant_mood: "孤独傲然".to_string(),
            emotional_arc_pattern: "gradual".to_string(),
            humor_style: "witty".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.4,
            dialogue_length: "terse".to_string(),
            subtext_ratio: 0.7,
            signature_patterns: vec!["机锋".to_string(), "留白".to_string()],
            tag_style: "minimal".to_string(),
        },
    }
}

/// 浪漫绮丽风格
pub fn romance_flowery() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "浪漫绮丽".to_string(),
            author: None,
            description: "言情小说风格，细腻柔美，情感外露，环境描写精致".to_string(),
            genre_association: Some("言情".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "high".to_string(),
            abstraction: "abstract".to_string(),
            temporal_quality: "modern".to_string(),
            preferred_categories: vec![
                "情感词汇".to_string(),
                "色彩词汇".to_string(),
                "自然描写".to_string(),
            ],
            signature_words: vec!["温柔".to_string(), "心跳".to_string(), "眼神".to_string()],
            avoided_patterns: vec!["粗俗".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 30,
            clause_complexity: "moderate".to_string(),
            rhythm_pattern: "柔美流畅".to_string(),
            preferred_structures: vec!["长句".to_string(), "排比".to_string()],
            opening_variety: "varied".to_string(),
            punctuation_style: "柔和".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.12,
            preferred_devices: vec!["比喻".to_string(), "拟人".to_string()],
            imagery_preference: vec!["自然意象".to_string(), "色彩意象".to_string()],
            parallelism_frequency: "moderate".to_string(),
            irony_usage: "none".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "close_third".to_string(),
            narrative_distance: "intimate".to_string(),
            interior_monologue_ratio: 0.45,
            omniscience_level: 0.2,
            temporal_handling: "linear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "expressive".to_string(),
            emotion_word_density: 0.12,
            dominant_mood: "甜蜜忧伤".to_string(),
            emotional_arc_pattern: "cyclical".to_string(),
            humor_style: "none".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.35,
            dialogue_length: "moderate".to_string(),
            subtext_ratio: 0.4,
            signature_patterns: vec!["温柔".to_string(), "试探".to_string()],
            tag_style: "varied_tags".to_string(),
        },
    }
}

/// 普鲁斯特风格
/// 特征：心理深度、内省、长句、意识流、时间感细腻
pub fn proust() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "普鲁斯特".to_string(),
            author: Some("马塞尔·普鲁斯特".to_string()),
            description: "意识流文学大师，以绵延不绝的长句探索记忆与时间的深渊，心理描写极致细腻"
                .to_string(),
            genre_association: Some("文学/心理".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "high".to_string(),
            abstraction: "abstract".to_string(),
            temporal_quality: "mixed".to_string(),
            preferred_categories: vec![
                "感官词汇".to_string(),
                "记忆触发词".to_string(),
                "色彩与光线".to_string(),
                "时间概念".to_string(),
            ],
            signature_words: vec![
                "记忆".to_string(),
                "时间".to_string(),
                "感觉".to_string(),
                "回忆".to_string(),
            ],
            avoided_patterns: vec!["直白叙述".to_string(), "短促句式".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 80,
            clause_complexity: "complex".to_string(),
            rhythm_pattern: "绵延起伏，层层递进".to_string(),
            preferred_structures: vec![
                "长句嵌套".to_string(),
                "从句堆叠".to_string(),
                "渐进式展开".to_string(),
            ],
            opening_variety: "varied".to_string(),
            punctuation_style: "分号、冒号、破折号大量使用".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.15,
            preferred_devices: vec!["隐喻".to_string(), "通感".to_string(), "联想".to_string()],
            imagery_preference: vec![
                "感官意象".to_string(),
                "时间意象".to_string(),
                "空间意象".to_string(),
            ],
            parallelism_frequency: "moderate".to_string(),
            irony_usage: "subtle".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "first_person".to_string(),
            narrative_distance: "intimate".to_string(),
            interior_monologue_ratio: 0.7,
            omniscience_level: 0.0,
            temporal_handling: "stream".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "expressive".to_string(),
            emotion_word_density: 0.08,
            dominant_mood: "怀旧 melancholy".to_string(),
            emotional_arc_pattern: "gradual".to_string(),
            humor_style: "witty".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.15,
            dialogue_length: "verbose".to_string(),
            subtext_ratio: 0.8,
            signature_patterns: vec!["对话中穿插大量心理活动".to_string(), "间接引语".to_string()],
            tag_style: "varied_tags".to_string(),
        },
    }
}

/// 马尔克斯风格（魔幻现实主义）
/// 特征：氛围哲理、象征、循环时间、奇幻与现实交织
pub fn marquez() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "马尔克斯".to_string(),
            author: Some("加西亚·马尔克斯".to_string()),
            description: "魔幻现实主义巨匠，以宏大的叙事循环和诗意的氛围构建，将奇幻与现实无缝融合"
                .to_string(),
            genre_association: Some("魔幻现实".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "medium".to_string(),
            abstraction: "balanced".to_string(),
            temporal_quality: "mixed".to_string(),
            preferred_categories: vec![
                "自然元素".to_string(),
                "家族称谓".to_string(),
                "色彩词汇".to_string(),
                "魔幻描述".to_string(),
            ],
            signature_words: vec![
                "孤独".to_string(),
                "百年".to_string(),
                "雨".to_string(),
                "家族".to_string(),
            ],
            avoided_patterns: vec!["直白解释".to_string(), "现代科技术语".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 45,
            clause_complexity: "complex".to_string(),
            rhythm_pattern: "史诗般的节奏，长句与短句交替".to_string(),
            preferred_structures: vec![
                "循环结构".to_string(),
                "预言式叙述".to_string(),
                "全景式铺陈".to_string(),
            ],
            opening_variety: "varied".to_string(),
            punctuation_style: "长句为主，段落长".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.12,
            preferred_devices: vec![
                "象征".to_string(),
                "预言".to_string(),
                "夸张".to_string(),
                "魔幻现实".to_string(),
            ],
            imagery_preference: vec![
                "自然意象".to_string(),
                "家族意象".to_string(),
                "时间意象".to_string(),
            ],
            parallelism_frequency: "frequent".to_string(),
            irony_usage: "subtle".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "omniscient".to_string(),
            narrative_distance: "distant".to_string(),
            interior_monologue_ratio: 0.2,
            omniscience_level: 0.9,
            temporal_handling: "nonlinear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "balanced".to_string(),
            emotion_word_density: 0.05,
            dominant_mood: "孤独与宿命".to_string(),
            emotional_arc_pattern: "cyclical".to_string(),
            humor_style: "dark".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.25,
            dialogue_length: "moderate".to_string(),
            subtext_ratio: 0.6,
            signature_patterns: vec![
                "预言式对话".to_string(),
                "家族 gossip".to_string(),
                "口语化史诗感".to_string(),
            ],
            tag_style: "minimal".to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_styles_count() {
        let styles = get_builtin_styles();
        assert_eq!(styles.len(), 52);
    }

    #[test]
    fn test_jin_yong_dna() {
        let dna = jin_yong();
        assert_eq!(dna.meta.name, "金庸");
        assert_eq!(dna.vocabulary.density, "high");
        assert_eq!(dna.syntax.avg_sentence_length, 35);
        assert_eq!(dna.perspective.pov_type, "omniscient");
    }

    #[test]
    fn test_zhang_ailing_dna() {
        let dna = zhang_ailing();
        assert_eq!(dna.meta.name, "张爱玲");
        assert_eq!(dna.emotion.expressiveness, "restrained");
        assert_eq!(dna.dialogue.subtext_ratio, 0.7);
    }

    #[test]
    fn test_hemingway_dna() {
        let dna = hemingway();
        assert_eq!(dna.syntax.avg_sentence_length, 15);
        assert_eq!(dna.dialogue.tag_style, "said_only");
    }

    #[test]
    fn test_proust_dna() {
        let dna = proust();
        assert_eq!(dna.meta.name, "普鲁斯特");
        assert_eq!(dna.syntax.avg_sentence_length, 80);
        assert_eq!(dna.perspective.interior_monologue_ratio, 0.7);
        assert_eq!(dna.perspective.temporal_handling, "stream");
    }

    #[test]
    fn test_marquez_dna() {
        let dna = marquez();
        assert_eq!(dna.meta.name, "马尔克斯");
        assert_eq!(dna.syntax.avg_sentence_length, 45);
        assert_eq!(dna.perspective.pov_type, "omniscient");
        assert_eq!(dna.perspective.temporal_handling, "nonlinear");
    }
}
