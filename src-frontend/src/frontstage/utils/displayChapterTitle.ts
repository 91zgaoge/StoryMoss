/**
 * 幕前章节标题展示规则。
 * - 空/空白 → 「第N章」
 * - 否则 → trim 后的真实标题
 */
export function displayChapterTitle(
  chapter: { title?: string | null; chapter_number: number } | null | undefined
): string {
  if (!chapter) return '';
  const t = (chapter.title ?? '').trim();
  if (!t) return `第${chapter.chapter_number}章`;
  return t;
}
