/**
 * 智能文思 — 感知层类型定义
 *
 * 三层架构：
 *   感知层 (Perception): 实时分析编辑器文本，提取结构化特征
 *   决策层 (Decision):    基于特征决定建议类型、时机、展示方式
 *   表达层 (Presentation): 将建议以最佳形式呈现给用户
 */

// ==================== 感知层输出 ====================

/** 段落分析结果 */
export interface ParagraphAnalysis {
  /** 段落总字数 */
  charCount: number;
  /** 段落句子数 */
  sentenceCount: number;
  /** 平均句长 */
  avgSentenceLength: number;
  /** 段落类型推断 */
  type: 'dialogue' | 'description' | 'narrative' | 'mixed' | 'short';
  /** 对话占比 (0-1) */
  dialogueRatio: number;
  /** 描写占比 (0-1) */
  descriptionRatio: number;
  /** 是否以对话为主 */
  isDialogueHeavy: boolean;
  /** 是否以描写为主 */
  isDescriptionHeavy: boolean;
}

/** 句式分析结果 */
export interface SentencePatternAnalysis {
  /** 总句子数 */
  totalSentences: number;
  /** 平均句长（字） */
  avgLength: number;
  /** 短句占比 (<10字) */
  shortSentenceRatio: number;
  /** 长句占比 (>30字) */
  longSentenceRatio: number;
  /** 句式多样性指数 (0-1) */
  varietyIndex: number;
  /** 常见开头词统计 */
  topStarters: Array<{ word: string; count: number }>;
  /** 是否句式单调 */
  isMonotonous: boolean;
}

/** 词汇分析结果 */
export interface VocabularyAnalysis {
  /** 总词数（以字为单位统计有意义的词） */
  totalWords: number;
  /** 唯一词数 */
  uniqueWords: number;
  /** 词汇丰富度 (unique/total, 0-1) */
  richness: number;
  /** 高频重复词 */
  repeatedWords: Array<{ word: string; count: number; ratio: number }>;
  /** 是否有严重重复 */
  hasRepetition: boolean;
  /** 形容词密度 */
  adjectiveDensity: number;
  /** 动词密度 */
  verbDensity: number;
}

/** 节奏分析结果 */
export interface PacingAnalysis {
  /** 整体节奏评分 (0-1, 1=变化丰富) */
  variationScore: number;
  /** 段落长度变化度 */
  paragraphVariation: number;
  /** 对话-叙述交替频率 */
  dialogueNarrativeAlternation: number;
  /** 当前区域节奏类型 */
  currentPacing: 'fast' | 'slow' | 'steady' | 'mixed';
  /** 是否存在连续同类型段落 */
  hasMonotonousSequence: boolean;
}

/** 内容类型分布 */
export interface ContentDistribution {
  /** 对话占比 */
  dialogue: number;
  /** 环境/动作描写占比 */
  description: number;
  /** 叙述/推进占比 */
  narrative: number;
  /** 心理/情感描写占比 */
  emotion: number;
  /** 主导类型 */
  dominant: 'dialogue' | 'description' | 'narrative' | 'emotion';
}

/** 感知层完整输出 */
export interface PerceptionResult {
  /** 全文总字数 */
  totalChars: number;
  /** 段落分析 */
  paragraphs: ParagraphAnalysis[];
  /** 句式分析 */
  sentencePattern: SentencePatternAnalysis;
  /** 词汇分析 */
  vocabulary: VocabularyAnalysis;
  /** 节奏分析 */
  pacing: PacingAnalysis;
  /** 内容分布 */
  contentDistribution: ContentDistribution;
  /** 分析时间戳 */
  analyzedAt: number;
}

// ==================== 决策层输出 ====================

/** 建议类型 */
export type SuggestionCategory =
  | 'pacing' // 节奏
  | 'dialogue' // 对话
  | 'description' // 描写
  | 'vocabulary' // 词汇
  | 'sentence' // 句式
  | 'emotion' // 情感
  | 'plot' // 情节
  | 'structure'; // 结构

/** 建议优先级 */
export type SuggestionPriority = 'high' | 'medium' | 'low';

/** 建议展示方式 */
export type SuggestionPresentation =
  | 'bubble' // 右侧气泡（当前段落相关）
  | 'ghost' // Ghost Text（输入栏）
  | 'inline' // 内联提示（段落边缘）
  | 'ambient'; // 环境提示（屏幕边缘）

/** 单条建议 */
export interface WritingSuggestion {
  id: string;
  category: SuggestionCategory;
  priority: SuggestionPriority;
  presentation: SuggestionPresentation;
  /** 建议标题（简短） */
  title: string;
  /** 建议内容 */
  message: string;
  /** 针对的段落索引（-1 表示全文） */
  targetParagraphIndex: number;
  /** 相关文本片段 */
  relevantText?: string;
  /** 触发此建议的具体原因 */
  triggerReason: string;
  /** 建议创建时间 */
  createdAt: number;
  /** 用户反馈：null=未反馈, true=有用, false=无用 */
  userFeedback: boolean | null;
  /** 传给 LLM 的修改指令 */
  instruction: string;
}

/** 决策层输出 */
export interface DecisionResult {
  suggestions: WritingSuggestion[];
  /** 是否处于"高创作负荷"状态（建议减少打扰） */
  isHighLoad: boolean;
  /** 建议展示间隔（毫秒） */
  displayInterval: number;
  /** 当前最突出的写作问题 */
  topIssue: SuggestionCategory | null;
}

// ==================== 表达层配置 ====================

export interface PresentationConfig {
  /** 是否启用气泡提示 */
  enableBubbles: boolean;
  /** 是否启用 Ghost Text */
  enableGhost: boolean;
  /** 是否启用环境提示 */
  enableAmbient: boolean;
  /** 最小展示间隔（毫秒） */
  minInterval: number;
  /** 同时最多显示几条建议 */
  maxConcurrent: number;
  /**  Zen 模式下完全关闭 */
  disableInZen: boolean;
}

export const DEFAULT_PRESENTATION_CONFIG: PresentationConfig = {
  enableBubbles: true,
  enableGhost: true,
  enableAmbient: false, // 默认关闭环境提示，减少打扰
  minInterval: 8000,
  maxConcurrent: 2,
  disableInZen: true,
};
