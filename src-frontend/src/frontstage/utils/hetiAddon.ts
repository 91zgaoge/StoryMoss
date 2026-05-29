/**
 * Heti 排版增强 —— 借鉴 https://github.com/sivan/heti
 *
 * 核心功能：
 * 1. 中西文混排自动空格（借鉴 heti-addon.js 正则逻辑）
 * 2. 标点挤压（借鉴 heti-addon.js 正则逻辑）
 *
 * 实现方式：对 DOM 文本节点进行正则匹配并插入 span 标签
 * 不改变编辑器内容，只影响显示效果
 */

// ==================== 正则定义（直接提取自 heti-addon.js）====================

const CJK =
  '\u2e80-\u2eff\u2f00-\u2fdf\u3040-\u309f\u30a0-\u30fa\u30fc-\u30ff\u3100-\u312f\u3200-\u32ff\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff';
const A = 'A-Za-z\u0080-\u00ff\u0370-\u03ff';
const N = '0-9';
const S = '`~!@#\\$%\\^\u0026\\*\\(\\)-_=\\+\\[\\]{}\\\\\\|;:\'",\u003c.\u003e\\\\/\\?';
const ANS = `${A}${N}${S}`;

// 检测浏览器是否支持 lookbehind
let supportLookBehind = true;
try {
  // eslint-disable-next-line no-new
  new RegExp(`(?<=\\d)\\d`, 'g');
} catch {
  supportLookBehind = false;
}

const REG_CJK_FULL = supportLookBehind
  ? new RegExp(`(?<=[${CJK}])( *[${ANS}]+(?: +[${ANS}]+)* *)(?=[${CJK}])`, 'g')
  : new RegExp(`(?:[${CJK}])( *[${ANS}]+(?: +[${ANS}]+)* *)(?=[${CJK}])`, 'g');

const REG_CJK_START = new RegExp(`([${ANS}]+(?: +[${ANS}]+)* *)(?=[${CJK}])`, 'g');

const REG_CJK_END = supportLookBehind
  ? new RegExp(`(?<=[${CJK}])( *[${ANS}]+(?: +[${ANS}]+)*)`, 'g')
  : new RegExp(`(?:[${CJK}])( *[${ANS}]+(?: +[${ANS}]+)*)`, 'g');

const REG_BD_STOP = `\u3002\uff0e\uff0c\u3001\uff1a\uff1b\uff01\u203c\uff1f\u2047`;
const REG_BD_SEP = `\u00b7\u30fb\u2027`;
const REG_BD_OPEN = `\u300c\u300e\uff08\u300a\u3008\u3010\u3016\u3014\uff3b\uff5b`;
const REG_BD_CLOSE = `\u300d\u300f\uff09\u300b\u3009\u3011\u3017\u3015\uff3d\uff5d`;
const REG_BD_START = `${REG_BD_OPEN}${REG_BD_CLOSE}`;
const REG_BD_END = `${REG_BD_STOP}${REG_BD_OPEN}${REG_BD_CLOSE}`;
const REG_BD_HALF_OPEN = `\u201c\u2018`;
const REG_BD_HALF_CLOSE = `\u201d\u2019`;
const REG_BD_HALF_START = `${REG_BD_HALF_OPEN}${REG_BD_HALF_CLOSE}`;

const REG_PUNCT_HALF = new RegExp(
  `([${REG_BD_STOP}])(?=[${REG_BD_START}])|([${REG_BD_OPEN}])(?=[${REG_BD_OPEN}])|([${REG_BD_CLOSE}])(?=[${REG_BD_END}])`,
  'g'
);

const REG_PUNCT_QUARTER = new RegExp(
  `([${REG_BD_SEP}])(?=[${REG_BD_OPEN}])|([${REG_BD_CLOSE}])(?=[${REG_BD_SEP}])`,
  'g'
);

const REG_PUNCT_QUARTER_2 = new RegExp(
  `([${REG_BD_STOP}])(?=[${REG_BD_HALF_START}])|([${REG_BD_HALF_OPEN}])(?=[${REG_BD_OPEN}])`,
  'g'
);

// 需要跳过的标签
const SKIPPED_TAGS = new Set(['pre', 'code', 'sup', 'sub', 'script', 'style', 'textarea']);

// ==================== 工具函数 ====================

function escapeHtml(text: string): string {
  return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

/** 对单个文本节点应用排版增强 */
function processTextNode(node: Text): void {
  const rawText = node.textContent || '';
  if (!rawText.trim()) return;

  let html = escapeHtml(rawText);
  let hasChange = false;

  // 1. 中西文混排：CJK 包围的 ANS（如「hello世界」→ 「<span>hello</span>世界」）
  html = html.replace(REG_CJK_FULL, match => {
    hasChange = true;
    return `<span class="heti-spacing heti-spacing-start heti-spacing-end">${match.trim()}</span>`;
  });

  // 2. 中西文混排：开头是 ANS，后面是 CJK（如 "Hello世界"）
  html = html.replace(REG_CJK_START, match => {
    hasChange = true;
    return `<span class="heti-spacing heti-spacing-start">${match.trim()}</span>`;
  });

  // 3. 中西文混排：前面是 CJK，后面是 ANS（如 "世界Hello"）
  html = html.replace(REG_CJK_END, match => {
    hasChange = true;
    return `<span class="heti-spacing heti-spacing-end">${match.trim()}</span>`;
  });

  // 4. 标点挤压：half
  html = html.replace(REG_PUNCT_HALF, match => {
    hasChange = true;
    return `<span class="heti-adjacent heti-adjacent-half">${match}</span>`;
  });

  // 5. 标点挤压：quarter
  html = html.replace(REG_PUNCT_QUARTER, match => {
    hasChange = true;
    return `<span class="heti-adjacent heti-adjacent-quarter">${match}</span>`;
  });

  // 6. 标点挤压：quarter（弯引号场景）
  html = html.replace(REG_PUNCT_QUARTER_2, match => {
    hasChange = true;
    return `<span class="heti-adjacent heti-adjacent-quarter">${match}</span>`;
  });

  if (!hasChange) return;

  const parent = node.parentNode;
  if (!parent) return;

  // 使用临时 wrapper 解析 HTML，然后替换原节点
  const wrapper = document.createElement('span');
  wrapper.innerHTML = html;

  const frag = document.createDocumentFragment();
  while (wrapper.firstChild) {
    frag.appendChild(wrapper.firstChild);
  }
  parent.replaceChild(frag, node);
}

// ==================== 核心 API ====================

/**
 * 对指定 DOM 元素应用 heti 排版增强
 *
 * 借鉴 heti-addon.js 的 spacingElement 方法
 * 遍历所有文本节点，插入 spacing 和 adjacent 标签
 */
export function applyHetiEnhancement(root: HTMLElement): void {
  // 收集所有需要处理的文本节点（先收集再处理，避免遍历过程中 DOM 变化导致问题）
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
    acceptNode: node => {
      const parent = node.parentElement;
      if (!parent) return NodeFilter.FILTER_REJECT;

      // 跳过已处理的节点
      if (
        parent.tagName === 'SPAN' &&
        (parent.classList.contains('heti-spacing') || parent.classList.contains('heti-adjacent'))
      ) {
        return NodeFilter.FILTER_REJECT;
      }

      // 跳过特定标签
      if (SKIPPED_TAGS.has(parent.tagName.toLowerCase())) {
        return NodeFilter.FILTER_REJECT;
      }

      // 跳过 heti-skip 类的元素
      if (parent.closest('.heti-skip')) {
        return NodeFilter.FILTER_REJECT;
      }

      return NodeFilter.FILTER_ACCEPT;
    },
  });

  const nodes: Text[] = [];
  let n: Node | null;
  while ((n = walker.nextNode()) !== null) {
    nodes.push(n as Text);
  }

  for (const node of nodes) {
    processTextNode(node);
  }
}

/**
 * 清除已应用的 heti 排版增强
 * 将 heti-spacing 和 heti-adjacent span 替换为纯文本
 */
export function clearHetiEnhancement(root: HTMLElement): void {
  const spans = root.querySelectorAll('.heti-spacing, .heti-adjacent');
  for (const span of Array.from(spans)) {
    const parent = span.parentNode;
    if (!parent) continue;
    parent.replaceChild(document.createTextNode(span.textContent || ''), span);
    // 合并相邻文本节点
    parent.normalize();
  }
}

/**
 * 检查文本是否需要 heti 排版增强
 */
export function shouldApplyHeti(text: string): boolean {
  const cjkRegex = /[\u4e00-\u9fff]/;
  const ansRegex = /[A-Za-z0-9]/;
  return cjkRegex.test(text) && ansRegex.test(text);
}
