export function formatDate(dateString: string): string {
  const date = new Date(dateString);
  return date.toLocaleDateString('zh-CN', {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });
}

export function formatNumber(num: number): string {
  if (num >= 1000) {
    return (num / 1000).toFixed(1) + 'k';
  }
  return num.toString();
}

export function countWords(text: string): number {
  // 中文字符 + 英文单词
  const chineseChars = (text.match(/[\u4e00-\u9fa5]/g) || []).length;
  const englishWords = (text.match(/[a-zA-Z]+/g) || []).length;
  return chineseChars + englishWords;
}

export function truncateText(text: string, maxLength: number): string {
  if (text.length <= maxLength) return text;
  return text.slice(0, maxLength) + '...';
}

// ==================== 中文引号规范化（借鉴 heti _variables.scss）====================

/** 直引号 → 中文弯引号（common 规范：「」『』） */
function normalizeQuotes(text: string): string {
  // 先处理成对引号
  let result = text;
  // 替换 "..." 为 「...」
  result = result.replace(/"([^"]*?)"/g, '「$1」');
  // 替换 '...' 为 『...』
  result = result.replace(/'([^']*?)'/g, '『$1』');
  // 替换 " 为 「（未配对的左双引号）
  result = result.replace(/(^|\s|[\u3002\uff01\uff1f.!?])"/g, '$1「');
  // 替换 " 为 」（未配对的右双引号）
  result = result.replace(/"($|\s|[\u3002\uff01\uff1f.!?])/g, '」$1');
  // 替换 ' 为 『（未配对的左单引号）
  result = result.replace(/(^|\s|[\u3002\uff01\uff1f.!?])'/g, '$1『');
  // 替换 ' 为 』（未配对的右单引号）
  result = result.replace(/'($|\s|[\u3002\uff01\uff1f.!?])/g, '』$1');
  return result;
}

// ==================== 自动排版：智能分段（借鉴 heti 排版理念）====================

/**
 * 自动排版：将连续的长文本智能分段为 HTML
 *
 * 设计原则（借鉴 heti）：
 * 1. 贴合网格排版 —— 段落长度控制在 2~4 个完整句子，避免过长过短
 * 2. 对话独立成段 —— 以引号开头的句子优先独立成段
 * 3. 中文引号规范化 —— 统一使用「」『』
 * 4. 保留已有 HTML 结构 —— 如果输入已有 <p> 标签则保留
 * 5. 输出标准 HTML —— 以 <p> 标签包裹每段
 */
export function autoFormatText(input: string): string {
  if (!input || !input.trim()) return '';

  // 1. 如果已经是格式良好的 HTML（有 <p> 标签且数量 >= 2），只规范化引号后保留
  const pTagMatches = input.match(/<p[\s>]/gi);
  if (pTagMatches && pTagMatches.length >= 2) {
    // 提取纯文本，规范化引号，然后重新包装
    let text = input.replace(/<br\s*\/?>/gi, '\n');
    text = text.replace(/<[^>]+>/g, '');
    text = normalizeQuotes(text);
    // 已有段落结构，不需要重新分段，但替换回原有结构
    return input.replace(/(?!<)[^\u003c]+(?=<)/g, match => {
      return normalizeQuotes(match);
    });
  }

  // 2. 去除现有的 HTML 标签，提取纯文本
  let text = input.replace(/<br\s*\/?>/gi, '\n');
  text = text.replace(/<[^>]+>/g, '');
  text = text.trim();

  if (!text) return '';

  // 3. 引号规范化
  text = normalizeQuotes(text);

  // 4. 按 \n\n 空行拆分（LLM 有时会用空行分段）
  const rawParagraphs = text
    .split(/\n\n+/)
    .map(s => s.trim())
    .filter(s => s.length > 0);
  if (rawParagraphs.length >= 2) {
    return rawParagraphs.map(p => `<p>${escapeHtml(p)}</p>`).join('');
  }

  // 5. 智能句子拆分（纯文本，无空行分隔）
  const sentences = splitChineseSentences(text);
  const paragraphs: string[] = [];
  let currentPara = '';
  let sentenceCountInPara = 0;

  for (let i = 0; i < sentences.length; i++) {
    const sentence = sentences[i];
    const nextSentence = sentences[i + 1] || '';

    // 对话检测：以引号/书名号/括号开头的句子优先独立成段
    const isDialogue = /^[\u201c\u2018\u300c\u300e\uff08\u300a"\'「『（《].*/.test(sentence.trim());
    const nextIsDialogue = /^[\u201c\u2018\u300c\u300e\uff08\u300a"\'「『（《].*/.test(
      nextSentence.trim()
    );

    const currentLen = currentPara.length;
    const sentenceLen = sentence.length;
    let shouldBreak = false;

    if (currentPara) {
      if (isDialogue && currentLen > 20) {
        // 对话前断开（如果前面有内容）
        shouldBreak = true;
      } else if (currentLen + sentenceLen > 220) {
        // 段落过长，强制断开（借鉴 heti：单段不宜过长）
        shouldBreak = true;
      } else if (currentLen >= 60 && /[\u3002\uff01\uff1f.!?]/.test(sentence.slice(-1))) {
        // 长度适中且句子完整，可以断开
        shouldBreak = true;
      } else if (!isDialogue && nextIsDialogue && currentLen > 20) {
        // 下一句是对话，当前不是对话，提前断开
        shouldBreak = true;
      } else if (currentLen >= 40 && sentenceCountInPara >= 4) {
        // 已有4个完整句子且超过40字，允许断开
        shouldBreak = true;
      }
    }

    if (shouldBreak && currentPara) {
      paragraphs.push(currentPara.trim());
      currentPara = sentence;
      sentenceCountInPara = 1;
    } else {
      currentPara += sentence;
      sentenceCountInPara++;
    }
  }

  // 处理最后一段
  if (currentPara.trim()) {
    paragraphs.push(currentPara.trim());
  }

  // 6. 后处理：合并过短段落（<15 字）到相邻段
  const merged: string[] = [];
  for (let i = 0; i < paragraphs.length; i++) {
    const para = paragraphs[i];
    if (para.length < 15 && merged.length > 0) {
      merged[merged.length - 1] += para;
    } else if (para.length < 15 && i < paragraphs.length - 1) {
      // 首段过短，合并到下一段
      paragraphs[i + 1] = para + paragraphs[i + 1];
    } else {
      merged.push(para);
    }
  }

  if (merged.length === 0) return '';
  return merged.map(p => `<p>${escapeHtml(p)}</p>`).join('');
}

/** 按中文句子边界拆分文本 */
function splitChineseSentences(text: string): string[] {
  // 匹配以句子结束标点结尾的片段（包含中英文标点）
  const regex = /[^\u3002\uff01\uff1f.!?]*[\u3002\uff01\uff1f.!?]+/g;
  const matches: string[] = [];
  let m: RegExpExecArray | null;
  while ((m = regex.exec(text)) !== null) {
    matches.push(m[0]);
  }
  // 处理末尾没有标点的残留文本
  const lastEnd = regex.lastIndex || 0;
  if (lastEnd < text.length) {
    const tail = text.slice(lastEnd).trim();
    if (tail) matches.push(tail);
  }
  return matches.length > 0 ? matches : [text];
}

/** 转义 HTML 特殊字符 */
function escapeHtml(text: string): string {
  return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}
