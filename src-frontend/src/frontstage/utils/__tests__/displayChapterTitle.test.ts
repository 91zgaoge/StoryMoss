import { describe, it, expect } from 'vitest';
import { displayChapterTitle } from '../displayChapterTitle';

describe('displayChapterTitle', () => {
  it('null → 空串', () => {
    expect(displayChapterTitle(null)).toBe('');
    expect(displayChapterTitle(undefined)).toBe('');
  });

  it('空标题 → 第N章（阿拉伯数字，对齐现有 UI）', () => {
    expect(displayChapterTitle({ chapter_number: 1, title: '' })).toBe('第1章');
    expect(displayChapterTitle({ chapter_number: 2, title: '  ' })).toBe('第2章');
    expect(displayChapterTitle({ chapter_number: 3, title: null })).toBe('第3章');
    expect(displayChapterTitle({ chapter_number: 12 })).toBe('第12章');
  });

  it('真实标题 → 原样 trim', () => {
    expect(displayChapterTitle({ chapter_number: 1, title: ' 开端 ' })).toBe('开端');
    expect(displayChapterTitle({ chapter_number: 1, title: '第一章' })).toBe('第一章');
  });
});
