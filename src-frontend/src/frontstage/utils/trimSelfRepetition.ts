import { normalizeForDuplicateCheck } from './isTextDuplicate';

/**
 * v0.26.14: 清理生成内容自身的重复。
 *
 * 某些模型在生成长文本时会输出“首尾重复”或“前后两段几乎相同”的内容
 * （例如 Genesis 第一章结尾把开头段落又写了一遍）。这类重复不是前端追加
 * 两次造成的，因此传统的“编辑器是否已包含该内容”检测无法拦截。
 *
 * 本函数在把内容写入编辑器前执行一次自重复裁剪：
 * 1. 先按段落检测“后半段 == 前半段”或“末段 == 首段”的明显重复；
 * 2. 再对整段文本做 KMP 最长 border 检测，若尾部与开头大量重合，则裁掉尾部。
 *
 * 阈值比较保守，只处理明显模型故障（重复长度 >= 30 字符且占全文 >= 8%），
 * 避免误伤正常文学修辞中的首尾呼应。
 */
export function trimSelfRepetition(text: string): string {
  if (!text || text.length < 40) return text;

  const trimmed = text.trim();
  if (trimmed.length < 40) return text;

  // 1) 段落级：先尝试按 \n\n 分割，处理最直观的情况
  const paragraphDeduped = trimRepeatedParagraphs(trimmed);
  if (paragraphDeduped.length < trimmed.length) {
    return paragraphDeduped;
  }

  // 2) 字符级 border：处理段落内或跨段的长尾重复
  return trimByLongestBorder(trimmed);
}

function normalizeParagraph(s: string): string {
  return normalizeForDuplicateCheck(s);
}

function trimRepeatedParagraphs(text: string): string {
  const paragraphs = text
    .split(/\n\n+/)
    .map(p => p.trim())
    .filter(Boolean);
  if (paragraphs.length < 2) return text;

  const normalizedParagraphs = paragraphs.map(normalizeParagraph);

  // 情况 A：后半部分整体重复前半部分（模型把整章写了两遍）
  if (normalizedParagraphs.length % 2 === 0) {
    const half = normalizedParagraphs.length / 2;
    const firstHalf = normalizedParagraphs.slice(0, half);
    const secondHalf = normalizedParagraphs.slice(half);
    if (firstHalf.every((p, i) => p === secondHalf[i])) {
      return paragraphs.slice(0, half).join('\n\n');
    }
  }

  // 情况 B：末段与首段相同（截图里的“首尾段落重复”）
  while (
    normalizedParagraphs.length > 1 &&
    normalizedParagraphs[normalizedParagraphs.length - 1] === normalizedParagraphs[0]
  ) {
    paragraphs.pop();
    normalizedParagraphs.pop();
  }

  if (paragraphs.length < text.split(/\n\n+/).filter(Boolean).length) {
    return paragraphs.join('\n\n');
  }

  return text;
}

/**
 * 用 KMP 计算 normalized 文本的最长 border（既是前缀也是后缀的真子串）。
 * 如果 border 足够长，就在原字符串中把尾部的重复部分裁掉。
 */
function trimByLongestBorder(text: string): string {
  const { normalized, indices } = buildNormalizedIndex(text);
  if (normalized.length < 40) return text;

  const borderLen = longestBorderLength(normalized);
  if (borderLen <= 0) return text;

  const minBorder = Math.max(30, Math.floor(normalized.length * 0.08));
  if (borderLen < minBorder) return text;

  // 保留的部分至少要有 30 个有效字符，避免把短文裁成碎片
  const remaining = normalized.length - borderLen;
  if (remaining < 30) return text;

  const cutIndex = indices[normalized.length - borderLen];
  if (cutIndex == null || cutIndex <= 0) return text;

  let result = text.slice(0, cutIndex).trim();

  // 如果裁掉后末尾是不完整的 HTML 标签，进一步清理到标签开始
  const lastOpen = result.lastIndexOf('<');
  const lastClose = result.lastIndexOf('>');
  if (lastOpen > lastClose) {
    result = result.slice(0, lastOpen).trim();
  }

  return result || text;
}

/**
 * 返回归一化字符串，以及每个归一化字符在原字符串中的索引位置。
 * 归一化规则与 isTextDuplicate 保持一致：去 HTML 标签、去空白、去标点。
 */
function buildNormalizedIndex(text: string): { normalized: string; indices: number[] } {
  const normalized: string[] = [];
  const indices: number[] = [];

  const punctPattern =
    /[\u3002\uff01\uff1f.!?，、；：""''（）《》\[\]【】…—～·\u201c\u201d\u2018\u2019]/;

  for (let i = 0; i < text.length; i++) {
    const ch = text[i];

    if (ch === '<') {
      // 跳过完整 HTML 标签
      const close = text.indexOf('>', i);
      if (close === -1) break;
      i = close;
      continue;
    }

    if (/\s/.test(ch) || punctPattern.test(ch)) {
      continue;
    }

    normalized.push(ch);
    indices.push(i);
  }

  return { normalized: normalized.join(''), indices };
}

function longestBorderLength(s: string): number {
  const n = s.length;
  const pi = new Array(n).fill(0);
  for (let i = 1; i < n; i++) {
    let j = pi[i - 1];
    while (j > 0 && s[i] !== s[j]) {
      j = pi[j - 1];
    }
    if (s[i] === s[j]) {
      j++;
    }
    pi[i] = j;
  }
  return pi[n - 1];
}
