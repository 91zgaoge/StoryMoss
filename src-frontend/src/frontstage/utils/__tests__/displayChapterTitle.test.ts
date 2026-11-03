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

  it('阿拉伯数字占位标题跟 chapter_number，不照显示过期库标题', () => {
    expect(displayChapterTitle({ chapter_number: 8, title: '第6章' })).toBe('第8章');
    expect(displayChapterTitle({ chapter_number: 2, title: '第2章' })).toBe('第2章');
  });

  it('作者手写的中文数字标题原样显示，不被章号覆盖', () => {
    expect(displayChapterTitle({ chapter_number: 1, title: '第一章' })).toBe('第一章');
    expect(displayChapterTitle({ chapter_number: 8, title: '第八章' })).toBe('第八章');
    // 作者可自选标题，即使与章号不一致也尊重原文
    expect(displayChapterTitle({ chapter_number: 8, title: '第一章' })).toBe('第一章');
  });

  it('真实标题 → 原样 trim', () => {
    expect(displayChapterTitle({ chapter_number: 1, title: ' 开端 ' })).toBe('开端');
    expect(displayChapterTitle({ chapter_number: 3, title: '临江夜雨' })).toBe('临江夜雨');
    expect(displayChapterTitle({ chapter_number: 1, title: '第一章 开端' })).toBe('第一章 开端');
  });
});
