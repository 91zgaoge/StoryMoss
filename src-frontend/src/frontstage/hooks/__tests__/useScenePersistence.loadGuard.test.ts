import { describe, it, expect } from 'vitest';
import { isEmptyEditorHtml } from '../useScenePersistence';

/**
 * v0.59.2 载入期空写保护的判定基元。
 *
 * 背景：ProseMirror 空文档序列化为 `<p></p>`（真值字符串），早先的
 * `if (!content) return` 挡不住它 → 正文到达前编辑器自带的空文档会被
 * 2s 防抖保存落库，静默清空整章（e2e/frontstage-editing 曾稳定复现）。
 */
describe('isEmptyEditorHtml（视觉空文档判定）', () => {
  it('空字符串与纯空白视为空', () => {
    expect(isEmptyEditorHtml('')).toBe(true);
    expect(isEmptyEditorHtml('   ')).toBe(true);
    expect(isEmptyEditorHtml('\n\t')).toBe(true);
  });

  it('ProseMirror 空文档的各种序列化视为空', () => {
    expect(isEmptyEditorHtml('<p></p>')).toBe(true);
    expect(isEmptyEditorHtml('<p><br></p>')).toBe(true);
    expect(isEmptyEditorHtml('<p><br class="ProseMirror-trailingBreak"></p>')).toBe(true);
    expect(isEmptyEditorHtml('<p>&nbsp;</p>')).toBe(true);
    expect(isEmptyEditorHtml('<p></p><p></p>')).toBe(true);
  });

  it('有正文时不视为空', () => {
    expect(isEmptyEditorHtml('<p>自动保存测试内容。</p>')).toBe(false);
    expect(isEmptyEditorHtml('<p></p><p>第二段有字</p>')).toBe(false);
    expect(isEmptyEditorHtml('字')).toBe(false);
  });
});
