//! 经典风格扩展库
//!
//! 新增 40 位经典作家与类型文学风格 DNA，覆盖中/日/欧美文学及类型 fiction。
//! 与 classic_styles.rs 合并后总数达 52 种。

use super::dna::*;

// ==================== 中国文学（12种）====================

/// 鲁迅风格
/// 特征：冷峻犀利、白话文运动、讽刺、解剖国民性
pub fn lu_xun() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "鲁迅".to_string(),
            author: Some("鲁迅".to_string()),
            description: "现代文学奠基人，以冷峻犀利的笔触解剖国民性，讽刺辛辣，白描精准，\
                          情感深沉内敛"
                .to_string(),
            genre_association: Some("现实主义/杂文".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "high".to_string(),
            abstraction: "abstract".to_string(),
            temporal_quality: "modern".to_string(),
            preferred_categories: vec![
                "医学隐喻".to_string(),
                "解剖术语".to_string(),
                "冷峻白描".to_string(),
                "讽刺语汇".to_string(),
            ],
            signature_words: vec![
                "铁屋子".to_string(),
                "看客".to_string(),
                "麻木".to_string(),
                "脊梁".to_string(),
            ],
            avoided_patterns: vec![
                "华丽辞藻".to_string(),
                "温情脉脉".to_string(),
                "说教口吻".to_string(),
            ],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 32,
            clause_complexity: "moderate".to_string(),
            rhythm_pattern: "冷峻短句与长句交替，顿挫感强".to_string(),
            preferred_structures: vec![
                "白描".to_string(),
                "反讽".to_string(),
                "递进式质问".to_string(),
            ],
            opening_variety: "varied".to_string(),
            punctuation_style: "善用句号制造停顿，省略号留白".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.07,
            preferred_devices: vec!["反讽".to_string(), "象征".to_string(), "隐喻".to_string()],
            imagery_preference: vec![
                "病态意象".to_string(),
                "铁屋意象".to_string(),
                "暗夜意象".to_string(),
            ],
            parallelism_frequency: "moderate".to_string(),
            irony_usage: "overt".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "close_third".to_string(),
            narrative_distance: "distant".to_string(),
            interior_monologue_ratio: 0.1,
            omniscience_level: 0.4,
            temporal_handling: "flashback".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "restrained".to_string(),
            emotion_word_density: 0.03,
            dominant_mood: "悲愤沉郁".to_string(),
            emotional_arc_pattern: "gradual".to_string(),
            humor_style: "dark".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.2,
            dialogue_length: "terse".to_string(),
            subtext_ratio: 0.8,
            signature_patterns: vec![
                "话中有刺".to_string(),
                "沉默胜过言语".to_string(),
                "方言土语夹杂".to_string(),
            ],
            tag_style: "action_beats".to_string(),
        },
    }
}

/// 老舍风格
/// 特征：京味儿、市井烟火、幽默温厚、口语化
pub fn lao_she() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "老舍".to_string(),
            author: Some("老舍".to_string()),
            description: "京味文学大师，以温厚幽默的笔触描绘市井生活，口语鲜活，人物栩栩如生"
                .to_string(),
            genre_association: Some("京味文学/市民小说".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "medium".to_string(),
            abstraction: "concrete".to_string(),
            temporal_quality: "modern".to_string(),
            preferred_categories: vec![
                "北京方言".to_string(),
                "市井俗语".to_string(),
                "饮食词汇".to_string(),
                "行当术语".to_string(),
            ],
            signature_words: vec![
                "咱".to_string(),
                "得嘞".to_string(),
                "劳驾".to_string(),
                "人缘儿".to_string(),
            ],
            avoided_patterns: vec!["书面雅语".to_string(), "欧化长句".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 22,
            clause_complexity: "simple".to_string(),
            rhythm_pattern: "口语化流畅，如听评书".to_string(),
            preferred_structures: vec![
                "对话推进".to_string(),
                "短句连缀".to_string(),
                "俗语入文".to_string(),
            ],
            opening_variety: "varied".to_string(),
            punctuation_style: "逗号句号为主，贴近口语停顿".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.04,
            preferred_devices: vec!["拟人".to_string(), "反讽".to_string()],
            imagery_preference: vec![
                "市井意象".to_string(),
                "饮食意象".to_string(),
                "季节意象".to_string(),
            ],
            parallelism_frequency: "rare".to_string(),
            irony_usage: "subtle".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "omniscient".to_string(),
            narrative_distance: "moderate".to_string(),
            interior_monologue_ratio: 0.15,
            omniscience_level: 0.8,
            temporal_handling: "linear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "balanced".to_string(),
            emotion_word_density: 0.05,
            dominant_mood: "温厚悲悯".to_string(),
            emotional_arc_pattern: "gradual".to_string(),
            humor_style: "witty".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.45,
            dialogue_length: "moderate".to_string(),
            subtext_ratio: 0.3,
            signature_patterns: vec![
                "京片子".to_string(),
                "俏皮话".to_string(),
                "儿化音".to_string(),
            ],
            tag_style: "varied_tags".to_string(),
        },
    }
}

/// 沈从文风格
/// 特征：湘西风情、田园牧歌、清澈自然、抒情诗化
pub fn shen_congwen() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "沈从文".to_string(),
            author: Some("沈从文".to_string()),
            description: "湘西世界的歌者，以清澈如水的文字描绘边地风情，诗化叙事，人性纯美"
                .to_string(),
            genre_association: Some("乡土抒情/牧歌".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "medium".to_string(),
            abstraction: "concrete".to_string(),
            temporal_quality: "mixed".to_string(),
            preferred_categories: vec![
                "自然词汇".to_string(),
                "湘西方言".to_string(),
                "色彩词汇".to_string(),
                "水意象".to_string(),
            ],
            signature_words: vec![
                "渡船".to_string(),
                "吊脚楼".to_string(),
                "流水".to_string(),
                "山歌".to_string(),
            ],
            avoided_patterns: vec![
                "城市词汇".to_string(),
                "现代术语".to_string(),
                "抽象议论".to_string(),
            ],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 26,
            clause_complexity: "moderate".to_string(),
            rhythm_pattern: "舒缓流畅，如水波荡漾".to_string(),
            preferred_structures: vec![
                "景物铺陈".to_string(),
                "长短句交错".to_string(),
                "民歌化句式".to_string(),
            ],
            opening_variety: "varied".to_string(),
            punctuation_style: "柔和，善用逗号延伸".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.09,
            preferred_devices: vec!["比喻".to_string(), "拟人".to_string(), "通感".to_string()],
            imagery_preference: vec![
                "自然意象".to_string(),
                "水意象".to_string(),
                "乡土意象".to_string(),
            ],
            parallelism_frequency: "moderate".to_string(),
            irony_usage: "none".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "omniscient".to_string(),
            narrative_distance: "moderate".to_string(),
            interior_monologue_ratio: 0.2,
            omniscience_level: 0.7,
            temporal_handling: "linear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "restrained".to_string(),
            emotion_word_density: 0.04,
            dominant_mood: "清澈忧伤".to_string(),
            emotional_arc_pattern: "gradual".to_string(),
            humor_style: "none".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.25,
            dialogue_length: "moderate".to_string(),
            subtext_ratio: 0.4,
            signature_patterns: vec![
                "山歌对唱".to_string(),
                "含蓄表白".to_string(),
                "方言轻柔".to_string(),
            ],
            tag_style: "minimal".to_string(),
        },
    }
}

/// 余华风格
/// 特征：冷酷叙事、暴力美学、简朴直白、黑色幽默
pub fn yu_hua() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "余华".to_string(),
            author: Some("余华".to_string()),
            description: "先锋文学代表，以冷酷直白的笔触直面暴力与死亡，后期转向温情但底色苍凉"
                .to_string(),
            genre_association: Some("先锋文学/现实主义".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "low".to_string(),
            abstraction: "concrete".to_string(),
            temporal_quality: "modern".to_string(),
            preferred_categories: vec![
                "身体词汇".to_string(),
                "死亡意象".to_string(),
                "日常词汇".to_string(),
            ],
            signature_words: vec![
                "活着".to_string(),
                "血".to_string(),
                "死亡".to_string(),
                "忍受".to_string(),
            ],
            avoided_patterns: vec![
                "华丽辞藻".to_string(),
                "心理分析".to_string(),
                "抒情议论".to_string(),
            ],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 20,
            clause_complexity: "simple".to_string(),
            rhythm_pattern: "冷静克制，近乎医学报告".to_string(),
            preferred_structures: vec![
                "主谓宾直叙".to_string(),
                "并列短句".to_string(),
                "重复句式".to_string(),
            ],
            opening_variety: "repetitive".to_string(),
            punctuation_style: "极简，句号为主".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.02,
            preferred_devices: vec!["象征".to_string()],
            imagery_preference: vec!["身体意象".to_string(), "死亡意象".to_string()],
            parallelism_frequency: "rare".to_string(),
            irony_usage: "subtle".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "close_third".to_string(),
            narrative_distance: "distant".to_string(),
            interior_monologue_ratio: 0.05,
            omniscience_level: 0.3,
            temporal_handling: "linear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "restrained".to_string(),
            emotion_word_density: 0.02,
            dominant_mood: "苍凉宿命".to_string(),
            emotional_arc_pattern: "static".to_string(),
            humor_style: "dark".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.3,
            dialogue_length: "terse".to_string(),
            subtext_ratio: 0.2,
            signature_patterns: vec![
                "直白粗粝".to_string(),
                "重复确认".to_string(),
                "沉默间隙".to_string(),
            ],
            tag_style: "said_only".to_string(),
        },
    }
}

/// 王小波风格
/// 特征：幽默反讽、理性思辨、自由洒脱、口语化智慧
pub fn wang_xiaobo() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "王小波".to_string(),
            author: Some("王小波".to_string()),
            description: "特立独行的作家，以幽默反讽包裹理性思辨，文字洒脱自由，充满智慧光芒"
                .to_string(),
            genre_association: Some("当代小说/杂文".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "medium".to_string(),
            abstraction: "balanced".to_string(),
            temporal_quality: "modern".to_string(),
            preferred_categories: vec![
                "科学术语".to_string(),
                "性隐喻".to_string(),
                "黑色幽默".to_string(),
                "逻辑词汇".to_string(),
            ],
            signature_words: vec![
                "有趣".to_string(),
                "智慧".to_string(),
                "自由".to_string(),
                "荒诞".to_string(),
            ],
            avoided_patterns: vec![
                "道学口吻".to_string(),
                "权威腔调".to_string(),
                "悲情叙事".to_string(),
            ],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 28,
            clause_complexity: "moderate".to_string(),
            rhythm_pattern: "跳跃活泼，旁征博引".to_string(),
            preferred_structures: vec![
                "口语化议论".to_string(),
                "故事套故事".to_string(),
                "反讽对比".to_string(),
            ],
            opening_variety: "varied".to_string(),
            punctuation_style: "轻松自然，善用破折号".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.08,
            preferred_devices: vec!["反讽".to_string(), "夸张".to_string(), "比喻".to_string()],
            imagery_preference: vec![
                "荒诞意象".to_string(),
                "性意象".to_string(),
                "科学意象".to_string(),
            ],
            parallelism_frequency: "moderate".to_string(),
            irony_usage: "overt".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "first_person".to_string(),
            narrative_distance: "close".to_string(),
            interior_monologue_ratio: 0.35,
            omniscience_level: 0.1,
            temporal_handling: "nonlinear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "balanced".to_string(),
            emotion_word_density: 0.04,
            dominant_mood: "幽默清醒".to_string(),
            emotional_arc_pattern: "cyclical".to_string(),
            humor_style: "witty".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.3,
            dialogue_length: "moderate".to_string(),
            subtext_ratio: 0.5,
            signature_patterns: vec![
                "机智问答".to_string(),
                "自嘲".to_string(),
                "逻辑诡辩".to_string(),
            ],
            tag_style: "minimal".to_string(),
        },
    }
}

/// 曹雪芹风格
/// 特征：诗词融合、细腻入微、贵族生活、宿命感
pub fn cao_xueqin() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "曹雪芹".to_string(),
            author: Some("曹雪芹".to_string()),
            description: "《红楼梦》作者，以诗词化的语言描绘贵族生活，细腻入微，人物众多而各具神态"
                .to_string(),
            genre_association: Some("古典世情".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "high".to_string(),
            abstraction: "balanced".to_string(),
            temporal_quality: "archaic".to_string(),
            preferred_categories: vec![
                "诗词典故".to_string(),
                "园林词汇".to_string(),
                "服饰器物".to_string(),
                "人情世故".to_string(),
            ],
            signature_words: vec![
                "花落".to_string(),
                "梦".to_string(),
                "情".to_string(),
                "空".to_string(),
            ],
            avoided_patterns: vec!["粗俗口语".to_string(), "直白议论".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 36,
            clause_complexity: "complex".to_string(),
            rhythm_pattern: "骈散结合，诗词化节奏".to_string(),
            preferred_structures: vec![
                "对偶".to_string(),
                "排比".to_string(),
                "诗词嵌入".to_string(),
            ],
            opening_variety: "varied".to_string(),
            punctuation_style: "传统标点，善用逗号延伸".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.11,
            preferred_devices: vec!["隐喻".to_string(), "象征".to_string(), "对仗".to_string()],
            imagery_preference: vec![
                "花卉意象".to_string(),
                "梦境意象".to_string(),
                "器物意象".to_string(),
            ],
            parallelism_frequency: "frequent".to_string(),
            irony_usage: "subtle".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "omniscient".to_string(),
            narrative_distance: "moderate".to_string(),
            interior_monologue_ratio: 0.25,
            omniscience_level: 0.85,
            temporal_handling: "flashback".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "balanced".to_string(),
            emotion_word_density: 0.06,
            dominant_mood: "繁华悲凉".to_string(),
            emotional_arc_pattern: "gradual".to_string(),
            humor_style: "witty".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.35,
            dialogue_length: "moderate".to_string(),
            subtext_ratio: 0.6,
            signature_patterns: vec![
                "机锋往来".to_string(),
                "诗词对答".to_string(),
                "笑语藏针".to_string(),
            ],
            tag_style: "action_beats".to_string(),
        },
    }
}

/// 蒲松龄风格
/// 特征：文言志怪、诡谲幽微、善恶报应、简洁传神
pub fn pu_songling() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "蒲松龄".to_string(),
            author: Some("蒲松龄".to_string()),
            description: "《聊斋志异》作者，以简练文言写鬼神狐妖，诡谲幽微，善恶分明，余韵悠长"
                .to_string(),
            genre_association: Some("文言志怪".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "high".to_string(),
            abstraction: "balanced".to_string(),
            temporal_quality: "archaic".to_string(),
            preferred_categories: vec![
                "文言词汇".to_string(),
                "鬼神术语".to_string(),
                "草木鸟兽".to_string(),
            ],
            signature_words: vec![
                "狐".to_string(),
                "鬼".to_string(),
                "异".to_string(),
                "怪".to_string(),
            ],
            avoided_patterns: vec!["白话俗语".to_string(), "冗长铺陈".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 18,
            clause_complexity: "moderate".to_string(),
            rhythm_pattern: "文言短句，戛然而止".to_string(),
            preferred_structures: vec![
                "史传笔法".to_string(),
                "四字格".to_string(),
                "省略主语".to_string(),
            ],
            opening_variety: "moderate".to_string(),
            punctuation_style: "句读简洁，句号密集".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.06,
            preferred_devices: vec!["拟人".to_string(), "象征".to_string(), "暗示".to_string()],
            imagery_preference: vec![
                "幽冥意象".to_string(),
                "自然意象".to_string(),
                "幻化意象".to_string(),
            ],
            parallelism_frequency: "moderate".to_string(),
            irony_usage: "subtle".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "omniscient".to_string(),
            narrative_distance: "distant".to_string(),
            interior_monologue_ratio: 0.05,
            omniscience_level: 0.9,
            temporal_handling: "flashback".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "restrained".to_string(),
            emotion_word_density: 0.04,
            dominant_mood: "幽微诡谲".to_string(),
            emotional_arc_pattern: "sudden".to_string(),
            humor_style: "none".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.2,
            dialogue_length: "terse".to_string(),
            subtext_ratio: 0.7,
            signature_patterns: vec![
                "文言对白".to_string(),
                "寓意式对话".to_string(),
                "画龙点睛".to_string(),
            ],
            tag_style: "minimal".to_string(),
        },
    }
}

/// 苏轼风格
/// 特征：豪放洒脱、古文功底、旷达人生观、兼融儒释道
pub fn su_shi() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "苏轼".to_string(),
            author: Some("苏轼".to_string()),
            description: "北宋文豪，文风豪放洒脱，诗词文赋皆精，旷达通透，议论风生".to_string(),
            genre_association: Some("古典散文/词".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "high".to_string(),
            abstraction: "balanced".to_string(),
            temporal_quality: "archaic".to_string(),
            preferred_categories: vec![
                "典故".to_string(),
                "自然意象".to_string(),
                "哲理词汇".to_string(),
                "饮食词汇".to_string(),
            ],
            signature_words: vec![
                "明月".to_string(),
                "大江".to_string(),
                "浮生".to_string(),
                "旷达".to_string(),
            ],
            avoided_patterns: vec!["矫揉造作".to_string(), "悲戚过度".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 24,
            clause_complexity: "moderate".to_string(),
            rhythm_pattern: "疏朗开阔，跌宕有致".to_string(),
            preferred_structures: vec![
                "散文化长句".to_string(),
                "议论排比".to_string(),
                "情景交融".to_string(),
            ],
            opening_variety: "varied".to_string(),
            punctuation_style: "传统标点，舒展自然".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.08,
            preferred_devices: vec!["比喻".to_string(), "用典".to_string(), "对仗".to_string()],
            imagery_preference: vec![
                "自然意象".to_string(),
                "历史意象".to_string(),
                "人生意象".to_string(),
            ],
            parallelism_frequency: "moderate".to_string(),
            irony_usage: "subtle".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "first_person".to_string(),
            narrative_distance: "close".to_string(),
            interior_monologue_ratio: 0.35,
            omniscience_level: 0.2,
            temporal_handling: "flashback".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "balanced".to_string(),
            emotion_word_density: 0.05,
            dominant_mood: "旷达洒脱".to_string(),
            emotional_arc_pattern: "gradual".to_string(),
            humor_style: "witty".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.15,
            dialogue_length: "terse".to_string(),
            subtext_ratio: 0.6,
            signature_patterns: vec![
                "哲理问答".to_string(),
                "典故引用".to_string(),
                "旷达之语".to_string(),
            ],
            tag_style: "minimal".to_string(),
        },
    }
}

/// 阿城风格
/// 特征：古典白描、淡泊节制、智慧内敛、棋道人生
pub fn a_cheng() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "阿城".to_string(),
            author: Some("阿城".to_string()),
            description: "当代作家，以极度克制的白描笔法写知青生活，古典韵味，淡泊中见深邃"
                .to_string(),
            genre_association: Some("当代小说".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "medium".to_string(),
            abstraction: "concrete".to_string(),
            temporal_quality: "mixed".to_string(),
            preferred_categories: vec![
                "器物词汇".to_string(),
                "棋道术语".to_string(),
                "饮食词汇".to_string(),
                "古典白描".to_string(),
            ],
            signature_words: vec![
                "棋".to_string(),
                "树".to_string(),
                "吃".to_string(),
                "闲".to_string(),
            ],
            avoided_patterns: vec![
                "抒情议论".to_string(),
                "心理分析".to_string(),
                "形容词堆砌".to_string(),
            ],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 19,
            clause_complexity: "simple".to_string(),
            rhythm_pattern: "冲淡平和，近乎古人笔记".to_string(),
            preferred_structures: vec![
                "白描".to_string(),
                "短句".to_string(),
                "动作先行".to_string(),
            ],
            opening_variety: "moderate".to_string(),
            punctuation_style: "极简，句号为主".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.02,
            preferred_devices: vec!["白描".to_string()],
            imagery_preference: vec!["日常意象".to_string(), "器物意象".to_string()],
            parallelism_frequency: "rare".to_string(),
            irony_usage: "subtle".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "close_third".to_string(),
            narrative_distance: "distant".to_string(),
            interior_monologue_ratio: 0.05,
            omniscience_level: 0.3,
            temporal_handling: "linear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "restrained".to_string(),
            emotion_word_density: 0.02,
            dominant_mood: "淡泊通透".to_string(),
            emotional_arc_pattern: "static".to_string(),
            humor_style: "dry".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.25,
            dialogue_length: "terse".to_string(),
            subtext_ratio: 0.5,
            signature_patterns: vec![
                "简短有力".to_string(),
                "动作伴随".to_string(),
                "留白".to_string(),
            ],
            tag_style: "minimal".to_string(),
        },
    }
}

/// 白先勇风格
/// 特征：台北人、苍凉精致、旧贵族、细腻心理
pub fn bai_xianyong() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "白先勇".to_string(),
            author: Some("白先勇".to_string()),
            description: "台湾作家，以精致细腻的笔触写流亡贵族的没落，苍凉华美，心理深度"
                .to_string(),
            genre_association: Some("现代小说".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "high".to_string(),
            abstraction: "abstract".to_string(),
            temporal_quality: "modern".to_string(),
            preferred_categories: vec![
                "色彩词汇".to_string(),
                "服饰细节".to_string(),
                "戏曲术语".to_string(),
                "贵族用语".to_string(),
            ],
            signature_words: vec![
                "游园".to_string(),
                "惊梦".to_string(),
                "繁华".to_string(),
                "没落".to_string(),
            ],
            avoided_patterns: vec!["粗俗口语".to_string(), "直白议论".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 30,
            clause_complexity: "moderate".to_string(),
            rhythm_pattern: "舒缓精致，如昆曲水磨调".to_string(),
            preferred_structures: vec![
                "长句铺陈".to_string(),
                "倒叙".to_string(),
                "意象叠加".to_string(),
            ],
            opening_variety: "varied".to_string(),
            punctuation_style: "精致，善用逗号与分号".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.1,
            preferred_devices: vec!["象征".to_string(), "比喻".to_string(), "通感".to_string()],
            imagery_preference: vec![
                "戏曲意象".to_string(),
                "色彩意象".to_string(),
                "繁华意象".to_string(),
            ],
            parallelism_frequency: "moderate".to_string(),
            irony_usage: "subtle".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "close_third".to_string(),
            narrative_distance: "close".to_string(),
            interior_monologue_ratio: 0.3,
            omniscience_level: 0.3,
            temporal_handling: "flashback".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "balanced".to_string(),
            emotion_word_density: 0.06,
            dominant_mood: "苍凉精致".to_string(),
            emotional_arc_pattern: "gradual".to_string(),
            humor_style: "none".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.3,
            dialogue_length: "moderate".to_string(),
            subtext_ratio: 0.6,
            signature_patterns: vec![
                "古典白话".to_string(),
                "戏曲化语言".to_string(),
                "欲言又止".to_string(),
            ],
            tag_style: "action_beats".to_string(),
        },
    }
}

/// 钱锺书风格
/// 特征：博学讽刺、机智俏皮、比喻密集、学贯中西
pub fn qian_zhongshu() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "钱锺书".to_string(),
            author: Some("钱锺书".to_string()),
            description: "学贯中西的讽刺大师，以博学和机智写知识分子的困境，比喻奇警，旁征博引"
                .to_string(),
            genre_association: Some("讽刺小说".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "high".to_string(),
            abstraction: "abstract".to_string(),
            temporal_quality: "modern".to_string(),
            preferred_categories: vec![
                "学术术语".to_string(),
                "西方典故".to_string(),
                "古典诗词".to_string(),
                "讽刺语汇".to_string(),
            ],
            signature_words: vec![
                "围城".to_string(),
                "文凭".to_string(),
                "留学".to_string(),
                "教授".to_string(),
            ],
            avoided_patterns: vec!["乡土土语".to_string(), "直白抒情".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 38,
            clause_complexity: "complex".to_string(),
            rhythm_pattern: "洋洋洒洒，妙趣横生".to_string(),
            preferred_structures: vec![
                "长句嵌套".to_string(),
                "对比反讽".to_string(),
                "引经据典".to_string(),
            ],
            opening_variety: "varied".to_string(),
            punctuation_style: "繁复精致，善用逗号分号".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.14,
            preferred_devices: vec![
                "比喻".to_string(),
                "反讽".to_string(),
                "用典".to_string(),
                "夸张".to_string(),
            ],
            imagery_preference: vec![
                "学术意象".to_string(),
                "西方意象".to_string(),
                "婚姻意象".to_string(),
            ],
            parallelism_frequency: "moderate".to_string(),
            irony_usage: "overt".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "omniscient".to_string(),
            narrative_distance: "distant".to_string(),
            interior_monologue_ratio: 0.2,
            omniscience_level: 0.9,
            temporal_handling: "linear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "restrained".to_string(),
            emotion_word_density: 0.04,
            dominant_mood: "机智讽刺".to_string(),
            emotional_arc_pattern: "gradual".to_string(),
            humor_style: "witty".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.35,
            dialogue_length: "verbose".to_string(),
            subtext_ratio: 0.7,
            signature_patterns: vec![
                "机锋往来".to_string(),
                "中西夹杂".to_string(),
                "知识分子腔".to_string(),
            ],
            tag_style: "varied_tags".to_string(),
        },
    }
}

/// 郁达夫风格
/// 特征：抒情自叙、感伤独白、浪漫颓废、心理暴露
pub fn yu_dafu() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "郁达夫".to_string(),
            author: Some("郁达夫".to_string()),
            description: "创造社代表，以自叙传体式写青年苦闷，感伤抒情，心理暴露，浪漫颓废"
                .to_string(),
            genre_association: Some("抒情小说".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "high".to_string(),
            abstraction: "abstract".to_string(),
            temporal_quality: "modern".to_string(),
            preferred_categories: vec![
                "情感词汇".to_string(),
                "自然意象".to_string(),
                "病态词汇".to_string(),
                "西洋语汇".to_string(),
            ],
            signature_words: vec![
                "沉沦".to_string(),
                "孤独".to_string(),
                "眼泪".to_string(),
                "秋".to_string(),
            ],
            avoided_patterns: vec!["客观叙事".to_string(), "社会分析".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 34,
            clause_complexity: "complex".to_string(),
            rhythm_pattern: "抒情长句，叹词频繁".to_string(),
            preferred_structures: vec![
                "独白".to_string(),
                "感叹".to_string(),
                "排比抒情".to_string(),
            ],
            opening_variety: "moderate".to_string(),
            punctuation_style: "感叹号、省略号、破折号密集".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.1,
            preferred_devices: vec!["拟人".to_string(), "象征".to_string(), "呼告".to_string()],
            imagery_preference: vec![
                "自然意象".to_string(),
                "病态意象".to_string(),
                "孤独意象".to_string(),
            ],
            parallelism_frequency: "frequent".to_string(),
            irony_usage: "none".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "first_person".to_string(),
            narrative_distance: "intimate".to_string(),
            interior_monologue_ratio: 0.6,
            omniscience_level: 0.0,
            temporal_handling: "linear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "expressive".to_string(),
            emotion_word_density: 0.1,
            dominant_mood: "感伤颓废".to_string(),
            emotional_arc_pattern: "cyclical".to_string(),
            humor_style: "none".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.15,
            dialogue_length: "terse".to_string(),
            subtext_ratio: 0.5,
            signature_patterns: vec![
                "自言自语".to_string(),
                "欲说还休".to_string(),
                "书信体".to_string(),
            ],
            tag_style: "minimal".to_string(),
        },
    }
}

// ==================== 日本文学（6种）====================

/// 川端康成风格
/// 特征：物哀、新感觉派、纤细、色彩与季节
pub fn kawabata_yasunari() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "川端康成".to_string(),
            author: Some("川端康成".to_string()),
            description: "诺贝尔文学奖得主，以纤细敏感的笔触捕捉日本美学的精髓，物哀、幽玄、余情"
                .to_string(),
            genre_association: Some("新感觉派/纯文学".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "high".to_string(),
            abstraction: "abstract".to_string(),
            temporal_quality: "modern".to_string(),
            preferred_categories: vec![
                "色彩词汇".to_string(),
                "季节用语".to_string(),
                "身体感知".to_string(),
                "传统美学".to_string(),
            ],
            signature_words: vec![
                "雪".to_string(),
                "花".to_string(),
                "夜".to_string(),
                "镜".to_string(),
            ],
            avoided_patterns: vec!["直白议论".to_string(), "冗长描写".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 24,
            clause_complexity: "moderate".to_string(),
            rhythm_pattern: "纤细流畅，余韵悠长".to_string(),
            preferred_structures: vec![
                "意象并置".to_string(),
                "省略主语".to_string(),
                "季节前置".to_string(),
            ],
            opening_variety: "varied".to_string(),
            punctuation_style: "柔和，句尾余韵".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.11,
            preferred_devices: vec!["通感".to_string(), "象征".to_string(), "暗示".to_string()],
            imagery_preference: vec![
                "季节意象".to_string(),
                "色彩意象".to_string(),
                "身体意象".to_string(),
            ],
            parallelism_frequency: "moderate".to_string(),
            irony_usage: "subtle".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "close_third".to_string(),
            narrative_distance: "close".to_string(),
            interior_monologue_ratio: 0.35,
            omniscience_level: 0.2,
            temporal_handling: "flashback".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "restrained".to_string(),
            emotion_word_density: 0.04,
            dominant_mood: "物哀幽玄".to_string(),
            emotional_arc_pattern: "gradual".to_string(),
            humor_style: "none".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.2,
            dialogue_length: "terse".to_string(),
            subtext_ratio: 0.8,
            signature_patterns: vec![
                "沉默间隙".to_string(),
                "含蓄试探".to_string(),
                "未尽之言".to_string(),
            ],
            tag_style: "minimal".to_string(),
        },
    }
}

/// 三岛由纪夫风格
/// 特征：暴烈美学、古典华丽、肌肉与死亡、仪式感
pub fn mishima_yukio() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "三岛由纪夫".to_string(),
            author: Some("三岛由纪夫".to_string()),
            description: "日本战后文学异端，以暴烈华丽的语言追求美与死亡的极致融合，古典与现代交织"
                .to_string(),
            genre_association: Some("后现代/美学小说".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "high".to_string(),
            abstraction: "balanced".to_string(),
            temporal_quality: "mixed".to_string(),
            preferred_categories: vec![
                "古典词汇".to_string(),
                "身体词汇".to_string(),
                "色彩词汇".to_string(),
                "军事术语".to_string(),
            ],
            signature_words: vec![
                "太阳".to_string(),
                "肌肉".to_string(),
                "血".to_string(),
                "金阁".to_string(),
            ],
            avoided_patterns: vec!["平淡口语".to_string(), "日常琐事".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 38,
            clause_complexity: "complex".to_string(),
            rhythm_pattern: "激昂与静谧强烈对比".to_string(),
            preferred_structures: vec![
                "长句铺排".to_string(),
                "仪式感描写".to_string(),
                "对比并列".to_string(),
            ],
            opening_variety: "varied".to_string(),
            punctuation_style: "繁复精致，句号与感叹号交替".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.13,
            preferred_devices: vec![
                "比喻".to_string(),
                "象征".to_string(),
                "夸张".to_string(),
                "对偶".to_string(),
            ],
            imagery_preference: vec![
                "身体意象".to_string(),
                "太阳意象".to_string(),
                "死亡意象".to_string(),
            ],
            parallelism_frequency: "frequent".to_string(),
            irony_usage: "subtle".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "close_third".to_string(),
            narrative_distance: "intimate".to_string(),
            interior_monologue_ratio: 0.45,
            omniscience_level: 0.3,
            temporal_handling: "nonlinear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "expressive".to_string(),
            emotion_word_density: 0.08,
            dominant_mood: "暴烈唯美".to_string(),
            emotional_arc_pattern: "sudden".to_string(),
            humor_style: "none".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.2,
            dialogue_length: "moderate".to_string(),
            subtext_ratio: 0.6,
            signature_patterns: vec![
                "宣言式".to_string(),
                "古典敬语".to_string(),
                "激烈独白".to_string(),
            ],
            tag_style: "minimal".to_string(),
        },
    }
}

/// 太宰治风格
/// 特征：斜阳、颓废、自毁、软弱与讨好
pub fn dazai_osamu() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "太宰治".to_string(),
            author: Some("太宰治".to_string()),
            description: "无赖派代表，以自毁式的坦诚写人类的软弱与羞耻，语气讨好又绝望".to_string(),
            genre_association: Some("无赖派/私小说".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "medium".to_string(),
            abstraction: "abstract".to_string(),
            temporal_quality: "modern".to_string(),
            preferred_categories: vec![
                "自贬词汇".to_string(),
                "死亡意象".to_string(),
                "酒类词汇".to_string(),
                "女性称谓".to_string(),
            ],
            signature_words: vec![
                "羞耻".to_string(),
                "失败".to_string(),
                "酒".to_string(),
                "女人".to_string(),
            ],
            avoided_patterns: vec!["自信表达".to_string(), "成功叙事".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 30,
            clause_complexity: "moderate".to_string(),
            rhythm_pattern: "絮叨自白，断续彷徨".to_string(),
            preferred_structures: vec![
                "独白".to_string(),
                "自贬句式".to_string(),
                "反复道歉".to_string(),
            ],
            opening_variety: "repetitive".to_string(),
            punctuation_style: "省略号、破折号频繁".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.06,
            preferred_devices: vec!["自嘲".to_string(), "反讽".to_string()],
            imagery_preference: vec![
                "黑暗意象".to_string(),
                "堕落意象".to_string(),
                "女性意象".to_string(),
            ],
            parallelism_frequency: "rare".to_string(),
            irony_usage: "overt".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "first_person".to_string(),
            narrative_distance: "intimate".to_string(),
            interior_monologue_ratio: 0.65,
            omniscience_level: 0.0,
            temporal_handling: "flashback".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "expressive".to_string(),
            emotion_word_density: 0.09,
            dominant_mood: "颓废自毁".to_string(),
            emotional_arc_pattern: "cyclical".to_string(),
            humor_style: "dark".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.25,
            dialogue_length: "moderate".to_string(),
            subtext_ratio: 0.5,
            signature_patterns: vec![
                "讨好语气".to_string(),
                "自我贬低".to_string(),
                "玩笑掩饰".to_string(),
            ],
            tag_style: "minimal".to_string(),
        },
    }
}

/// 夏目漱石风格
/// 特征：余裕派、知识分子、幽默、心理深潜
pub fn natsume_soseki() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "夏目漱石".to_string(),
            author: Some("夏目漱石".to_string()),
            description: "日本近代文学巨擘，以知识分子的视角剖析现代人的孤独与利己，余裕派美学"
                .to_string(),
            genre_association: Some("近代文学/知识小说".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "high".to_string(),
            abstraction: "abstract".to_string(),
            temporal_quality: "modern".to_string(),
            preferred_categories: vec![
                "学术术语".to_string(),
                "自然意象".to_string(),
                "心理词汇".to_string(),
                "汉文调".to_string(),
            ],
            signature_words: vec![
                "孤独".to_string(),
                "余裕".to_string(),
                "月亮".to_string(),
                "猫".to_string(),
            ],
            avoided_patterns: vec!["通俗口语".to_string(), " melodramatic 表达".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 42,
            clause_complexity: "complex".to_string(),
            rhythm_pattern: "从容不迫，汉文调余韵".to_string(),
            preferred_structures: vec![
                "长句议论".to_string(),
                "心理分析".to_string(),
                "迂回表达".to_string(),
            ],
            opening_variety: "varied".to_string(),
            punctuation_style: "传统与现代融合，分号破折号".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.07,
            preferred_devices: vec!["比喻".to_string(), "反讽".to_string(), "象征".to_string()],
            imagery_preference: vec![
                "自然意象".to_string(),
                "知识意象".to_string(),
                "孤独意象".to_string(),
            ],
            parallelism_frequency: "moderate".to_string(),
            irony_usage: "overt".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "first_person".to_string(),
            narrative_distance: "close".to_string(),
            interior_monologue_ratio: 0.5,
            omniscience_level: 0.0,
            temporal_handling: "linear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "restrained".to_string(),
            emotion_word_density: 0.04,
            dominant_mood: "孤独余裕".to_string(),
            emotional_arc_pattern: "gradual".to_string(),
            humor_style: "witty".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.25,
            dialogue_length: "moderate".to_string(),
            subtext_ratio: 0.7,
            signature_patterns: vec![
                "知识分子腔".to_string(),
                "迂回试探".to_string(),
                "自嘲式".to_string(),
            ],
            tag_style: "action_beats".to_string(),
        },
    }
}

/// 芥川龙之介风格
/// 特征：历史题材、冷峻怀疑、精致短篇、人性黑暗
pub fn akutagawa_ryunosuke() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "芥川龙之介".to_string(),
            author: Some("芥川龙之介".to_string()),
            description: "短篇小说鬼才，以冷峻精致的笔法重写历史题材，怀疑主义，人性黑暗面"
                .to_string(),
            genre_association: Some("历史小说/短篇".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "high".to_string(),
            abstraction: "abstract".to_string(),
            temporal_quality: "archaic".to_string(),
            preferred_categories: vec![
                "历史用语".to_string(),
                "佛教术语".to_string(),
                "古典词汇".to_string(),
                "病态词汇".to_string(),
            ],
            signature_words: vec![
                "罗生门".to_string(),
                "疑惑".to_string(),
                "利己".to_string(),
                "地狱".to_string(),
            ],
            avoided_patterns: vec!["温情脉脉".to_string(), "道德说教".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 30,
            clause_complexity: "moderate".to_string(),
            rhythm_pattern: "冷峻精致，如刀削木雕".to_string(),
            preferred_structures: vec![
                "史传笔法".to_string(),
                "多角度叙述".to_string(),
                "悬念结尾".to_string(),
            ],
            opening_variety: "varied".to_string(),
            punctuation_style: "简洁精确，句号有力".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.06,
            preferred_devices: vec!["反讽".to_string(), "象征".to_string(), "暗示".to_string()],
            imagery_preference: vec![
                "历史意象".to_string(),
                "黑暗意象".to_string(),
                "宗教意象".to_string(),
            ],
            parallelism_frequency: "moderate".to_string(),
            irony_usage: "overt".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "multiple".to_string(),
            narrative_distance: "distant".to_string(),
            interior_monologue_ratio: 0.15,
            omniscience_level: 0.6,
            temporal_handling: "flashback".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "restrained".to_string(),
            emotion_word_density: 0.03,
            dominant_mood: "冷峻怀疑".to_string(),
            emotional_arc_pattern: "sudden".to_string(),
            humor_style: "dark".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.2,
            dialogue_length: "terse".to_string(),
            subtext_ratio: 0.8,
            signature_patterns: vec![
                "古典白话".to_string(),
                "冷嘲".to_string(),
                "沉默".to_string(),
            ],
            tag_style: "minimal".to_string(),
        },
    }
}

/// 东野圭吾风格
/// 特征：社会派推理、冷静、反转、日常恐怖
pub fn higashino_keigo() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "东野圭吾".to_string(),
            author: Some("东野圭吾".to_string()),
            description: "日本推理天王，以冷静克制的笔法写社会派推理，反转精妙，人性剖析深刻"
                .to_string(),
            genre_association: Some("社会派推理".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "medium".to_string(),
            abstraction: "concrete".to_string(),
            temporal_quality: "modern".to_string(),
            preferred_categories: vec![
                "日常词汇".to_string(),
                "科技词汇".to_string(),
                "法律术语".to_string(),
                "心理术语".to_string(),
            ],
            signature_words: vec![
                "真相".to_string(),
                "动机".to_string(),
                "秘密".to_string(),
                "绝望".to_string(),
            ],
            avoided_patterns: vec!["华丽辞藻".to_string(), "过度抒情".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 22,
            clause_complexity: "moderate".to_string(),
            rhythm_pattern: "冷静推进，信息密集".to_string(),
            preferred_structures: vec![
                "调查推进".to_string(),
                "多线并行".to_string(),
                "时间跳跃".to_string(),
            ],
            opening_variety: "moderate".to_string(),
            punctuation_style: "清晰冷静，句号为主".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.02,
            preferred_devices: vec!["伏笔".to_string(), "暗示".to_string()],
            imagery_preference: vec!["日常意象".to_string(), "科技意象".to_string()],
            parallelism_frequency: "rare".to_string(),
            irony_usage: "subtle".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "multiple".to_string(),
            narrative_distance: "close".to_string(),
            interior_monologue_ratio: 0.2,
            omniscience_level: 0.5,
            temporal_handling: "nonlinear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "restrained".to_string(),
            emotion_word_density: 0.03,
            dominant_mood: "冷静绝望".to_string(),
            emotional_arc_pattern: "sudden".to_string(),
            humor_style: "none".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.35,
            dialogue_length: "moderate".to_string(),
            subtext_ratio: 0.5,
            signature_patterns: vec![
                "审讯式".to_string(),
                "试探性".to_string(),
                "关键线索".to_string(),
            ],
            tag_style: "said_only".to_string(),
        },
    }
}

// ==================== 欧美文学（14种）====================

/// 陀思妥耶夫斯基风格
/// 特征：心理深渊、癫狂、长独白、罪与罚
pub fn dostoevsky() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "陀思妥耶夫斯基".to_string(),
            author: Some("陀思妥耶夫斯基".to_string()),
            description: "俄罗斯文学深渊，以癫狂的长篇独白探索人性的罪恶与救赎，心理描写极致"
                .to_string(),
            genre_association: Some("心理小说/哲学".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "high".to_string(),
            abstraction: "abstract".to_string(),
            temporal_quality: "modern".to_string(),
            preferred_categories: vec![
                "宗教术语".to_string(),
                "哲学词汇".to_string(),
                "心理术语".to_string(),
                "俄语语气".to_string(),
            ],
            signature_words: vec![
                "上帝".to_string(),
                "罪恶".to_string(),
                "疯狂".to_string(),
                "苦难".to_string(),
            ],
            avoided_patterns: vec!["简洁克制".to_string(), "平淡叙述".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 55,
            clause_complexity: "complex".to_string(),
            rhythm_pattern: "汹涌澎湃，一气呵成".to_string(),
            preferred_structures: vec![
                "长篇独白".to_string(),
                "对话辩论".to_string(),
                "意识流".to_string(),
            ],
            opening_variety: "varied".to_string(),
            punctuation_style: "感叹号、破折号、分号密集".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.09,
            preferred_devices: vec!["反问".to_string(), "夸张".to_string(), "对比".to_string()],
            imagery_preference: vec![
                "宗教意象".to_string(),
                "黑暗意象".to_string(),
                "城市意象".to_string(),
            ],
            parallelism_frequency: "frequent".to_string(),
            irony_usage: "subtle".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "close_third".to_string(),
            narrative_distance: "intimate".to_string(),
            interior_monologue_ratio: 0.6,
            omniscience_level: 0.4,
            temporal_handling: "nonlinear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "expressive".to_string(),
            emotion_word_density: 0.1,
            dominant_mood: "狂热绝望".to_string(),
            emotional_arc_pattern: "sudden".to_string(),
            humor_style: "dark".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.4,
            dialogue_length: "verbose".to_string(),
            subtext_ratio: 0.3,
            signature_patterns: vec![
                "长篇辩论".to_string(),
                "癫狂独白".to_string(),
                "哲学质问".to_string(),
            ],
            tag_style: "varied_tags".to_string(),
        },
    }
}

/// 托尔斯泰风格
/// 特征：史诗全景、道德、朴素、历史与家庭
pub fn tolstoy() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "托尔斯泰".to_string(),
            author: Some("列夫·托尔斯泰".to_string()),
            description: "俄国文学泰斗，以史诗般的全景视角写历史与家庭，道德追问，朴素而深邃"
                .to_string(),
            genre_association: Some("史诗/现实主义".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "medium".to_string(),
            abstraction: "concrete".to_string(),
            temporal_quality: "mixed".to_string(),
            preferred_categories: vec![
                "农事词汇".to_string(),
                "军事术语".to_string(),
                "宗教词汇".to_string(),
                "家庭用语".to_string(),
            ],
            signature_words: vec![
                "灵魂".to_string(),
                "土地".to_string(),
                "战争".to_string(),
                "和平".to_string(),
            ],
            avoided_patterns: vec!["华丽修饰".to_string(), "过度象征".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 40,
            clause_complexity: "complex".to_string(),
            rhythm_pattern: "雄浑开阔，从容不迫".to_string(),
            preferred_structures: vec![
                "全景描写".to_string(),
                "内心分析".to_string(),
                "历史议论".to_string(),
            ],
            opening_variety: "varied".to_string(),
            punctuation_style: "清晰从容，长句为主".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.05,
            preferred_devices: vec!["对比".to_string(), "象征".to_string()],
            imagery_preference: vec![
                "自然意象".to_string(),
                "战争意象".to_string(),
                "家庭意象".to_string(),
            ],
            parallelism_frequency: "moderate".to_string(),
            irony_usage: "subtle".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "omniscient".to_string(),
            narrative_distance: "moderate".to_string(),
            interior_monologue_ratio: 0.3,
            omniscience_level: 0.95,
            temporal_handling: "nonlinear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "balanced".to_string(),
            emotion_word_density: 0.05,
            dominant_mood: "悲悯庄严".to_string(),
            emotional_arc_pattern: "gradual".to_string(),
            humor_style: "none".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.3,
            dialogue_length: "moderate".to_string(),
            subtext_ratio: 0.4,
            signature_patterns: vec![
                "家庭辩论".to_string(),
                "内心独白式对话".to_string(),
                "俄国贵族腔".to_string(),
            ],
            tag_style: "varied_tags".to_string(),
        },
    }
}

/// 卡夫卡风格
/// 特征：荒诞、异化、冷静恐怖、官僚迷宫
pub fn kafka() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "卡夫卡".to_string(),
            author: Some("弗朗茨·卡夫卡".to_string()),
            description: "现代主义先驱，以冷静理性的笔法写荒诞与异化，官僚迷宫，存在焦虑"
                .to_string(),
            genre_association: Some("现代主义/荒诞".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "medium".to_string(),
            abstraction: "abstract".to_string(),
            temporal_quality: "modern".to_string(),
            preferred_categories: vec![
                "法律术语".to_string(),
                "官僚用语".to_string(),
                "建筑词汇".to_string(),
                "家庭称谓".to_string(),
            ],
            signature_words: vec![
                "审判".to_string(),
                "变形".to_string(),
                "门".to_string(),
                "城堡".to_string(),
            ],
            avoided_patterns: vec!["情感词汇".to_string(), "抒情表达".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 35,
            clause_complexity: "complex".to_string(),
            rhythm_pattern: "冷静冗长，近乎报告".to_string(),
            preferred_structures: vec![
                "长句铺排".to_string(),
                "条件从句".to_string(),
                "间接引语".to_string(),
            ],
            opening_variety: "moderate".to_string(),
            punctuation_style: "冷静精确，逗号连接".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.03,
            preferred_devices: vec!["象征".to_string(), "寓言".to_string()],
            imagery_preference: vec![
                "建筑意象".to_string(),
                "迷宫意象".to_string(),
                "变形意象".to_string(),
            ],
            parallelism_frequency: "moderate".to_string(),
            irony_usage: "subtle".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "close_third".to_string(),
            narrative_distance: "close".to_string(),
            interior_monologue_ratio: 0.4,
            omniscience_level: 0.1,
            temporal_handling: "nonlinear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "restrained".to_string(),
            emotion_word_density: 0.02,
            dominant_mood: "焦虑荒诞".to_string(),
            emotional_arc_pattern: "static".to_string(),
            humor_style: "dark".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.3,
            dialogue_length: "moderate".to_string(),
            subtext_ratio: 0.6,
            signature_patterns: vec![
                "官僚式".to_string(),
                "荒诞问答".to_string(),
                "间接引语".to_string(),
            ],
            tag_style: "said_only".to_string(),
        },
    }
}

/// 福克纳风格
/// 特征：美国南方、多角度、繁复长句、时间跳跃
pub fn faulkner() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "福克纳".to_string(),
            author: Some("威廉·福克纳".to_string()),
            description: "美国南方文学代表，以繁复长句和多角度叙事写家族史诗，时间跳跃，意识流"
                .to_string(),
            genre_association: Some("南方哥特/现代主义".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "high".to_string(),
            abstraction: "abstract".to_string(),
            temporal_quality: "mixed".to_string(),
            preferred_categories: vec![
                "南方方言".to_string(),
                "家族词汇".to_string(),
                "自然词汇".to_string(),
                "宗教用语".to_string(),
            ],
            signature_words: vec![
                "时间".to_string(),
                "家族".to_string(),
                "土地".to_string(),
                "荣誉".to_string(),
            ],
            avoided_patterns: vec!["简洁克制".to_string(), "线性叙事".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 65,
            clause_complexity: "complex".to_string(),
            rhythm_pattern: "奔腾不息，意识流淌".to_string(),
            preferred_structures: vec![
                "长句嵌套".to_string(),
                "意识流".to_string(),
                "时间跳跃".to_string(),
            ],
            opening_variety: "varied".to_string(),
            punctuation_style: "括号、分号、破折号繁复".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.1,
            preferred_devices: vec![
                "象征".to_string(),
                "意识流".to_string(),
                "多角度".to_string(),
            ],
            imagery_preference: vec![
                "南方意象".to_string(),
                "家族意象".to_string(),
                "自然意象".to_string(),
            ],
            parallelism_frequency: "frequent".to_string(),
            irony_usage: "subtle".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "multiple".to_string(),
            narrative_distance: "intimate".to_string(),
            interior_monologue_ratio: 0.5,
            omniscience_level: 0.6,
            temporal_handling: "nonlinear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "expressive".to_string(),
            emotion_word_density: 0.08,
            dominant_mood: "悲怆狂乱".to_string(),
            emotional_arc_pattern: "sudden".to_string(),
            humor_style: "dark".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.25,
            dialogue_length: "verbose".to_string(),
            subtext_ratio: 0.5,
            signature_patterns: vec![
                "南方口音".to_string(),
                "家族 gossip".to_string(),
                "意识流对话".to_string(),
            ],
            tag_style: "varied_tags".to_string(),
        },
    }
}

/// 菲茨杰拉德风格
/// 特征：爵士时代、华丽忧郁、美国梦、精致感伤
pub fn fitzgerald() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "菲茨杰拉德".to_string(),
            author: Some("菲茨杰拉德".to_string()),
            description: "爵士时代代言人，以华丽精致的文风写美国梦的破灭，忧郁感伤，金句频出"
                .to_string(),
            genre_association: Some("爵士时代/现代主义".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "high".to_string(),
            abstraction: "balanced".to_string(),
            temporal_quality: "modern".to_string(),
            preferred_categories: vec![
                "奢华词汇".to_string(),
                "色彩词汇".to_string(),
                "音乐术语".to_string(),
                "时代用语".to_string(),
            ],
            signature_words: vec![
                "绿灯".to_string(),
                "梦想".to_string(),
                "奢华".to_string(),
                "失落".to_string(),
            ],
            avoided_patterns: vec!["粗俗口语".to_string(), "直白议论".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 32,
            clause_complexity: "moderate".to_string(),
            rhythm_pattern: "华丽流畅，诗化散文".to_string(),
            preferred_structures: vec![
                "意象铺陈".to_string(),
                "对比".to_string(),
                "象征".to_string(),
            ],
            opening_variety: "varied".to_string(),
            punctuation_style: "精致，善用分号与破折号".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.12,
            preferred_devices: vec!["比喻".to_string(), "象征".to_string(), "对比".to_string()],
            imagery_preference: vec![
                "奢华意象".to_string(),
                "色彩意象".to_string(),
                "梦想意象".to_string(),
            ],
            parallelism_frequency: "moderate".to_string(),
            irony_usage: "subtle".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "first_person".to_string(),
            narrative_distance: "close".to_string(),
            interior_monologue_ratio: 0.35,
            omniscience_level: 0.1,
            temporal_handling: "flashback".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "expressive".to_string(),
            emotion_word_density: 0.07,
            dominant_mood: "华丽忧伤".to_string(),
            emotional_arc_pattern: "gradual".to_string(),
            humor_style: "dry".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.35,
            dialogue_length: "moderate".to_string(),
            subtext_ratio: 0.6,
            signature_patterns: vec![
                "机智交谈".to_string(),
                "社交寒暄".to_string(),
                "酒后真言".to_string(),
            ],
            tag_style: "action_beats".to_string(),
        },
    }
}

/// 博尔赫斯风格
/// 特征：迷宫、智性、浓缩、时间循环、图书馆
pub fn borges() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "博尔赫斯".to_string(),
            author: Some("豪尔赫·路易斯·博尔赫斯".to_string()),
            description: "阿根廷文学大师，以智性迷宫和浓缩的笔法探索时间、无限与镜像，百科全书式"
                .to_string(),
            genre_association: Some("后现代/幻想".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "high".to_string(),
            abstraction: "abstract".to_string(),
            temporal_quality: "mixed".to_string(),
            preferred_categories: vec![
                "哲学术语".to_string(),
                "神学词汇".to_string(),
                "东方典故".to_string(),
                "数学术语".to_string(),
            ],
            signature_words: vec![
                "迷宫".to_string(),
                "镜子".to_string(),
                "无限".to_string(),
                "图书馆".to_string(),
            ],
            avoided_patterns: vec!["冗长描写".to_string(), "情感铺陈".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 28,
            clause_complexity: "moderate".to_string(),
            rhythm_pattern: "浓缩精炼，如寓言".to_string(),
            preferred_structures: vec![
                "浓缩叙述".to_string(),
                "伪学术".to_string(),
                "循环结构".to_string(),
            ],
            opening_variety: "varied".to_string(),
            punctuation_style: "简洁精确，句号有力".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.08,
            preferred_devices: vec!["寓言".to_string(), "悖论".to_string(), "象征".to_string()],
            imagery_preference: vec![
                "迷宫意象".to_string(),
                "镜子意象".to_string(),
                "时间意象".to_string(),
            ],
            parallelism_frequency: "moderate".to_string(),
            irony_usage: "subtle".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "first_person".to_string(),
            narrative_distance: "distant".to_string(),
            interior_monologue_ratio: 0.3,
            omniscience_level: 0.1,
            temporal_handling: "nonlinear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "restrained".to_string(),
            emotion_word_density: 0.02,
            dominant_mood: "智性孤寂".to_string(),
            emotional_arc_pattern: "static".to_string(),
            humor_style: "dry".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.1,
            dialogue_length: "terse".to_string(),
            subtext_ratio: 0.9,
            signature_patterns: vec![
                "哲学问答".to_string(),
                "箴言式".to_string(),
                "间接引语".to_string(),
            ],
            tag_style: "minimal".to_string(),
        },
    }
}

/// 科塔萨尔风格
/// 特征：日常变形、奇幻跳脱、游戏规则、读者参与
pub fn cortazar() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "科塔萨尔".to_string(),
            author: Some("胡里奥·科塔萨尔".to_string()),
            description: "拉美文学爆炸代表，以日常变形和跳脱结构打破叙事常规，游戏感，读者参与"
                .to_string(),
            genre_association: Some("后现代/奇幻".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "medium".to_string(),
            abstraction: "balanced".to_string(),
            temporal_quality: "modern".to_string(),
            preferred_categories: vec![
                "日常词汇".to_string(),
                "游戏术语".to_string(),
                "音乐术语".to_string(),
                "动物词汇".to_string(),
            ],
            signature_words: vec![
                "门".to_string(),
                "跳房子".to_string(),
                "兔子".to_string(),
                "地铁".to_string(),
            ],
            avoided_patterns: vec!["宏大叙事".to_string(), "道德说教".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 26,
            clause_complexity: "moderate".to_string(),
            rhythm_pattern: "跳跃灵动，如爵士即兴".to_string(),
            preferred_structures: vec![
                "日常变形".to_string(),
                "分支叙事".to_string(),
                "读者指令".to_string(),
            ],
            opening_variety: "varied".to_string(),
            punctuation_style: "活泼，善用逗号与破折号".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.07,
            preferred_devices: vec!["超现实".to_string(), "游戏".to_string(), "象征".to_string()],
            imagery_preference: vec![
                "都市意象".to_string(),
                "动物意象".to_string(),
                "游戏意象".to_string(),
            ],
            parallelism_frequency: "moderate".to_string(),
            irony_usage: "overt".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "multiple".to_string(),
            narrative_distance: "close".to_string(),
            interior_monologue_ratio: 0.25,
            omniscience_level: 0.4,
            temporal_handling: "nonlinear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "balanced".to_string(),
            emotion_word_density: 0.04,
            dominant_mood: " playful 疏离".to_string(),
            emotional_arc_pattern: "cyclical".to_string(),
            humor_style: "witty".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.3,
            dialogue_length: "moderate".to_string(),
            subtext_ratio: 0.5,
            signature_patterns: vec![
                "日常怪谈".to_string(),
                "游戏式".to_string(),
                "读者对话".to_string(),
            ],
            tag_style: "minimal".to_string(),
        },
    }
}

/// 爱伦·坡风格
/// 特征：哥特恐怖、韵律、死亡、心理分析
pub fn poe() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "爱伦·坡".to_string(),
            author: Some("埃德加·爱伦·坡".to_string()),
            description: "哥特文学之父，以精密计算的语言营造恐怖氛围，死亡迷恋，心理分析先驱"
                .to_string(),
            genre_association: Some("哥特/恐怖".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "high".to_string(),
            abstraction: "abstract".to_string(),
            temporal_quality: "archaic".to_string(),
            preferred_categories: vec![
                "死亡词汇".to_string(),
                "建筑术语".to_string(),
                "心理术语".to_string(),
                "色彩词汇".to_string(),
            ],
            signature_words: vec![
                "死亡".to_string(),
                "乌鸦".to_string(),
                "心脏".to_string(),
                "坟墓".to_string(),
            ],
            avoided_patterns: vec!["日常口语".to_string(), "幽默轻松".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 30,
            clause_complexity: "moderate".to_string(),
            rhythm_pattern: "韵律感强，如诗歌".to_string(),
            preferred_structures: vec!["重复".to_string(), "递进".to_string(), "倒叙".to_string()],
            opening_variety: "moderate".to_string(),
            punctuation_style: "感叹号、破折号、分号".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.1,
            preferred_devices: vec!["象征".to_string(), "重复".to_string(), "夸张".to_string()],
            imagery_preference: vec![
                "黑暗意象".to_string(),
                "死亡意象".to_string(),
                "建筑意象".to_string(),
            ],
            parallelism_frequency: "frequent".to_string(),
            irony_usage: "none".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "first_person".to_string(),
            narrative_distance: "intimate".to_string(),
            interior_monologue_ratio: 0.55,
            omniscience_level: 0.0,
            temporal_handling: "flashback".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "expressive".to_string(),
            emotion_word_density: 0.1,
            dominant_mood: "恐怖阴郁".to_string(),
            emotional_arc_pattern: "sudden".to_string(),
            humor_style: "none".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.15,
            dialogue_length: "terse".to_string(),
            subtext_ratio: 0.4,
            signature_patterns: vec![
                "独白".to_string(),
                "疯狂低语".to_string(),
                "死亡宣告".to_string(),
            ],
            tag_style: "minimal".to_string(),
        },
    }
}

/// 洛夫克拉夫特风格
/// 特征：宇宙恐怖、不可名状、冗长、科学冷静
pub fn lovecraft() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "洛夫克拉夫特".to_string(),
            author: Some("H.P.洛夫克拉夫特".to_string()),
            description: "克苏鲁神话创始人，以科学冷静的长篇描写构建宇宙恐怖，不可名状，细节密集"
                .to_string(),
            genre_association: Some("宇宙恐怖/科幻".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "high".to_string(),
            abstraction: "abstract".to_string(),
            temporal_quality: "modern".to_string(),
            preferred_categories: vec![
                "古词汇".to_string(),
                "科学术语".to_string(),
                "建筑术语".to_string(),
                "神话词汇".to_string(),
            ],
            signature_words: vec![
                "不可名状".to_string(),
                "疯狂".to_string(),
                "远古".to_string(),
                "深渊".to_string(),
            ],
            avoided_patterns: vec![
                "日常口语".to_string(),
                "幽默".to_string(),
                "情感直白".to_string(),
            ],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 48,
            clause_complexity: "complex".to_string(),
            rhythm_pattern: "冗长密集，层层叠加".to_string(),
            preferred_structures: vec![
                "长篇描写".to_string(),
                "条件从句".to_string(),
                "否定式".to_string(),
            ],
            opening_variety: "moderate".to_string(),
            punctuation_style: "逗号密集，长句连绵".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.08,
            preferred_devices: vec!["暗示".to_string(), "夸张".to_string(), "象征".to_string()],
            imagery_preference: vec![
                "宇宙意象".to_string(),
                "建筑意象".to_string(),
                "深渊意象".to_string(),
            ],
            parallelism_frequency: "moderate".to_string(),
            irony_usage: "none".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "first_person".to_string(),
            narrative_distance: "close".to_string(),
            interior_monologue_ratio: 0.4,
            omniscience_level: 0.0,
            temporal_handling: "flashback".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "restrained".to_string(),
            emotion_word_density: 0.03,
            dominant_mood: "宇宙恐怖".to_string(),
            emotional_arc_pattern: "gradual".to_string(),
            humor_style: "none".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.1,
            dialogue_length: "terse".to_string(),
            subtext_ratio: 0.8,
            signature_patterns: vec![
                "警告".to_string(),
                "日记体".to_string(),
                "科学记录".to_string(),
            ],
            tag_style: "minimal".to_string(),
        },
    }
}

/// 简·奥斯汀风格
/// 特征：讽刺、礼仪、机智、婚姻市场
pub fn austen() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "简·奥斯汀".to_string(),
            author: Some("简·奥斯汀".to_string()),
            description: "英国古典讽刺大师，以机智优雅的笔法剖析婚姻与阶级，讽刺含蓄，对话精彩"
                .to_string(),
            genre_association: Some("社会风俗/浪漫".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "high".to_string(),
            abstraction: "balanced".to_string(),
            temporal_quality: "archaic".to_string(),
            preferred_categories: vec![
                "社交用语".to_string(),
                "财产词汇".to_string(),
                "礼仪用语".to_string(),
                "情感委婉语".to_string(),
            ],
            signature_words: vec![
                "婚姻".to_string(),
                "财产".to_string(),
                "体面".to_string(),
                "偏见".to_string(),
            ],
            avoided_patterns: vec!["粗俗口语".to_string(), "直白情感".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 34,
            clause_complexity: "complex".to_string(),
            rhythm_pattern: "优雅从容，如舞步".to_string(),
            preferred_structures: vec![
                "自由间接引语".to_string(),
                "反讽对比".to_string(),
                "礼貌迂回".to_string(),
            ],
            opening_variety: "varied".to_string(),
            punctuation_style: "精致，善用逗号分号".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.06,
            preferred_devices: vec!["反讽".to_string(), "对比".to_string(), "象征".to_string()],
            imagery_preference: vec![
                "社交意象".to_string(),
                "乡村意象".to_string(),
                "财产意象".to_string(),
            ],
            parallelism_frequency: "moderate".to_string(),
            irony_usage: "overt".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "close_third".to_string(),
            narrative_distance: "close".to_string(),
            interior_monologue_ratio: 0.35,
            omniscience_level: 0.5,
            temporal_handling: "linear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "restrained".to_string(),
            emotion_word_density: 0.04,
            dominant_mood: "机智优雅".to_string(),
            emotional_arc_pattern: "gradual".to_string(),
            humor_style: "witty".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.4,
            dialogue_length: "moderate".to_string(),
            subtext_ratio: 0.7,
            signature_patterns: vec![
                "机智交锋".to_string(),
                "礼貌刺探".to_string(),
                "间接表白".to_string(),
            ],
            tag_style: "action_beats".to_string(),
        },
    }
}

/// 狄更斯风格
/// 特征：社会批判、人物类型化、温情、连载节奏
pub fn dickens() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "狄更斯".to_string(),
            author: Some("查尔斯·狄更斯".to_string()),
            description: "维多利亚时代小说巨匠，以夸张生动的人物和社会批判写伦敦众生相，温情脉脉"
                .to_string(),
            genre_association: Some("社会批判/连载".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "medium".to_string(),
            abstraction: "concrete".to_string(),
            temporal_quality: "archaic".to_string(),
            preferred_categories: vec![
                "伦敦方言".to_string(),
                "法律术语".to_string(),
                "贫困词汇".to_string(),
                "儿童用语".to_string(),
            ],
            signature_words: vec![
                "伦敦".to_string(),
                "雾".to_string(),
                "孤儿".to_string(),
                "圣诞".to_string(),
            ],
            avoided_patterns: vec!["粗俗直描".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 28,
            clause_complexity: "moderate".to_string(),
            rhythm_pattern: "生动活泼，戏剧性".to_string(),
            preferred_structures: vec![
                "类型化描写".to_string(),
                "悬念结尾".to_string(),
                "温情转折".to_string(),
            ],
            opening_variety: "varied".to_string(),
            punctuation_style: "戏剧化，感叹号频繁".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.07,
            preferred_devices: vec!["夸张".to_string(), "拟人".to_string(), "象征".to_string()],
            imagery_preference: vec![
                "城市意象".to_string(),
                "贫困意象".to_string(),
                "自然意象".to_string(),
            ],
            parallelism_frequency: "moderate".to_string(),
            irony_usage: "overt".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "omniscient".to_string(),
            narrative_distance: "moderate".to_string(),
            interior_monologue_ratio: 0.15,
            omniscience_level: 0.9,
            temporal_handling: "linear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "expressive".to_string(),
            emotion_word_density: 0.07,
            dominant_mood: "温情悲悯".to_string(),
            emotional_arc_pattern: "gradual".to_string(),
            humor_style: "witty".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.4,
            dialogue_length: "verbose".to_string(),
            subtext_ratio: 0.3,
            signature_patterns: vec![
                "方言腔调".to_string(),
                "戏剧式".to_string(),
                "温情说教".to_string(),
            ],
            tag_style: "varied_tags".to_string(),
        },
    }
}

/// 福楼拜风格
/// 特征：客观、精雕细琢、包法利式、农民语言
pub fn flaubert() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "福楼拜".to_string(),
            author: Some("居斯塔夫·福楼拜".to_string()),
            description: "法国现实主义巅峰，以极度客观和精雕细琢的笔法写人性欲望，作者隐退"
                .to_string(),
            genre_association: Some("现实主义/自然主义".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "high".to_string(),
            abstraction: "concrete".to_string(),
            temporal_quality: "modern".to_string(),
            preferred_categories: vec![
                "医学词汇".to_string(),
                "农业术语".to_string(),
                "色彩词汇".to_string(),
                "宗教用语".to_string(),
            ],
            signature_words: vec![
                "包法利".to_string(),
                "外省".to_string(),
                "梦想".to_string(),
                "庸俗".to_string(),
            ],
            avoided_patterns: vec![
                "作者评论".to_string(),
                "道德判断".to_string(),
                "情感直白".to_string(),
            ],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 36,
            clause_complexity: "complex".to_string(),
            rhythm_pattern: "精确冷静，如外科手术".to_string(),
            preferred_structures: vec![
                "场景描写".to_string(),
                "自由间接引语".to_string(),
                "细节堆砌".to_string(),
            ],
            opening_variety: "varied".to_string(),
            punctuation_style: "精确冷静，长句为主".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.05,
            preferred_devices: vec!["象征".to_string(), "对比".to_string()],
            imagery_preference: vec![
                "自然意象".to_string(),
                "乡村意象".to_string(),
                "物质意象".to_string(),
            ],
            parallelism_frequency: "moderate".to_string(),
            irony_usage: "subtle".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "close_third".to_string(),
            narrative_distance: "distant".to_string(),
            interior_monologue_ratio: 0.25,
            omniscience_level: 0.4,
            temporal_handling: "linear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "restrained".to_string(),
            emotion_word_density: 0.03,
            dominant_mood: "冷静悲悯".to_string(),
            emotional_arc_pattern: "gradual".to_string(),
            humor_style: "none".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.25,
            dialogue_length: "moderate".to_string(),
            subtext_ratio: 0.5,
            signature_patterns: vec![
                "自由间接引语".to_string(),
                "农民口语".to_string(),
                "社交寒暄".to_string(),
            ],
            tag_style: "action_beats".to_string(),
        },
    }
}

/// 雨果风格
/// 特征：浪漫主义、宏大、人道、史诗
pub fn hugo() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "雨果".to_string(),
            author: Some("维克多·雨果".to_string()),
            description: "法国浪漫主义巨匠，以宏大的叙事和人道主义情怀写历史与社会，激情澎湃"
                .to_string(),
            genre_association: Some("浪漫主义/史诗".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "high".to_string(),
            abstraction: "balanced".to_string(),
            temporal_quality: "archaic".to_string(),
            preferred_categories: vec![
                "历史术语".to_string(),
                "建筑词汇".to_string(),
                "海洋词汇".to_string(),
                "宗教用语".to_string(),
            ],
            signature_words: vec![
                "人民".to_string(),
                "自由".to_string(),
                "苦难".to_string(),
                "光明".to_string(),
            ],
            avoided_patterns: vec!["平淡克制".to_string(), "琐碎日常".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 42,
            clause_complexity: "complex".to_string(),
            rhythm_pattern: "激情澎湃，排山倒海".to_string(),
            preferred_structures: vec![
                "长篇议论".to_string(),
                "全景描写".to_string(),
                "对比排比".to_string(),
            ],
            opening_variety: "varied".to_string(),
            punctuation_style: "感叹号、分号、破折号密集".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.09,
            preferred_devices: vec![
                "比喻".to_string(),
                "排比".to_string(),
                "对比".to_string(),
                "呼告".to_string(),
            ],
            imagery_preference: vec![
                "建筑意象".to_string(),
                "海洋意象".to_string(),
                "人民意象".to_string(),
            ],
            parallelism_frequency: "frequent".to_string(),
            irony_usage: "subtle".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "omniscient".to_string(),
            narrative_distance: "moderate".to_string(),
            interior_monologue_ratio: 0.2,
            omniscience_level: 0.95,
            temporal_handling: "nonlinear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "expressive".to_string(),
            emotion_word_density: 0.08,
            dominant_mood: "激情人道".to_string(),
            emotional_arc_pattern: "gradual".to_string(),
            humor_style: "none".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.25,
            dialogue_length: "verbose".to_string(),
            subtext_ratio: 0.3,
            signature_patterns: vec![
                "宣言式".to_string(),
                "长篇辩论".to_string(),
                "戏剧式".to_string(),
            ],
            tag_style: "varied_tags".to_string(),
        },
    }
}

/// 纳博科夫风格
/// 特征：博学、文字游戏、华丽、不可靠叙事
pub fn nabokov() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "纳博科夫".to_string(),
            author: Some("弗拉基米尔·纳博科夫".to_string()),
            description: "俄裔美国文学大师，以博学和文字游戏构建华丽迷宫，不可靠叙事，语言炫技"
                .to_string(),
            genre_association: Some("后现代/元小说".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "high".to_string(),
            abstraction: "abstract".to_string(),
            temporal_quality: "modern".to_string(),
            preferred_categories: vec![
                "多语言词汇".to_string(),
                "蝴蝶学术".to_string(),
                "象棋术语".to_string(),
                "文学典故".to_string(),
            ],
            signature_words: vec![
                "蝴蝶".to_string(),
                "洛丽塔".to_string(),
                "语言".to_string(),
                "记忆".to_string(),
            ],
            avoided_patterns: vec!["平淡直白".to_string(), "道德说教".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 44,
            clause_complexity: "complex".to_string(),
            rhythm_pattern: "华丽繁复，如蝴蝶振翅".to_string(),
            preferred_structures: vec![
                "长句嵌套".to_string(),
                "文字游戏".to_string(),
                "元叙事".to_string(),
            ],
            opening_variety: "varied".to_string(),
            punctuation_style: "精致繁复，括号注释".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.13,
            preferred_devices: vec![
                "比喻".to_string(),
                "双关".to_string(),
                "典故".to_string(),
                "戏仿".to_string(),
            ],
            imagery_preference: vec![
                "蝴蝶意象".to_string(),
                "童年意象".to_string(),
                "语言意象".to_string(),
            ],
            parallelism_frequency: "moderate".to_string(),
            irony_usage: "overt".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "first_person".to_string(),
            narrative_distance: "intimate".to_string(),
            interior_monologue_ratio: 0.55,
            omniscience_level: 0.0,
            temporal_handling: "nonlinear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "restrained".to_string(),
            emotion_word_density: 0.04,
            dominant_mood: "智性迷狂".to_string(),
            emotional_arc_pattern: "cyclical".to_string(),
            humor_style: "witty".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.2,
            dialogue_length: "moderate".to_string(),
            subtext_ratio: 0.8,
            signature_patterns: vec![
                "多语言夹杂".to_string(),
                "文字游戏".to_string(),
                "不可靠叙述".to_string(),
            ],
            tag_style: "minimal".to_string(),
        },
    }
}

// ==================== 类型文学风格（8种）====================

/// 赛博朋克风格
/// 特征：高科技低生活、霓虹、碎片化、黑客
pub fn cyberpunk() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "赛博朋克".to_string(),
            author: None,
            description: "高科技与低生活的黑暗融合，霓虹灯雨夜，信息过载，身体改造，企业霸权"
                .to_string(),
            genre_association: Some("科幻/赛博朋克".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "high".to_string(),
            abstraction: "concrete".to_string(),
            temporal_quality: "futuristic".to_string(),
            preferred_categories: vec![
                "科技术语".to_string(),
                "日语借词".to_string(),
                "毒品词汇".to_string(),
                "网络用语".to_string(),
            ],
            signature_words: vec![
                "赛博空间".to_string(),
                "神经接口".to_string(),
                "霓虹".to_string(),
                "黑客".to_string(),
            ],
            avoided_patterns: vec!["田园牧歌".to_string(), "温情脉脉".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 20,
            clause_complexity: "moderate".to_string(),
            rhythm_pattern: "快节奏，信息密集".to_string(),
            preferred_structures: vec![
                "碎片化叙事".to_string(),
                "技术说明".to_string(),
                "视角跳跃".to_string(),
            ],
            opening_variety: "varied".to_string(),
            punctuation_style: "极简，短句为主".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.04,
            preferred_devices: vec!["比喻".to_string(), "象征".to_string()],
            imagery_preference: vec![
                "科技意象".to_string(),
                "城市意象".to_string(),
                "身体意象".to_string(),
            ],
            parallelism_frequency: "rare".to_string(),
            irony_usage: "subtle".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "first_person".to_string(),
            narrative_distance: "close".to_string(),
            interior_monologue_ratio: 0.3,
            omniscience_level: 0.0,
            temporal_handling: "nonlinear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "restrained".to_string(),
            emotion_word_density: 0.02,
            dominant_mood: "冷漠疏离".to_string(),
            emotional_arc_pattern: "static".to_string(),
            humor_style: "dark".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.3,
            dialogue_length: "terse".to_string(),
            subtext_ratio: 0.4,
            signature_patterns: vec![
                "黑客黑话".to_string(),
                "街头 slang".to_string(),
                "日语借词".to_string(),
            ],
            tag_style: "minimal".to_string(),
        },
    }
}

/// 蒸汽朋克风格
/// 特征：维多利亚、齿轮、冒险、绅士风度
pub fn steampunk() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "蒸汽朋克".to_string(),
            author: None,
            description: "维多利亚时代与蒸汽科技的浪漫融合，齿轮飞艇，绅士冒险，复古未来"
                .to_string(),
            genre_association: Some("科幻/蒸汽朋克".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "high".to_string(),
            abstraction: "concrete".to_string(),
            temporal_quality: "archaic".to_string(),
            preferred_categories: vec![
                "机械术语".to_string(),
                "维多利亚用语".to_string(),
                "航海词汇".to_string(),
                "绅士用语".to_string(),
            ],
            signature_words: vec![
                "蒸汽".to_string(),
                "齿轮".to_string(),
                "飞艇".to_string(),
                "绅士".to_string(),
            ],
            avoided_patterns: vec!["现代俚语".to_string(), "电子词汇".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 30,
            clause_complexity: "moderate".to_string(),
            rhythm_pattern: "优雅冒险，如老式探险小说".to_string(),
            preferred_structures: vec![
                "场景描写".to_string(),
                "技术说明".to_string(),
                "绅士对话".to_string(),
            ],
            opening_variety: "varied".to_string(),
            punctuation_style: "传统精致，长句为主".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.06,
            preferred_devices: vec!["比喻".to_string(), "夸张".to_string()],
            imagery_preference: vec![
                "机械意象".to_string(),
                "维多利亚意象".to_string(),
                "冒险意象".to_string(),
            ],
            parallelism_frequency: "moderate".to_string(),
            irony_usage: "subtle".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "first_person".to_string(),
            narrative_distance: "close".to_string(),
            interior_monologue_ratio: 0.2,
            omniscience_level: 0.1,
            temporal_handling: "linear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "balanced".to_string(),
            emotion_word_density: 0.04,
            dominant_mood: "浪漫冒险".to_string(),
            emotional_arc_pattern: "gradual".to_string(),
            humor_style: "witty".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.3,
            dialogue_length: "moderate".to_string(),
            subtext_ratio: 0.5,
            signature_patterns: vec![
                "维多利亚腔".to_string(),
                "绅士风度".to_string(),
                "冒险术语".to_string(),
            ],
            tag_style: "varied_tags".to_string(),
        },
    }
}

/// 新怪谈风格
/// 特征：都市奇幻、不可解、日常恐怖、官僚迷宫
pub fn new_weird() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "新怪谈".to_string(),
            author: None,
            description: "当代恐怖美学，以不可解的异常渗透日常，官僚机构，档案体，理性崩塌"
                .to_string(),
            genre_association: Some("恐怖/都市奇幻".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "high".to_string(),
            abstraction: "balanced".to_string(),
            temporal_quality: "modern".to_string(),
            preferred_categories: vec![
                "官僚术语".to_string(),
                "建筑词汇".to_string(),
                "档案用语".to_string(),
                "异常描述".to_string(),
            ],
            signature_words: vec![
                "异常".to_string(),
                "档案".to_string(),
                "阈限".to_string(),
                "不可解".to_string(),
            ],
            avoided_patterns: vec!["传统鬼怪".to_string(), "宗教驱魔".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 28,
            clause_complexity: "moderate".to_string(),
            rhythm_pattern: "冷静描述，渐进不安".to_string(),
            preferred_structures: vec![
                "档案体".to_string(),
                "调查报告".to_string(),
                "列表式".to_string(),
            ],
            opening_variety: "moderate".to_string(),
            punctuation_style: "冷静精确，括号注释".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.03,
            preferred_devices: vec!["暗示".to_string(), "并列".to_string()],
            imagery_preference: vec![
                "建筑意象".to_string(),
                "档案意象".to_string(),
                "阈限意象".to_string(),
            ],
            parallelism_frequency: "moderate".to_string(),
            irony_usage: "subtle".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "first_person".to_string(),
            narrative_distance: "close".to_string(),
            interior_monologue_ratio: 0.3,
            omniscience_level: 0.0,
            temporal_handling: "nonlinear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "restrained".to_string(),
            emotion_word_density: 0.02,
            dominant_mood: "不安疏离".to_string(),
            emotional_arc_pattern: "gradual".to_string(),
            humor_style: "none".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.2,
            dialogue_length: "terse".to_string(),
            subtext_ratio: 0.7,
            signature_patterns: vec![
                "官僚问答".to_string(),
                "录音转写".to_string(),
                "冷静报告".to_string(),
            ],
            tag_style: "minimal".to_string(),
        },
    }
}

/// 硬科幻风格
/// 特征：技术细节、概念密集、冷静、工程师思维
pub fn hard_sf() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "硬科幻".to_string(),
            author: None,
            description: "以严格科学为基础，技术细节密集，工程师思维，概念优先，冷静推演"
                .to_string(),
            genre_association: Some("硬科幻".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "high".to_string(),
            abstraction: "abstract".to_string(),
            temporal_quality: "futuristic".to_string(),
            preferred_categories: vec![
                "物理术语".to_string(),
                "工程词汇".to_string(),
                "数学概念".to_string(),
                "天文术语".to_string(),
            ],
            signature_words: vec![
                "轨道".to_string(),
                "引擎".to_string(),
                "辐射".to_string(),
                "计算".to_string(),
            ],
            avoided_patterns: vec!["情感铺陈".to_string(), "魔法元素".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 32,
            clause_complexity: "complex".to_string(),
            rhythm_pattern: "信息密集，逻辑推进".to_string(),
            preferred_structures: vec![
                "技术说明".to_string(),
                "推演论证".to_string(),
                "场景模拟".to_string(),
            ],
            opening_variety: "moderate".to_string(),
            punctuation_style: "精确清晰，术语密集".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.01,
            preferred_devices: vec!["类比".to_string()],
            imagery_preference: vec!["科技意象".to_string(), "宇宙意象".to_string()],
            parallelism_frequency: "rare".to_string(),
            irony_usage: "none".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "close_third".to_string(),
            narrative_distance: "close".to_string(),
            interior_monologue_ratio: 0.15,
            omniscience_level: 0.3,
            temporal_handling: "linear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "restrained".to_string(),
            emotion_word_density: 0.01,
            dominant_mood: "冷静理性".to_string(),
            emotional_arc_pattern: "gradual".to_string(),
            humor_style: "dry".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.25,
            dialogue_length: "terse".to_string(),
            subtext_ratio: 0.2,
            signature_patterns: vec![
                "技术讨论".to_string(),
                "简报式".to_string(),
                "工程师幽默".to_string(),
            ],
            tag_style: "said_only".to_string(),
        },
    }
}

/// 史诗奇幻风格
/// 特征：托尔金式、宏大、神话、中古用语
pub fn epic_fantasy() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "史诗奇幻".to_string(),
            author: None,
            description: "托尔金式宏大奇幻，神话体系，中古氛围，善恶对抗，世界观详尽".to_string(),
            genre_association: Some("奇幻/史诗".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "high".to_string(),
            abstraction: "balanced".to_string(),
            temporal_quality: "archaic".to_string(),
            preferred_categories: vec![
                "中古词汇".to_string(),
                "神话术语".to_string(),
                "种族用语".to_string(),
                "魔法词汇".to_string(),
            ],
            signature_words: vec![
                "命运".to_string(),
                "王国".to_string(),
                "宝剑".to_string(),
                "龙".to_string(),
            ],
            avoided_patterns: vec!["现代科技".to_string(), "口语俚语".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 34,
            clause_complexity: "complex".to_string(),
            rhythm_pattern: "宏大庄严，史诗吟诵".to_string(),
            preferred_structures: vec![
                "史诗叙述".to_string(),
                "预言".to_string(),
                "种族语言".to_string(),
            ],
            opening_variety: "varied".to_string(),
            punctuation_style: "传统庄重，长句为主".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.08,
            preferred_devices: vec!["比喻".to_string(), "象征".to_string(), "预言".to_string()],
            imagery_preference: vec![
                "自然意象".to_string(),
                "神话意象".to_string(),
                "战争意象".to_string(),
            ],
            parallelism_frequency: "frequent".to_string(),
            irony_usage: "none".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "multiple".to_string(),
            narrative_distance: "moderate".to_string(),
            interior_monologue_ratio: 0.2,
            omniscience_level: 0.8,
            temporal_handling: "nonlinear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "expressive".to_string(),
            emotion_word_density: 0.06,
            dominant_mood: "庄严悲壮".to_string(),
            emotional_arc_pattern: "gradual".to_string(),
            humor_style: "none".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.3,
            dialogue_length: "verbose".to_string(),
            subtext_ratio: 0.3,
            signature_patterns: vec![
                "中古腔调".to_string(),
                "预言式".to_string(),
                "种族口音".to_string(),
            ],
            tag_style: "varied_tags".to_string(),
        },
    }
}

/// 黑暗奇幻风格
/// 特征：残酷、灰色道德、血腥、现实主义魔法
pub fn grimdark() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "黑暗奇幻".to_string(),
            author: None,
            description: "残酷现实主义奇幻，灰色道德，血腥暴力，魔法代价高昂，世界黑暗".to_string(),
            genre_association: Some("奇幻/黑暗".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "high".to_string(),
            abstraction: "concrete".to_string(),
            temporal_quality: "archaic".to_string(),
            preferred_categories: vec![
                "战争术语".to_string(),
                "酷刑词汇".to_string(),
                "政治术语".to_string(),
                "粗俗语".to_string(),
            ],
            signature_words: vec![
                "血".to_string(),
                "背叛".to_string(),
                "权力".to_string(),
                "死亡".to_string(),
            ],
            avoided_patterns: vec!["童话氛围".to_string(), "英雄光环".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 26,
            clause_complexity: "moderate".to_string(),
            rhythm_pattern: "冷酷直接，节奏紧凑".to_string(),
            preferred_structures: vec![
                "多视角".to_string(),
                "政治博弈".to_string(),
                "战斗描写".to_string(),
            ],
            opening_variety: "varied".to_string(),
            punctuation_style: "直接冷酷，短句为主".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.04,
            preferred_devices: vec!["讽刺".to_string(), "对比".to_string()],
            imagery_preference: vec![
                "战争意象".to_string(),
                "政治意象".to_string(),
                "黑暗意象".to_string(),
            ],
            parallelism_frequency: "rare".to_string(),
            irony_usage: "overt".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "multiple".to_string(),
            narrative_distance: "close".to_string(),
            interior_monologue_ratio: 0.25,
            omniscience_level: 0.5,
            temporal_handling: "nonlinear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "restrained".to_string(),
            emotion_word_density: 0.03,
            dominant_mood: "冷酷绝望".to_string(),
            emotional_arc_pattern: "sudden".to_string(),
            humor_style: "dark".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.3,
            dialogue_length: "terse".to_string(),
            subtext_ratio: 0.6,
            signature_patterns: vec![
                "威胁".to_string(),
                "政治谈判".to_string(),
                "粗口".to_string(),
            ],
            tag_style: "action_beats".to_string(),
        },
    }
}

/// 仙侠修真风格
/// 特征：古风、升级、世界观、丹药法宝
pub fn xianxia() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "仙侠修真".to_string(),
            author: None,
            description: "东方玄幻修真体系，境界升级，丹药法宝，宗门派系，古风语言".to_string(),
            genre_association: Some("仙侠/玄幻".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "high".to_string(),
            abstraction: "abstract".to_string(),
            temporal_quality: "archaic".to_string(),
            preferred_categories: vec![
                "修真术语".to_string(),
                "丹药名称".to_string(),
                "法宝词汇".to_string(),
                "境界称谓".to_string(),
            ],
            signature_words: vec![
                "境界".to_string(),
                "灵气".to_string(),
                "法宝".to_string(),
                "突破".to_string(),
            ],
            avoided_patterns: vec!["现代科技".to_string(), "西方术语".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 28,
            clause_complexity: "moderate".to_string(),
            rhythm_pattern: "古风流畅，战斗紧凑".to_string(),
            preferred_structures: vec![
                "境界说明".to_string(),
                "战斗描写".to_string(),
                "功法描述".to_string(),
            ],
            opening_variety: "moderate".to_string(),
            punctuation_style: "传统标点，战斗短句".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.06,
            preferred_devices: vec!["比喻".to_string(), "夸张".to_string()],
            imagery_preference: vec![
                "自然意象".to_string(),
                "神话意象".to_string(),
                "战斗意象".to_string(),
            ],
            parallelism_frequency: "moderate".to_string(),
            irony_usage: "subtle".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "close_third".to_string(),
            narrative_distance: "close".to_string(),
            interior_monologue_ratio: 0.3,
            omniscience_level: 0.3,
            temporal_handling: "linear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "balanced".to_string(),
            emotion_word_density: 0.04,
            dominant_mood: "热血执念".to_string(),
            emotional_arc_pattern: "gradual".to_string(),
            humor_style: "none".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.3,
            dialogue_length: "moderate".to_string(),
            subtext_ratio: 0.4,
            signature_patterns: vec![
                "古风对白".to_string(),
                "宗门规矩".to_string(),
                "挑衅".to_string(),
            ],
            tag_style: "action_beats".to_string(),
        },
    }
}

/// 无限流风格
/// 特征：副本、惊悚、智斗、系统提示
pub fn infinite_flow() -> StyleDNA {
    StyleDNA {
        meta: StyleMeta {
            name: "无限流".to_string(),
            author: None,
            description: "副本闯关体系，惊悚生存，智斗博弈，系统提示，数据面板，团队协作"
                .to_string(),
            genre_association: Some("无限流/惊悚".to_string()),
        },
        vocabulary: VocabularyProfile {
            density: "high".to_string(),
            abstraction: "concrete".to_string(),
            temporal_quality: "modern".to_string(),
            preferred_categories: vec![
                "游戏术语".to_string(),
                "恐怖元素".to_string(),
                "系统提示".to_string(),
                "数据词汇".to_string(),
            ],
            signature_words: vec![
                "副本".to_string(),
                "系统".to_string(),
                "积分".to_string(),
                "生存".to_string(),
            ],
            avoided_patterns: vec!["抒情议论".to_string()],
        },
        syntax: SyntaxProfile {
            avg_sentence_length: 22,
            clause_complexity: "moderate".to_string(),
            rhythm_pattern: "紧张刺激，信息轰炸".to_string(),
            preferred_structures: vec![
                "系统提示".to_string(),
                "规则说明".to_string(),
                "智斗推演".to_string(),
            ],
            opening_variety: "moderate".to_string(),
            punctuation_style: "快节奏，括号注释".to_string(),
        },
        rhetoric: RhetoricProfile {
            metaphor_density: 0.02,
            preferred_devices: vec!["悬念".to_string(), "伏笔".to_string()],
            imagery_preference: vec!["恐怖意象".to_string(), "游戏意象".to_string()],
            parallelism_frequency: "rare".to_string(),
            irony_usage: "subtle".to_string(),
        },
        perspective: PerspectiveProfile {
            pov_type: "close_third".to_string(),
            narrative_distance: "close".to_string(),
            interior_monologue_ratio: 0.25,
            omniscience_level: 0.2,
            temporal_handling: "linear".to_string(),
        },
        emotion: EmotionProfile {
            expressiveness: "balanced".to_string(),
            emotion_word_density: 0.03,
            dominant_mood: "紧张刺激".to_string(),
            emotional_arc_pattern: "sudden".to_string(),
            humor_style: "dry".to_string(),
        },
        dialogue: DialogueProfile {
            dialogue_ratio: 0.35,
            dialogue_length: "terse".to_string(),
            subtext_ratio: 0.4,
            signature_patterns: vec![
                "系统提示音".to_string(),
                "团队战术".to_string(),
                "规则讨论".to_string(),
            ],
            tag_style: "minimal".to_string(),
        },
    }
}

// ==================== 扩展库汇总函数 ====================

/// 获取所有扩展的经典风格（40种）
pub fn get_extended_styles() -> Vec<StyleDNA> {
    vec![
        // 中国文学（12种）
        lu_xun(),
        lao_she(),
        shen_congwen(),
        yu_hua(),
        wang_xiaobo(),
        cao_xueqin(),
        pu_songling(),
        su_shi(),
        a_cheng(),
        bai_xianyong(),
        qian_zhongshu(),
        yu_dafu(),
        // 日本文学（6种）
        kawabata_yasunari(),
        mishima_yukio(),
        dazai_osamu(),
        natsume_soseki(),
        akutagawa_ryunosuke(),
        higashino_keigo(),
        // 欧美文学（14种）
        dostoevsky(),
        tolstoy(),
        kafka(),
        faulkner(),
        fitzgerald(),
        borges(),
        cortazar(),
        poe(),
        lovecraft(),
        austen(),
        dickens(),
        flaubert(),
        hugo(),
        nabokov(),
        // 类型文学（8种）
        cyberpunk(),
        steampunk(),
        new_weird(),
        hard_sf(),
        epic_fantasy(),
        grimdark(),
        xianxia(),
        infinite_flow(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extended_styles_count() {
        let styles = get_extended_styles();
        assert_eq!(styles.len(), 40);
    }

    #[test]
    fn test_lu_xun_dna() {
        let dna = lu_xun();
        assert_eq!(dna.meta.name, "鲁迅");
        assert_eq!(dna.emotion.expressiveness, "restrained");
    }

    #[test]
    fn test_dostoevsky_dna() {
        let dna = dostoevsky();
        assert_eq!(dna.meta.name, "陀思妥耶夫斯基");
        assert!(dna.syntax.avg_sentence_length > 50);
    }

    #[test]
    fn test_cyberpunk_dna() {
        let dna = cyberpunk();
        assert_eq!(dna.meta.name, "赛博朋克");
        assert_eq!(dna.vocabulary.temporal_quality, "futuristic");
    }
}
