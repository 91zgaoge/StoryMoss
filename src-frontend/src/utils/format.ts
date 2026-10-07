import { trimSelfRepetition } from './textCleanup';

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

// ==================== 悬挂闭合标点合并 ====================

/**
 * 纯文本级：删除「换行后紧跟闭合标点」的换行，把闭合标点并回上一行。
 *
 * 根因：LLM 生成长对话时软换行，闭合引号（" ' 」 』 等）单独占一行，
 * 后续按 \n 分段会产生只含闭合引号的孤段落。
 * 只合并闭合向字符：" ' ’ ” 」 』 ） 】 》 〉 ] }；
 * 开向字符（「 『 （ 【 《 〈 [ {）不合并。
 * 边界：上一行不存在（文本以换行+闭合标点开头）时不合并。
 * 已知限制：直引号 " ' 无方向信息，刻意以闭合引号开场的段落或
 * 跨段引语（每段以 " 开场）会被误并——中文小说语料下罕见，属启发式取舍。
 */
export function mergeHangingClosingPunct(text: string): string {
  if (!text) return text;
  // 跳过换行与下引号之间的空白/全角缩进/零宽字符，否则
  // 「禁军。\n\n　　”」会落成带段首缩进的孤 <p>”</p>
  return text.replace(/([^\n])(?:\n+[\s\u3000\u200b]*)+(?=["'’”」』）】》〉\]}])/g, '$1');
}

/** 闭合标点的 HTML 实体形态（含数据里误编码的开向实体 &ldquo;/&lsquo;） */
const CLOSING_PUNCT_ENTITY =
  '&(?:rdquo|ldquo|quot|apos|rsquo|lsquo|#x201[dD]|#x2019|#8221|#8217|#34|#39);';
const CLOSING_PUNCT_CHAR = '["\'’”」』）】》〉\\]\\}]';
/** 孤闭合标字段内允许出现的空白 token（&nbsp; 只算空白，不算标点） */
const LONE_WS = '(?:\\s|&nbsp;|\u3000|\u200b)';
/** 孤闭合标字段内允许出现的 token：闭合标点（字符或实体）或空白 */
const LONE_TOKEN = `(?:${CLOSING_PUNCT_ENTITY}|${CLOSING_PUNCT_CHAR}|${LONE_WS})`;
const LONE_PUNCT = `(?:${CLOSING_PUNCT_ENTITY}|${CLOSING_PUNCT_CHAR})`;
/** 整段内容仅为闭合标点+空白（至少一个标点）的 <p>，且前面存在可并入的段落 */
const LONE_CLOSING_PARA_RE = new RegExp(
  `</p>\\s*<p>(${LONE_TOKEN}*${LONE_PUNCT}${LONE_TOKEN}*)</p>`,
  'g'
);

/**
 * 段首修复只认**有方向的**收尾符：`"` `'` 无方向信息，可能是开引号（英文式
 * 对话每段以 " 开场），保持原样。
 */
const DIRECTIONAL_CLOSING = '(?:[”’」』）】》〉]|&(?:rdquo|rsquo|#x201[dD]|#x2019|#8221|#8217);)';
/** 段首为闭合标点（可跟空白）的 <p>：`</p><p>”正文…</p>` */
const LEADING_CLOSING_PARA_RE = new RegExp(
  `</p>\\s*<p>(${DIRECTIONAL_CLOSING}+)(${LONE_WS}*)([\\s\\S]*?)</p>`,
  'g'
);

/**
 * HTML 级：把「整段内容仅为闭合标点+空白」的 <p> 并入上一段。
 * 覆盖段落内是字符或 HTML 实体（&rdquo; &quot; &#x201D; &#8221; 等）与首尾空白；
 * 只有当前面存在可并入的段落（</p>）时才合并。
 * 对无 <p> 的输入原样返回。
 */
export function mergeLoneClosingPunctParagraphs(html: string): string {
  if (!html || !html.includes('<p>')) return html;
  let result = html;
  const stripWs = new RegExp(`(?:${LONE_WS})+`, 'g');
  // 连续多个孤闭合标字段需要循环到不动点（每次替换会产生新的 </p><p> 边界）
  for (;;) {
    const next = result.replace(LONE_CLOSING_PARA_RE, (_m, inner: string) => {
      return `${inner.replace(stripWs, '')}</p>`;
    });
    if (next === result) return result;
    result = next;
  }
}

/**
 * HTML 级：把**段首**的闭合标点并回上一段末尾。
 *
 * 根因：句子切分在句末标点（。！？）处断开，把紧随其后的收尾引号切给了下一句；
 * 段落组装若正好在那一处断段，就落成 `<p>”\n正文…</p>`——孤引号独占一行
 * （真机《帝国的烟火》第 2 章即此形态，V128 的「整段仅闭合标点」规则覆盖不到，
 * 因为引号后面还跟着正文）。
 *
 * 并回后：引号进入上一段末尾，正文去掉其后的空白/换行留在本段；整段仅剩引号时
 * 直接并段。段首只认出有方向的收尾符（见 `DIRECTIONAL_CLOSING`）。
 */
export function mergeLeadingClosingPunctParagraphs(html: string): string {
  if (!html || !html.includes('</p>')) return html;
  let result = html;
  for (;;) {
    const next = result.replace(
      LEADING_CLOSING_PARA_RE,
      (_m, punct: string, _ws: string, body: string) => {
        return body.trim().length > 0 ? `${punct}</p><p>${body}</p>` : `${punct}</p>`;
      }
    );
    if (next === result) return result;
    result = next;
  }
}

/** 段落级收尾修复：孤闭合标字段 + 段首闭合标点两条规则。 */
function repairClosingPunctParagraphs(html: string): string {
  return mergeLeadingClosingPunctParagraphs(mergeLoneClosingPunctParagraphs(html));
}

/**
 * 纯文本 → 段落 HTML：先合并悬挂闭合标点，再按 \n+ 切行，非空行各包 <p>。
 * 替换 naive 的 `<p>${text.replace(/\n/g, '</p><p>')}</p>` 写法。
 */
export function textToParagraphsHtml(text: string): string {
  if (!text) return '';
  const merged = mergeHangingClosingPunct(text);
  return repairClosingPunctParagraphs(
    merged
      .split(/\n+/)
      .filter(line => line.trim().length > 0)
      .map(line => `<p>${line}</p>`)
      .join('')
  );
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

  // v0.26.15: 模型可能生成自重复内容（首尾段落重复、后半部分重复前半部分等）。
  // 在自动排版前先清理一次，作为渲染层的兜底防线。
  input = trimSelfRepetition(input);
  if (!input || !input.trim()) return '';

  // 1. 如果已经是格式良好的 HTML（有 <p> 标签且数量 >= 2），只规范化引号后保留
  const pTagMatches = input.match(/<p[\s>]/gi);
  if (pTagMatches && pTagMatches.length >= 2) {
    // 提取纯文本，规范化引号，然后重新包装
    let text = input.replace(/<br\s*\/?>/gi, '\n');
    text = text.replace(/<[^>]+>/g, '');
    text = normalizeQuotes(text);
    // 已有段落结构，不需要重新分段，但替换回原有结构；
    // 顺带修复 DB 存量里的孤闭合标字段（<p>"</p>），并入上一段
    return repairClosingPunctParagraphs(
      input.replace(/(?!<)[^\u003c]+(?=<)/g, match => {
        return normalizeQuotes(match);
      })
    );
  }

  // 2. 去除现有的 HTML 标签，提取纯文本
  let text = input.replace(/<br\s*\/?>/gi, '\n');
  text = text.replace(/<[^>]+>/g, '');
  text = text.trim();

  if (!text) return '';

  // 3. 引号规范化
  text = normalizeQuotes(text);

  // 3.5 悬挂闭合标点（LLM 软换行把闭合引号单独成行）并回上一行，避免产生孤闭合引号段
  text = mergeHangingClosingPunct(text);

  // 4. 按 \n\n 空行拆分（LLM 有时会用空行分段）
  const rawParagraphs = text
    .split(/\n\n+/)
    .map(s => s.trim())
    .filter(s => s.length > 0);
  if (rawParagraphs.length >= 2) {
    return repairClosingPunctParagraphs(rawParagraphs.map(p => `<p>${escapeHtml(p)}</p>`).join(''));
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
  return repairClosingPunctParagraphs(merged.map(p => `<p>${escapeHtml(p)}</p>`).join(''));
}

/** 按中文句子边界拆分文本 */
function splitChineseSentences(text: string): string[] {
  // 匹配以句子结束标点结尾的片段（包含中英文标点）。
  // 句末标点后紧跟的收尾符（” ’ 」 』 ） 】 》 〉 及 ASCII 直引号）属于**本句**：
  // 不带上它们，句末引号会被切到下一句，段落正好在此断开时就成了段首孤引号
  // （真机《帝国的烟火》第 2 章：`…还是你苏家的命。</p><p>”\n大堂内的空气…`）。
  const regex = /[^\u3002\uff01\uff1f.!?]*[\u3002\uff01\uff1f.!?]+["'’”」』）】》〉]*/g;
  const matches: string[] = [];
  let lastEnd = 0;
  let m: RegExpExecArray | null;
  while ((m = regex.exec(text)) !== null) {
    matches.push(m[0]);
    // 在循环内捕获 lastIndex；exec 返回 null 后全局正则的 lastIndex 会被重置为 0，
    // 不能在循环外读取，否则 tail 会变成整段文本导致内容翻倍。
    lastEnd = regex.lastIndex;
  }
  // 处理末尾没有标点的残留文本
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
