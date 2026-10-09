/**
 * v0.26.15: 通用文本清理工具。
 *
 * 用于检测/裁剪模型生成内容中常见的自重复模式（首尾段落重复、
 * 后半部分重复前半部分、段落内长尾重复等）。
 *
 * 这些工具原本位于 frontstage/utils，现提升到 src/utils，供前端渲染层、
 * 自动排版、后端保存前统一调用。
 */

/** 归一化文本用于去重比较：去 HTML 标签、去空白、去标点。 */
export const normalizeForDuplicateCheck = (s: string): string => {
  return s
    .replace(/<[^>]*>/g, '')
    .replace(/\s+/g, '')
    .replace(
      /[\u3002\uff01\uff1f.!?，、；：""''（）《》\[\]【】…—～·\u201c\u201d\u2018\u2019]/g,
      ''
    );
};

/** 判断 generatedText 是否已经包含在 existingText 中。 */
export const isTextDuplicate = (existingText: string, generatedText: string): boolean => {
  const trimmedExisting = existingText.trim();
  const trimmedGenerated = generatedText.trim();
  if (!trimmedExisting || !trimmedGenerated) return false;

  const normalizedExisting = normalizeForDuplicateCheck(trimmedExisting);
  const normalizedGenerated = normalizeForDuplicateCheck(trimmedGenerated);
  if (!normalizedExisting || !normalizedGenerated) return false;

  // v0.30.41: 最小长度守卫--归一化后 < 30 字符的生成文本几乎一定出现在
  // 长篇已有正文中（如"续写"2 字在 9656 字小说中必然命中），导致假阳性
  // 去重判定，静默丢弃生成内容。打字机动画首帧仅 3 字符，尤为脆弱。
  // 只有生成文本足够长（>= 30 归一化字符）时才进行去重检查。
  const MIN_DUPLICATE_CHECK_LEN = 30;
  if (normalizedGenerated.length < MIN_DUPLICATE_CHECK_LEN) return false;

  const fingerprintLen = Math.min(500, normalizedGenerated.length);
  const generatedFingerprint = normalizedGenerated.slice(0, fingerprintLen);

  return (
    normalizedExisting.includes(generatedFingerprint) ||
    (normalizedExisting.length >= normalizedGenerated.length * 0.9 &&
      normalizedGenerated.includes(
        normalizedExisting.slice(0, Math.min(200, normalizedExisting.length))
      ))
  );
};

/**
 * v0.26.15: 清理生成内容自身的重复。
 *
 * 某些模型在生成长文本时会输出“首尾重复”或“前后两段几乎相同”的内容
 * （例如 Genesis 第一章结尾把开头段落又写了一遍）。这类重复不是前端追加
 * 两次造成的，因此传统的“编辑器是否已包含该内容”检测无法拦截。
 *
 * 本函数在把内容写入编辑器/数据库前执行一次自重复裁剪：
 * 1. 先按段落检测末尾连续 k 段是否重复开头连续 k 段；
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

  // 2) v0.26.24: 散布式句子块重复——同一句子或多句块在文中以不同上下文
  //    出现 ≥2 次。续写时模型陷入意象循环（冥界/牢笼/苦楚）的典型症状。
  const interspersedDeduped = trimInterspersedRepeatedBlocks(trimmed);
  if (interspersedDeduped.length < trimmed.length) {
    return interspersedDeduped;
  }

  // 3) 字符级 border：处理段落内或跨段的长尾重复
  return trimByLongestBorder(trimmed);
}

/**
 * v0.26.24: 续写跨内容重叠剥离。
 *
 * 续写时 Writer 可能把已有正文中的段落重新输出（非「全文前缀」匹配），
 * `startsWith(currentText)` 无法拦截。找 generated 归一化文本的最长前缀 L，
 * 使其是 existing 尾部 3000 字的子串；L ≥ 25 归一化字时在原文中截断。
 *
 * 与 Rust `TextUtils::strip_existing_overlap` 对齐。
 */
export function stripExistingOverlap(generated: string, existing: string): string {
  const genTrimmed = generated.trim();
  const existingTrimmed = existing.trim();
  if (!genTrimmed || !existingTrimmed) return generated;

  const existingTail =
    existingTrimmed.length > 3000 ? [...existingTrimmed].slice(-3000).join('') : existingTrimmed;

  const { normalized: normGen, indices: genIdx } = buildNormalizedIndex(genTrimmed);
  const { normalized: normExisting } = buildNormalizedIndex(existingTail);
  if (!normGen || !normExisting) return generated;

  const MIN_OVERLAP = 25;
  const upper = Math.min(normGen.length, normExisting.length);
  let best = 0;
  for (let l = upper; l >= MIN_OVERLAP; l--) {
    if (normExisting.includes(normGen.slice(0, l))) {
      best = l;
      break;
    }
  }
  if (best < MIN_OVERLAP) return generated;

  const cutIndex = genIdx[best] ?? genTrimmed.length;
  const remaining = genTrimmed.slice(cutIndex).trimStart();
  if ([...remaining].length < 10) return generated;
  return remaining;
}

/**
 * v0.26.24: 裁掉 token/超时截断留下的极短末句（归一化 < 12 字且全文 ≥ 2 句）。
 * 与 Rust `TextUtils::trim_dangling_tail` 对齐。
 */
export function trimDanglingTail(text: string): string {
  const trimmed = text.trim();
  if (!trimmed) return text;

  const sentences = splitSentencesWithDelimiters(trimmed);
  if (sentences.length < 2) return text;

  const lastNorm = normalizeForDuplicateCheck(sentences[sentences.length - 1].body);
  if (lastNorm.length >= 12) return text;

  let result = '';
  for (let i = 0; i < sentences.length - 1; i++) {
    result += sentences[i].raw;
  }
  result = result.trim();
  return result || text;
}

/**
 * v0.26.24: 续写后处理管线--自重复清理 + 跨内容重叠剥离 + 截断末句裁剪。
 */
export function sanitizeContinuationOutput(generated: string, existing: string): string {
  let result = trimSelfRepetition(generated);
  result = stripExistingOverlap(result, existing);
  result = trimDanglingTail(result);
  return result;
}

/**
 * v0.30.41: 剥离模型回显的用户指令前缀。
 *
 * 某些模型（如 deepseek-v4）会在生成内容开头回显用户的写作指令
 * （如用户输入"续写"，模型响应以"续写\n黑暗。..."开头）。
 * 这不是正文内容，而是指令回显，应当剥离。
 *
 * 剥离策略：归一化比较生成文本开头与用户指令，若开头匹配则裁掉
 * 原始文本中对应长度的前缀（含紧随的换行/冒号等分隔符）。
 *
 * @param generated 模型生成的文本
 * @param userInput 用户的原始输入指令
 * @returns 剥离指令回显后的文本
 */
export function stripInstructionEcho(generated: string, userInput: string): string {
  const trimmedInput = userInput.trim();
  if (!trimmedInput || trimmedInput.length < 2) return generated;

  const normInput = normalizeForDuplicateCheck(trimmedInput);
  if (!normInput || normInput.length < 2) return generated;

  // 检查生成文本开头（取指令长度的 3 倍作为检测窗口，覆盖分隔符）是否匹配指令
  const detectionWindow = generated.slice(0, Math.max(trimmedInput.length * 3, 50));
  const normWindow = normalizeForDuplicateCheck(detectionWindow);
  if (!normWindow.startsWith(normInput)) return generated;

  // 在原始文本中定位指令结束位置：找到 trimmedInput 第一次出现的位置，
  // 裁掉它及其后的前导分隔符（换行、冒号、空格等）
  const echoIndex = generated.indexOf(trimmedInput);
  if (echoIndex !== 0) return generated;

  let cutEnd = trimmedInput.length;
  // 跳过紧随的分隔符：\n : ： 、 ，, 等
  while (cutEnd < generated.length) {
    const ch = generated[cutEnd];
    if (
      ch === '\n' ||
      ch === '\r' ||
      ch === ':' ||
      ch === '：' ||
      ch === '、' ||
      ch === ',' ||
      ch === '，' ||
      ch === ' ' ||
      ch === '\t' ||
      ch === '。'
    ) {
      cutEnd++;
    } else {
      break;
    }
  }

  const remaining = generated.slice(cutEnd);
  // 剥离后剩余内容过短则保留原文（避免误剥导致内容丢失）
  if ([...remaining.trim()].length < 10) return generated;
  return remaining;
}

/**
 * v0.65.3: 切章时判定「未确认的幽灵续文是否已随新章正文呈现」。
 *
 * 幽灵文本（generatedText）是尚未 Tab 确认的 AI 续写。Agency 续写在生成时即已
 * 落库，自动分章会把溢出正文搬进新章，此时幽灵内容已在新章正文里，切章丢弃它
 * 不算丢正文；但若新章正文既不含幽灵开头（幽灵整体/前缀已是正文片段），也不含
 * 幽灵结尾（幽灵 = 旧文尾部重演 + 新续写的常见形态），说明幽灵里有新章没有的
 * 文字，切章时必须保留。
 *
 * @returns true 表示幽灵内容已在新章正文中（可安全丢弃）；false 表示需保留
 */
export function isGhostDeliveredInContent(ghost: string, content: string): boolean {
  const trimmedGhost = ghost.trim();
  const trimmedContent = content.trim();
  if (!trimmedGhost || !trimmedContent) return false;

  // 幽灵整体（或前缀指纹）已在新章正文中——覆盖幽灵是新章正文子串的情况
  if (isTextDuplicate(trimmedContent, trimmedGhost)) return true;

  // 幽灵结尾已在新章正文中——覆盖「重演旧文尾部 + 新续写」的分章形态
  const normGhost = normalizeForDuplicateCheck(trimmedGhost);
  const normContent = normalizeForDuplicateCheck(trimmedContent);
  const tailLen = Math.min(120, normGhost.length);
  // 尾部过短无法判断时按「未呈现」处理：宁可多保留一个可见幽灵，也不静默丢字
  // （accept 路径自带 stripExistingPrefix + 去重，误保留不会造成正文重复）
  if (tailLen < 24) return false;
  return normContent.includes(normGhost.slice(-tailLen));
}

/**
 * v0.26.24: 检测并裁剪散布式句子块重复。
 *
 * 把文本按句末标点（。！？.?!）切成句子序列，归一化后查找在文中出现 ≥2 次
 * 的句子或多句块，保留首次出现，裁掉后续重复。只处理归一化后 ≥ 15 字符
 * 的块，避免误伤首尾呼应等良性短句重复。
 *
 * 与 Rust `TextUtils::trim_interspersed_repeated_blocks` 对齐，跨层 golden
 * 双跑锁定一致性。
 */
function trimInterspersedRepeatedBlocks(text: string): string {
  const sentences = splitSentencesWithDelimiters(text);
  if (sentences.length < 3) return text;
  // 超长输入降级（O(n²) 代价控制）：>300 句交给 border 兜底
  if (sentences.length > 300) return text;

  const normalized = sentences.map(s => normalizeForDuplicateCheck(s.body));
  const n = sentences.length;
  const removed: boolean[] = new Array(n).fill(false);
  const MIN_BLOCK_CHARS = 15;

  for (let i = 0; i < n; i++) {
    if (removed[i]) continue;
    for (let j = i + 1; j < n; j++) {
      if (removed[j]) continue;
      // 计算从 i 与 j 起的最长公共句子块长度 L（完整扩展）
      let l = 0;
      while (
        i + l < n &&
        j + l < n &&
        !removed[i + l] &&
        !removed[j + l] &&
        normalized[i + l] === normalized[j + l]
      ) {
        l++;
      }
      if (l === 0) continue;
      // 块归一化字符数须 ≥ 15
      let totalBlockChars = 0;
      for (let k = 0; k < l; k++) totalBlockChars += normalized[i + k].length;
      if (totalBlockChars < MIN_BLOCK_CHARS) continue;
      // 标记后续出现的整块为重复
      for (let k = 0; k < l; k++) removed[j + k] = true;
    }
  }

  if (!removed.some(r => r)) return text;

  let result = '';
  for (let i = 0; i < n; i++) {
    if (!removed[i]) result += sentences[i].raw;
  }
  result = result.trim();
  return result || text;
}

interface SentenceSegment {
  /** 原始片段（含句末标点与相邻空白/换行），用于重建 */
  raw: string;
  /** 去掉首尾空白的句子正文，用于归一化比较 */
  body: string;
}

function splitSentencesWithDelimiters(text: string): SentenceSegment[] {
  const segments: SentenceSegment[] = [];
  const delimiters = new Set(['。', '？', '！', '.', '?', '!']);
  let start = 0;
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (delimiters.has(ch)) {
      const end = i + 1;
      const raw = text.slice(start, end);
      const body = raw.trim();
      if (body) segments.push({ raw, body });
      start = end;
    }
  }
  if (start < text.length) {
    const raw = text.slice(start);
    const body = raw.trim();
    if (body) segments.push({ raw, body });
  }
  return segments;
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

  // 情况 B：末尾连续 k 个段落重复开头连续 k 个段落（k 不限于 1）
  // 例如 P1 P2 P3 P4 P1 P2 P3，应裁掉末尾 P1 P2 P3
  const maxK = Math.floor(normalizedParagraphs.length / 2);
  for (let k = maxK; k >= 1; k--) {
    const start = normalizedParagraphs.slice(0, k);
    const end = normalizedParagraphs.slice(-k);
    if (start.every((p, i) => p === end[i])) {
      const remaining = paragraphs.slice(0, paragraphs.length - k);
      if (remaining.length >= 1) {
        return remaining.join('\n\n');
      }
    }
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
