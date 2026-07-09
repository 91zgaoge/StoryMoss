/** 占位/草稿标题：空、品牌名「草苔」、或「未命名」 */
export function isPlaceholderTitle(title: string | null | undefined): boolean {
  const t = (title ?? '').trim();
  return t === '' || t === '草苔' || t === '未命名';
}

/**
 * 幕前顶部故事名展示规则。
 * - 无故事且无正文 → 「草苔」（品牌占位）
 * - 有正文且标题为空/「草苔」→ 「未命名」
 * - 否则 → 真实标题
 */
export function displayStoryTitle(
  story: { title: string; id?: string } | null,
  hasBodyContent: boolean
): string {
  if (!story) return hasBodyContent ? '未命名' : '草苔';
  const t = story.title.trim();
  if (!t || t === '草苔') return hasBodyContent ? '未命名' : '草苔';
  return t;
}
