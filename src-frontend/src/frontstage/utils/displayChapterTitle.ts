/**
 * 章节标题统一格式（v0.99.8）。
 *
 * 规则：`第<中文数字>章 <标题>`。
 * - 有标题：`第十三章 香炉边的银镯与铜钱`
 * - 只有占位：`第十三章`
 * - 训练沙盒上下文继承的源章号（`第 134 章`）保留 134；
 * - 自动分章后过期的系统占位（`第6章`）跟当前 chapter_number；
 * - 章后紧跟正文的真实标题（`第三章的雨`）整段作为标题，不误拆。
 */

const CN_DIGITS = ['零', '一', '二', '三', '四', '五', '六', '七', '八', '九'];

export function chineseNumeral(n: number): string {
  if (!Number.isFinite(n) || n <= 0) return '零';
  const value = Math.floor(n);
  if (value >= 10000) {
    const high = Math.floor(value / 10000);
    const low = value % 10000;
    let out = `${chineseNumeral(high)}万`;
    if (low > 0) {
      if (low < 1000) out += '零';
      out += chineseNumeral(low);
    }
    return out;
  }
  let out = '';
  let remainder = value;
  let zeroPending = false;
  for (const [unit, label] of [
    [1000, '千'],
    [100, '百'],
    [10, '十'],
  ] as const) {
    const digit = Math.floor(remainder / unit);
    remainder %= unit;
    if (digit > 0) {
      if (zeroPending && out) out += '零';
      if (!(digit === 1 && unit === 10 && !out)) out += CN_DIGITS[digit];
      out += label;
      zeroPending = false;
    } else if (out && remainder > 0) {
      zeroPending = true;
    }
  }
  if (remainder > 0) {
    if (zeroPending) out += '零';
    out += CN_DIGITS[remainder];
  }
  return out || '零';
}

const CN_NUMERAL: Record<string, number> = {
  零: 0,
  〇: 0,
  一: 1,
  二: 2,
  两: 2,
  三: 3,
  四: 4,
  五: 5,
  六: 6,
  七: 7,
  八: 8,
  九: 9,
};

const CN_UNIT: Record<string, number> = {
  十: 10,
  百: 100,
  千: 1000,
  万: 10000,
};

function parseNumberToken(token: string): number | undefined {
  if (!token) return undefined;
  if (/^[0-9]+$/.test(token)) return Number(token);
  let total = 0;
  let section = 0;
  let pending = 0;
  let sawAny = false;
  for (const c of token) {
    if (c in CN_NUMERAL) {
      pending = CN_NUMERAL[c];
      sawAny = true;
      continue;
    }
    const unit = CN_UNIT[c];
    if (unit === undefined) return undefined;
    sawAny = true;
    if (unit === 10000) {
      total += Math.max(1, section + pending) * 10000;
      section = 0;
      pending = 0;
    } else {
      section += (pending || 1) * unit;
      pending = 0;
    }
  }
  return sawAny ? total + section + pending : undefined;
}

type ChapterTitleParts = {
  number?: number;
  /** undefined：章后紧跟正文，整段是真实标题；''：只有章号。 */
  body?: string;
  spaced: boolean;
};

const CHAPTER_TITLE_RE = /^第(\s*[0-9]+|[一二三四五六七八九十百千万零〇两\s]+)(\s*)章(.*)$/s;

function isSeparator(c: string): boolean {
  return /[\s：:·、\-—－)）\]】]/.test(c);
}

function parseChapterTitle(raw: string): ChapterTitleParts {
  const m = raw.trim().match(CHAPTER_TITLE_RE);
  if (!m) return { spaced: false };
  const numberToken = m[1].replace(/\s+/g, '');
  const spaced = /\s/.test(m[1]) || /\s/.test(m[2]);
  const rawRest = `${m[2]}${m[3]}`;
  const body =
    rawRest.trim() === ''
      ? ''
      : isSeparator(rawRest[0])
        ? rawRest
            .slice(1)
            .trimStart()
            .replace(/^[：:·、\-—－)）\]】\s]+/, '')
        : undefined;
  return { number: parseNumberToken(numberToken), body, spaced };
}

/** 统一为「第二章 铜钱与脚印」。 */
export function formatChapterTitle(chapterNumber: number, rawTitle?: string | null): string {
  const n = Number.isFinite(chapterNumber) && chapterNumber > 0 ? Math.floor(chapterNumber) : 1;
  const raw = (rawTitle ?? '').trim();
  if (!raw) return `第${chineseNumeral(n)}章`;
  const parts = parseChapterTitle(raw);
  let displayNumber = n;
  let body = raw;
  if (parts.number !== undefined && parts.body !== undefined) {
    if (parts.body) {
      displayNumber = parts.number;
      body = parts.body;
    } else if (
      parts.spaced ||
      /[一二三四五六七八九十百千万零〇两]/.test(raw.slice(1, raw.indexOf('章')))
    ) {
      // 「第 134 章」/「第一百三十四章」：训练沙盒或作者继承的源章号。
      displayNumber = parts.number;
      body = '';
    } else {
      // 「第12章」：系统占位，跟当前 chapter_number（分章重排后修正过期标题）。
      displayNumber = n;
      body = '';
    }
  }
  return body
    ? `第${chineseNumeral(displayNumber)}章 ${body}`
    : `第${chineseNumeral(displayNumber)}章`;
}

export function displayChapterTitle(
  chapter: { title?: string | null; chapter_number: number } | null | undefined
): string {
  if (!chapter) return '';
  return formatChapterTitle(chapter.chapter_number, chapter.title);
}
