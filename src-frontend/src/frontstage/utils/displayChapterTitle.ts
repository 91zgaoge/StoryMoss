/**
 * 幕前章节标题展示规则。
 * - 空/空白 → 「第N章」
 * - 阿拉伯数字占位标题（「第2章」「第12章」）→ 跟当前 chapter_number
 *   （自动分章重排后库里的标题会落后，不能照显示）
 * - 中文数字标题（「第一章」「第八章」）→ 原样显示。系统占位标题一律是
 *   阿拉伯数字（canonical_chapter_title），中文数字只会来自作者手写；旧库
 *   的占位中文数字已由 V130 迁移成阿拉伯数字。
 * - 否则 → trim 后的真实标题
 */
const ARABIC_CHAPTER_TITLE = /^第[0-9]+章$/;

export function displayChapterTitle(
  chapter: { title?: string | null; chapter_number: number } | null | undefined
): string {
  if (!chapter) return '';
  const t = (chapter.title ?? '').trim();
  if (!t || ARABIC_CHAPTER_TITLE.test(t)) {
    return `第${chapter.chapter_number}章`;
  }
  return t;
}
