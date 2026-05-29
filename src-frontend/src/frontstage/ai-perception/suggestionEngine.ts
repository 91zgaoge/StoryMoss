/**
 * 智能文思 — 决策层：建议策略引擎
 *
 * 基于感知层的分析结果，生成有针对性的写作建议。
 * 纯前端实现，零后端调用。
 */

import type {
  PerceptionResult,
  WritingSuggestion,
  DecisionResult,
  SuggestionCategory,
  SuggestionPriority,
  SuggestionPresentation,
} from './types';

// ==================== 建议模板库 ====================

interface SuggestionTemplate {
  category: SuggestionCategory;
  priority: SuggestionPriority;
  presentation: SuggestionPresentation;
  title: string;
  messages: string[]; // 多条候选，随机选一条避免单调
  /** 传给 LLM 的修改指令（可选，未提供时使用默认值） */
  instruction?: string;
  condition: (p: PerceptionResult) => boolean;
  score: (p: PerceptionResult) => number; // 0-1，匹配度
}

const TEMPLATES: SuggestionTemplate[] = [
  // === 节奏类 ===
  {
    category: 'pacing',
    priority: 'high',
    presentation: 'bubble',
    title: '节奏单调',
    messages: [
      '连续多段同类型内容，节奏略显单调。可插入一段不同风格的描写来打破沉闷。',
      '此处节奏持续平稳，读者可能会感到倦怠。尝试加快或放慢一段，制造起伏。',
      '连续的环境描写让节奏变慢了，或许可以加入一段对话或动作来推进情节。',
    ],
    condition: p => p.pacing.hasMonotonousSequence,
    score: p => (p.pacing.hasMonotonousSequence ? 0.9 : 0),
  },
  {
    category: 'pacing',
    priority: 'medium',
    presentation: 'ambient',
    title: '节奏偏慢',
    messages: [
      '段落较长，节奏偏慢。适合营造氛围，但不宜持续太久。',
      '大段描写正在铺陈氛围，注意不要让读者失去耐心。',
    ],
    condition: p => p.pacing.currentPacing === 'slow' && p.paragraphs.length >= 3,
    score: p => (p.pacing.currentPacing === 'slow' ? 0.6 : 0),
  },
  {
    category: 'pacing',
    priority: 'medium',
    presentation: 'ambient',
    title: '节奏偏快',
    messages: [
      '对话密集，节奏很快。适当插入描写让读者喘口气。',
      '情节推进很快，可以考虑在某处稍作停顿，加深情感张力。',
    ],
    condition: p => p.pacing.currentPacing === 'fast' && p.paragraphs.length >= 3,
    score: p => (p.pacing.currentPacing === 'fast' ? 0.5 : 0),
  },

  // === 对话类 ===
  {
    category: 'dialogue',
    priority: 'medium',
    presentation: 'bubble',
    title: '对话缺少动作',
    messages: [
      '大段对话中缺少动作描写，人物显得有些"悬浮"。试试在对话间插入一些小动作。',
      '对话流畅，但如果在说话时加入一些神态或动作，人物会更加立体。',
      '此处对话密集，可在关键对白前后添加人物的表情或肢体语言。',
    ],
    condition: p => {
      const last3 = p.paragraphs.slice(-3);
      return last3.length >= 2 && last3.every(pa => pa.type === 'dialogue');
    },
    score: p => {
      const last3 = p.paragraphs.slice(-3);
      return last3.length >= 2 && last3.every(pa => pa.type === 'dialogue') ? 0.8 : 0;
    },
  },
  {
    category: 'dialogue',
    priority: 'low',
    presentation: 'ghost',
    title: '对话提示',
    messages: [
      '试着让对话更有张力——一方话中有话，另一方心领神会。',
      '好的对话不仅是信息的传递，更是性格的交锋。',
    ],
    condition: p => p.contentDistribution.dialogue > 0.3,
    score: p => p.contentDistribution.dialogue * 0.4,
  },

  // === 描写类 ===
  {
    category: 'description',
    priority: 'medium',
    presentation: 'bubble',
    title: '描写可更具体',
    messages: [
      '环境描写较多，但缺少感官细节。试试加入声音、气味或触觉。',
      '此处描写不错，如果能让读者"看到"更多细节，画面感会更强。',
      '描写正营造氛围，可考虑用一两个独特的意象来加深印象。',
    ],
    condition: p => {
      const last2 = p.paragraphs.slice(-2);
      return last2.length >= 2 && last2.every(pa => pa.type === 'description');
    },
    score: p => {
      const last2 = p.paragraphs.slice(-2);
      return last2.length >= 2 && last2.every(pa => pa.type === 'description') ? 0.7 : 0;
    },
  },
  {
    category: 'description',
    priority: 'low',
    presentation: 'ambient',
    title: '描写占比偏高',
    messages: [
      '描写占比较高，注意保持叙事推进的节奏。',
      '丰富的描写能营造氛围，但也要给情节发展留出空间。',
    ],
    condition: p => p.contentDistribution.description > 0.5 && p.paragraphs.length >= 4,
    score: p => Math.max(0, p.contentDistribution.description - 0.4),
  },

  // === 词汇类 ===
  {
    category: 'vocabulary',
    priority: 'high',
    presentation: 'bubble',
    title: '词汇重复',
    messages: [
      `注意到"{word}"出现较频繁。试着用同义词或不同的表达方式来替换其中几处。`,
      '某些词汇重复较多，适当替换能让文字更有变化。',
      '用词有重复倾向，可以尝试换一种说法，增加语言的丰富性。',
    ],
    condition: p => p.vocabulary.hasRepetition,
    score: p => (p.vocabulary.hasRepetition ? 0.85 : 0),
  },
  {
    category: 'vocabulary',
    priority: 'medium',
    presentation: 'ghost',
    title: '词汇丰富度',
    messages: [
      '词汇丰富度有提升空间。试着用更精准的动词或更生动的形容词。',
      '文字的质感可以通过词汇的精心选择来提升。',
    ],
    condition: p => p.vocabulary.richness < 0.4 && p.vocabulary.totalWords > 100,
    score: p => Math.max(0, 0.5 - p.vocabulary.richness),
  },

  // === 句式类 ===
  {
    category: 'sentence',
    priority: 'medium',
    presentation: 'bubble',
    title: '句式单调',
    messages: [
      `多个段落以"{starter}"开头，句式略显单调。试着变换一下开头方式。`,
      '句式结构比较单一，长短句交替会让文字更有韵律感。',
      '注意句式的多样性，适当使用倒装、省略或排比等手法。',
    ],
    condition: p => p.sentencePattern.isMonotonous,
    score: p => (p.sentencePattern.isMonotonous ? 0.75 : 0),
  },
  {
    category: 'sentence',
    priority: 'low',
    presentation: 'ambient',
    title: '长句较多',
    messages: [
      '长句较多，信息密度大。适当拆分或使用短句来调节呼吸感。',
      '长句能承载复杂的情感，但连续使用会让读者疲劳。',
    ],
    condition: p => p.sentencePattern.longSentenceRatio > 0.4,
    score: p => p.sentencePattern.longSentenceRatio,
  },
  {
    category: 'sentence',
    priority: 'low',
    presentation: 'ambient',
    title: '短句较多',
    messages: [
      '短句节奏明快，但如果一直这么快，读者难以沉浸。',
      '短句有力，但也需要一些舒展的长句来平衡。',
    ],
    condition: p => p.sentencePattern.shortSentenceRatio > 0.5,
    score: p => p.sentencePattern.shortSentenceRatio,
  },

  // === 情感类 ===
  {
    category: 'emotion',
    priority: 'medium',
    presentation: 'bubble',
    title: '情感层次',
    messages: [
      '叙述推进较多，情感表达稍显不足。可以试试深入人物的内心世界。',
      '情节在推进，但人物的情感反应可以写得更细腻一些。',
    ],
    condition: p => p.contentDistribution.narrative > 0.6 && p.totalChars > 200,
    score: p => Math.max(0, p.contentDistribution.narrative - 0.5),
  },

  // === 情节类 ===
  {
    category: 'plot',
    priority: 'low',
    presentation: 'ghost',
    title: '情节推进',
    messages: [
      '此处可以埋下一个伏笔，为后文制造呼应。',
      '试试让某个细节成为后续情节的关键线索。',
      '情节正在推进，考虑一下下一步的转折是否合理且有张力。',
    ],
    condition: p => p.totalChars > 300 && p.pacing.currentPacing === 'mixed',
    score: p => (p.totalChars > 300 ? 0.3 : 0),
  },

  // === 结构类 ===
  {
    category: 'structure',
    priority: 'medium',
    presentation: 'bubble',
    title: '段落过短',
    messages: [
      '连续出现短段落，可能是节奏被打断。检查是否需要合并或扩展。',
      '段落偏短，内容显得零散。尝试将相关描述合并为更完整的段落。',
    ],
    condition: p => {
      const last3 = p.paragraphs.slice(-3);
      return last3.length >= 3 && last3.every(pa => pa.charCount < 30);
    },
    score: p => {
      const last3 = p.paragraphs.slice(-3);
      return last3.length >= 3 && last3.every(pa => pa.charCount < 30) ? 0.7 : 0;
    },
  },
  {
    category: 'structure',
    priority: 'low',
    presentation: 'ambient',
    title: '段落结构',
    messages: ['段落长度变化丰富，结构有层次感。', '段落节奏不错，长短交错自然。'],
    condition: p => p.pacing.paragraphVariation > 0.5 && p.paragraphs.length >= 5,
    score: p => p.pacing.paragraphVariation * 0.3,
  },
];

// ==================== 决策引擎 ====================

/**
 * 基于感知结果生成写作建议
 */
export function generateSuggestions(perception: PerceptionResult): DecisionResult {
  const suggestions: WritingSuggestion[] = [];

  // 评估每条模板
  for (const template of TEMPLATES) {
    if (!template.condition(perception)) continue;

    const score = template.score(perception);
    if (score < 0.3) continue; // 匹配度太低的忽略

    // 选择一条消息
    const message = template.messages[Math.floor(Math.random() * template.messages.length)];

    // 处理模板变量
    let finalMessage = message;
    if (finalMessage.includes('{word}') && perception.vocabulary.repeatedWords.length > 0) {
      finalMessage = finalMessage.replace('{word}', perception.vocabulary.repeatedWords[0].word);
    }
    if (finalMessage.includes('{starter}') && perception.sentencePattern.topStarters.length > 0) {
      finalMessage = finalMessage.replace(
        '{starter}',
        perception.sentencePattern.topStarters[0].word
      );
    }

    // 默认指令映射
    const defaultInstructions: Record<string, string> = {
      pacing: '改写这段文字，调整叙事节奏。增加对话与描写的交替变化，避免单一节奏的疲劳感。',
      dialogue: '改写这段对话，在人物说话时加入动作、神态或环境细节，让对话更立体生动。',
      description: '改写这段描写，增加感官细节（声音、气味、触觉等），让画面更加具体可感。',
      vocabulary: '润色这段文字，替换重复使用的词汇，使用更丰富多样的表达方式。',
      sentence: '改写这段文字，增加句式多样性。尝试长短句交替、倒装、省略等不同句式。',
      emotion: '改写这段文字，深入描写人物的内心活动和情感变化，增强读者的情感共鸣。',
      plot: '改写这段文字，增加情节张力。可以加入伏笔、转折或冲突升级的元素。',
      structure: '改写这段文字，将零散的句子整合为结构更完整的段落，让叙述更连贯。',
    };

    suggestions.push({
      id: `${template.category}-${Date.now()}-${Math.random().toString(36).substr(2, 5)}`,
      category: template.category,
      priority: template.priority,
      presentation: template.presentation,
      title: template.title,
      message: finalMessage,
      targetParagraphIndex: -1,
      triggerReason: `score=${score.toFixed(2)}, condition matched`,
      createdAt: Date.now(),
      userFeedback: null,
      instruction:
        (template as any).instruction ||
        defaultInstructions[template.category] ||
        '润色这段文字，提升写作质量。',
    });
  }

  // 按优先级和分数排序
  const priorityOrder = { high: 0, medium: 1, low: 2 };
  suggestions.sort((a, b) => priorityOrder[a.priority] - priorityOrder[b.priority]);

  // 确定是否处于高创作负荷状态
  // 如果用户正在快速写作（字数多、段落多、节奏快），减少打扰
  const isHighLoad =
    perception.totalChars > 500 &&
    perception.paragraphs.length > 8 &&
    perception.pacing.currentPacing === 'fast';

  // 确定展示间隔
  const displayInterval = isHighLoad ? 15000 : 8000;

  // 确定最突出的问题
  const topIssue = suggestions.length > 0 ? suggestions[0].category : null;

  return {
    suggestions: suggestions.slice(0, 5), // 最多同时保留 5 条待展示
    isHighLoad,
    displayInterval,
    topIssue,
  };
}

/**
 * 过滤建议：根据展示配置和用户反馈历史
 */
export function filterSuggestions(
  decision: DecisionResult,
  feedbackHistory: Map<string, boolean>, // suggestion category -> was_useful
  config: { enableBubbles: boolean; enableGhost: boolean; enableAmbient: boolean }
): WritingSuggestion[] {
  return decision.suggestions.filter(s => {
    // 根据展示配置过滤
    if (s.presentation === 'bubble' && !config.enableBubbles) return false;
    if (s.presentation === 'ghost' && !config.enableGhost) return false;
    if (s.presentation === 'ambient' && !config.enableAmbient) return false;

    // 如果用户之前标记过某类建议无用，降低其出现频率（但不完全屏蔽）
    const previousFeedback = feedbackHistory.get(s.category);
    if (previousFeedback === false) {
      // 用户觉得这类建议没用，只有 high 优先级的才保留
      return s.priority === 'high';
    }

    return true;
  });
}

/**
 * 选择下一条要展示的建议
 */
export function selectNextSuggestion(
  available: WritingSuggestion[],
  currentlyDisplayed: Set<string>
): WritingSuggestion | null {
  // 排除已在展示的
  const candidates = available.filter(s => !currentlyDisplayed.has(s.id));
  if (candidates.length === 0) return null;

  // 优先返回高优先级且未展示过的
  const highPriority = candidates.filter(s => s.priority === 'high');
  if (highPriority.length > 0) {
    return highPriority[0];
  }

  // 然后返回中等优先级
  const mediumPriority = candidates.filter(s => s.priority === 'medium');
  if (mediumPriority.length > 0) {
    return mediumPriority[0];
  }

  // 最后返回低优先级
  return candidates[0];
}

/**
 * 记录用户反馈
 */
export function recordFeedback(
  suggestion: WritingSuggestion,
  wasUseful: boolean,
  feedbackHistory: Map<string, boolean>
): Map<string, boolean> {
  const next = new Map(feedbackHistory);
  next.set(suggestion.category, wasUseful);
  return next;
}
