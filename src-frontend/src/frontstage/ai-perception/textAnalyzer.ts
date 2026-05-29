/**
 * 智能文思 — 感知层：文本实时分析引擎
 *
 * 纯前端实现，零后端调用，零网络延迟。
 * 基于中文文本的启发式分析，提取写作特征。
 */

import type {
  PerceptionResult,
  ParagraphAnalysis,
  SentencePatternAnalysis,
  VocabularyAnalysis,
  PacingAnalysis,
  ContentDistribution,
} from './types';

// ==================== 辅助函数 ====================

/** 将 HTML 内容转为纯文本 */
function htmlToText(html: string): string {
  const tmp = document.createElement('div');
  tmp.innerHTML = html;
  return tmp.textContent || tmp.innerText || '';
}

/** 按段落分割文本 */
function splitParagraphs(text: string): string[] {
  return text
    .split(/\n+/)
    .map(p => p.trim())
    .filter(p => p.length > 0);
}

/** 按句子分割（中文标点） */
function splitSentences(text: string): string[] {
  return text
    .split(/([。！？；\.\!\?\;]+)/)
    .reduce((acc: string[], part, i, arr) => {
      if (i % 2 === 0 && part.trim()) {
        const punctuation = arr[i + 1] || '';
        acc.push(part.trim() + punctuation);
      }
      return acc;
    }, [])
    .filter(s => s.length > 0);
}

/** 检测是否为对话 */
function isDialogue(text: string): boolean {
  // 检测中文/英文引号包裹的内容
  const quotePairs = [
    ['\u201C', '\u201D'],
    ['\u2018', '\u2019'], // 中文弯引号 " " ' '
    ['\u300C', '\u300D'],
    ['\u300E', '\u300F'], // 中文角引号 「 」 『 』
    ['"', '"'],
    ["'", "'"], // 英文直引号
  ];
  for (const [open, close] of quotePairs) {
    const regex = new RegExp(open + '.{2,50}' + close);
    if (regex.test(text)) return true;
  }
  return false;
}

/** 检测是否为描写（环境/动作/外貌） */
function isDescription(text: string): boolean {
  const descKeywords = [
    '风',
    '雨',
    '雪',
    '云',
    '天',
    '地',
    '山',
    '水',
    '花',
    '树',
    '阳光',
    '月光',
    '灯光',
    '影子',
    '颜色',
    '声音',
    '气味',
    '走',
    '跑',
    '站',
    '坐',
    '看',
    '望',
    '低',
    '抬',
    '转',
    '红',
    '绿',
    '蓝',
    '白',
    '黑',
    '金',
    '银',
  ];
  const descCount = descKeywords.filter(kw => text.includes(kw)).length;
  return descCount >= 2;
}

/** 检测是否为心理/情感描写 */
function isEmotion(text: string): boolean {
  const emotionKeywords = [
    '想',
    '觉得',
    '感到',
    '感觉',
    '心里',
    '心中',
    '念头',
    '喜',
    '怒',
    '哀',
    '乐',
    '悲',
    '欢',
    '愁',
    '恨',
    '爱',
    '紧张',
    '害怕',
    '兴奋',
    '难过',
    '开心',
    '痛苦',
    '幸福',
  ];
  const emotionCount = emotionKeywords.filter(kw => text.includes(kw)).length;
  return emotionCount >= 1;
}

/** 分词（简单基于字典的前向最大匹配） */
function simpleTokenize(text: string): string[] {
  // 简单实现：按字拆分，过滤标点
  const chars = text.split('');
  const words: string[] = [];
  const stopChars = new Set([
    '\uFF0C',
    '\u3002',
    '\uFF01',
    '\uFF1F',
    '\uFF1B',
    '\uFF1A',
    '\u3001', // ，。！？；：、
    '\u201C',
    '\u201D',
    '\u2018',
    '\u2019', // " " ' '
    '\u300C',
    '\u300D',
    '\u300E',
    '\u300F', // 「 」 『 』
    '\uFF08',
    '\uFF09',
    '\u3010',
    '\u3011',
    '\u300A',
    '\u300B', // （）【】《》
    ' ',
    '\n',
    '\t',
  ]);

  // 简单的双字词检测
  const commonWords = new Set([
    '一个',
    '没有',
    '什么',
    '自己',
    '知道',
    '可以',
    '就是',
    '还是',
    '这样',
    '那个',
    '已经',
    '开始',
    '突然',
    '看着',
    '听到',
    '感觉',
    '心里',
    '不知',
    '只是',
    '一直',
    '不会',
    '不能',
    '不要',
    '这么',
    '那么',
    '如何',
    '为什么',
    '因为',
    '所以',
    '但是',
    '时间',
    '地方',
    '世界',
    '目光',
    '声音',
    '身体',
    '脸上',
    '眼中',
    '心中',
    '手里',
  ]);

  let i = 0;
  while (i < chars.length) {
    const ch = chars[i];
    if (stopChars.has(ch)) {
      i++;
      continue;
    }
    // 尝试匹配双字词
    if (i + 1 < chars.length) {
      const twoChars = ch + chars[i + 1];
      if (commonWords.has(twoChars)) {
        words.push(twoChars);
        i += 2;
        continue;
      }
    }
    words.push(ch);
    i++;
  }
  return words;
}

/** 统计词频 */
function countWordFrequency(words: string[]): Map<string, number> {
  const freq = new Map<string, number>();
  for (const word of words) {
    freq.set(word, (freq.get(word) || 0) + 1);
  }
  return freq;
}

// ==================== 分析函数 ====================

function analyzeParagraph(text: string): ParagraphAnalysis {
  const sentences = splitSentences(text);
  const charCount = text.length;
  const sentenceCount = sentences.length;
  const avgSentenceLength = sentenceCount > 0 ? charCount / sentenceCount : 0;

  // 检测段落类型
  const dialogueSentences = sentences.filter(isDialogue).length;
  const descriptionSentences = sentences.filter(isDescription).length;
  const emotionSentences = sentences.filter(isEmotion).length;

  const dialogueRatio = sentenceCount > 0 ? dialogueSentences / sentenceCount : 0;
  const descriptionRatio = sentenceCount > 0 ? descriptionSentences / sentenceCount : 0;

  let type: ParagraphAnalysis['type'] = 'mixed';
  if (charCount < 30) {
    type = 'short';
  } else if (dialogueRatio > 0.6) {
    type = 'dialogue';
  } else if (descriptionRatio > 0.5) {
    type = 'description';
  } else if (emotionSentences > sentences.length * 0.3) {
    type = 'description'; // 情感描写归类为描写
  }

  return {
    charCount,
    sentenceCount,
    avgSentenceLength,
    type,
    dialogueRatio,
    descriptionRatio,
    isDialogueHeavy: dialogueRatio > 0.6,
    isDescriptionHeavy: descriptionRatio > 0.5,
  };
}

function analyzeSentencePattern(paragraphs: string[]): SentencePatternAnalysis {
  const allSentences: string[] = [];
  for (const para of paragraphs) {
    allSentences.push(...splitSentences(para));
  }

  const totalSentences = allSentences.length;
  if (totalSentences === 0) {
    return {
      totalSentences: 0,
      avgLength: 0,
      shortSentenceRatio: 0,
      longSentenceRatio: 0,
      varietyIndex: 0,
      topStarters: [],
      isMonotonous: false,
    };
  }

  const lengths = allSentences.map(s => s.length);
  const avgLength = lengths.reduce((a, b) => a + b, 0) / totalSentences;
  const shortSentences = lengths.filter(l => l < 10).length;
  const longSentences = lengths.filter(l => l > 30).length;

  // 句式多样性：用长度标准差归一化
  const variance = lengths.reduce((sum, l) => sum + Math.pow(l - avgLength, 2), 0) / totalSentences;
  const stdDev = Math.sqrt(variance);
  const varietyIndex = Math.min(stdDev / 15, 1); // 归一化到 0-1

  // 开头词统计
  const starterCounts = new Map<string, number>();
  for (const sentence of allSentences) {
    const firstChar = sentence.charAt(0);
    if (firstChar) {
      starterCounts.set(firstChar, (starterCounts.get(firstChar) || 0) + 1);
    }
  }
  const topStarters = Array.from(starterCounts.entries())
    .map(([word, count]) => ({ word, count }))
    .sort((a, b) => b.count - a.count)
    .slice(0, 5);

  // 判断句式是否单调
  const maxStarterRatio = topStarters.length > 0 ? topStarters[0].count / totalSentences : 0;
  const isMonotonous = maxStarterRatio > 0.4 || varietyIndex < 0.15;

  return {
    totalSentences,
    avgLength,
    shortSentenceRatio: shortSentences / totalSentences,
    longSentenceRatio: longSentences / totalSentences,
    varietyIndex,
    topStarters,
    isMonotonous,
  };
}

function analyzeVocabulary(paragraphs: string[]): VocabularyAnalysis {
  const fullText = paragraphs.join('');
  const words = simpleTokenize(fullText);
  const totalWords = words.length;

  if (totalWords === 0) {
    return {
      totalWords: 0,
      uniqueWords: 0,
      richness: 0,
      repeatedWords: [],
      hasRepetition: false,
      adjectiveDensity: 0,
      verbDensity: 0,
    };
  }

  const freq = countWordFrequency(words);
  const uniqueWords = freq.size;
  const richness = uniqueWords / totalWords;

  // 找出高频重复词（排除常见虚词）
  const stopWords = new Set([
    '的',
    '了',
    '是',
    '在',
    '有',
    '我',
    '他',
    '她',
    '它',
    '你',
    '我们',
    '他们',
    '一',
    '不',
    '人',
    '都',
    '要',
    '会',
    '对',
    '也',
    '很',
    '好',
    '就',
    '让',
    '上',
    '下',
    '来',
    '去',
    '到',
    '说',
    '看',
    '着',
    '个',
    '这',
    '那',
  ]);

  const repeatedWords = Array.from(freq.entries())
    .filter(([word, count]) => !stopWords.has(word) && count >= 3)
    .map(([word, count]) => ({
      word,
      count,
      ratio: count / totalWords,
    }))
    .sort((a, b) => b.count - a.count)
    .slice(0, 8);

  const hasRepetition = repeatedWords.some(r => r.ratio > 0.05);

  // 简单的形容词/动词密度估计
  const adjIndicators = [
    '大',
    '小',
    '高',
    '低',
    '长',
    '短',
    '深',
    '浅',
    '新',
    '旧',
    '美',
    '丑',
    '冷',
    '热',
    '快',
    '慢',
    '轻',
    '重',
    '明',
    '暗',
    '红',
    '绿',
    '蓝',
    '白',
    '黑',
    '金',
    '银',
    '紫',
    '青',
    '黄',
  ];
  const verbIndicators = [
    '走',
    '跑',
    '跳',
    '站',
    '坐',
    '躺',
    '看',
    '望',
    '听',
    '说',
    '想',
    '拿',
    '放',
    '推',
    '拉',
    '打',
    '抓',
    '握',
    '挥',
    '点',
    '笑',
    '哭',
    '怒',
    '喜',
    '叹',
    '叫',
    '喊',
    '问',
    '答',
    '唱',
  ];

  const adjCount = words.filter(w => adjIndicators.includes(w)).length;
  const verbCount = words.filter(w => verbIndicators.includes(w)).length;

  return {
    totalWords,
    uniqueWords,
    richness,
    repeatedWords,
    hasRepetition,
    adjectiveDensity: totalWords > 0 ? adjCount / totalWords : 0,
    verbDensity: totalWords > 0 ? verbCount / totalWords : 0,
  };
}

function analyzePacing(paragraphs: string[]): PacingAnalysis {
  const paraAnalyses = paragraphs.map(analyzeParagraph);

  if (paraAnalyses.length === 0) {
    return {
      variationScore: 0,
      paragraphVariation: 0,
      dialogueNarrativeAlternation: 0,
      currentPacing: 'steady',
      hasMonotonousSequence: false,
    };
  }

  // 段落长度变化
  const lengths = paraAnalyses.map(p => p.charCount);
  const avgLen = lengths.reduce((a, b) => a + b, 0) / lengths.length;
  const lenVariance = lengths.reduce((sum, l) => sum + Math.pow(l - avgLen, 2), 0) / lengths.length;
  const paragraphVariation = Math.min(Math.sqrt(lenVariance) / avgLen, 1);

  // 对话-叙述交替频率
  let alternations = 0;
  for (let i = 1; i < paraAnalyses.length; i++) {
    const prev = paraAnalyses[i - 1].type;
    const curr = paraAnalyses[i].type;
    if (
      (prev === 'dialogue' && curr !== 'dialogue') ||
      (prev !== 'dialogue' && curr === 'dialogue')
    ) {
      alternations++;
    }
  }
  const dialogueNarrativeAlternation =
    paraAnalyses.length > 1 ? alternations / (paraAnalyses.length - 1) : 0;

  // 检测连续同类型段落
  let maxSameTypeSequence = 1;
  let currentSequence = 1;
  for (let i = 1; i < paraAnalyses.length; i++) {
    if (paraAnalyses[i].type === paraAnalyses[i - 1].type) {
      currentSequence++;
      maxSameTypeSequence = Math.max(maxSameTypeSequence, currentSequence);
    } else {
      currentSequence = 1;
    }
  }
  const hasMonotonousSequence = maxSameTypeSequence >= 4;

  // 当前节奏类型（基于最近3段）
  const recent = paraAnalyses.slice(-3);
  const avgRecentLength = recent.reduce((sum, p) => sum + p.charCount, 0) / recent.length;
  const dialogueCount = recent.filter(p => p.type === 'dialogue').length;

  let currentPacing: PacingAnalysis['currentPacing'] = 'steady';
  if (dialogueCount >= 2) {
    currentPacing = 'fast';
  } else if (avgRecentLength > 150) {
    currentPacing = 'slow';
  } else if (
    recent.some(p => p.type === 'description') &&
    recent.some(p => p.type === 'dialogue')
  ) {
    currentPacing = 'mixed';
  }

  // 综合变化度评分
  const variationScore = paragraphVariation * 0.4 + dialogueNarrativeAlternation * 0.6;

  return {
    variationScore,
    paragraphVariation,
    dialogueNarrativeAlternation,
    currentPacing,
    hasMonotonousSequence,
  };
}

function analyzeContentDistribution(paragraphs: string[]): ContentDistribution {
  const paraAnalyses = paragraphs.map(analyzeParagraph);

  if (paraAnalyses.length === 0) {
    return { dialogue: 0, description: 0, narrative: 0, emotion: 0, dominant: 'narrative' };
  }

  const dialogueCount = paraAnalyses.filter(p => p.type === 'dialogue').length;
  const descriptionCount = paraAnalyses.filter(p => p.type === 'description').length;
  const shortCount = paraAnalyses.filter(p => p.type === 'short').length;
  const mixedCount = paraAnalyses.filter(p => p.type === 'mixed').length;
  const total = paraAnalyses.length;

  const dialogue = dialogueCount / total;
  const description = descriptionCount / total;
  // 叙述 = 混合段落 + 短段落（短段落往往是过渡/叙述）
  const narrative = (mixedCount + shortCount) / total;
  // 情感嵌入在描写中估算
  const emotion = description * 0.3; // 简化估算

  let dominant: ContentDistribution['dominant'] = 'narrative';
  const maxVal = Math.max(dialogue, description, narrative);
  if (maxVal === dialogue) dominant = 'dialogue';
  else if (maxVal === description) dominant = 'description';

  return {
    dialogue,
    description,
    narrative,
    emotion,
    dominant,
  };
}

// ==================== 主入口 ====================

/**
 * 分析文本内容，返回完整的感知结果
 * @param htmlContent TipTap 编辑器的 HTML 内容
 * @returns PerceptionResult
 */
export function analyzeText(htmlContent: string): PerceptionResult {
  const text = htmlToText(htmlContent);
  const paragraphs = splitParagraphs(text);

  const paragraphAnalyses = paragraphs.map(analyzeParagraph);
  const sentencePattern = analyzeSentencePattern(paragraphs);
  const vocabulary = analyzeVocabulary(paragraphs);
  const pacing = analyzePacing(paragraphs);
  const contentDistribution = analyzeContentDistribution(paragraphs);

  return {
    totalChars: text.length,
    paragraphs: paragraphAnalyses,
    sentencePattern,
    vocabulary,
    pacing,
    contentDistribution,
    analyzedAt: Date.now(),
  };
}

/**
 * 增量分析：只分析最近添加/修改的部分
 * 用于性能优化，避免每次全量分析
 */
export function analyzeRecent(
  htmlContent: string,
  lastAnalyzedText: string
): PerceptionResult | null {
  const text = htmlToText(htmlContent);
  if (text === lastAnalyzedText) return null;

  // If text is significantly longer and starts with the previous text,
  // only analyze the new suffix for performance
  if (text.length > lastAnalyzedText.length && text.startsWith(lastAnalyzedText)) {
    const newSuffix = text.slice(lastAnalyzedText.length);
    if (newSuffix.length < 50) {
      // Too small to bother with incremental analysis
      return null;
    }
    // Analyze only the new content and merge with cached result
    // For now, fall back to full analysis to ensure correctness
  }

  return analyzeText(htmlContent);
}

/**
 * 判断当前文本是否有足够内容进行分析
 */
export function hasEnoughContent(htmlContent: string): boolean {
  const text = htmlToText(htmlContent);
  return text.length >= 20; // 至少 20 字就开始分析，降低门槛
}
