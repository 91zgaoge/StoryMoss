import { describe, it, expect } from 'vitest';
import { chineseNumeral, displayChapterTitle, formatChapterTitle } from '../displayChapterTitle';

describe('chineseNumeral', () => {
  it('常见章号', () => {
    expect(chineseNumeral(1)).toBe('一');
    expect(chineseNumeral(2)).toBe('二');
    expect(chineseNumeral(10)).toBe('十');
    expect(chineseNumeral(11)).toBe('十一');
    expect(chineseNumeral(20)).toBe('二十');
    expect(chineseNumeral(134)).toBe('一百三十四');
    expect(chineseNumeral(1001)).toBe('一千零一');
  });
});

describe('formatChapterTitle', () => {
  it('统一为中文数字章号 + 标题', () => {
    expect(formatChapterTitle(2, '铜钱与脚印')).toBe('第二章 铜钱与脚印');
    expect(formatChapterTitle(13, '香炉边的银镯与铜钱')).toBe('第十三章 香炉边的银镯与铜钱');
    expect(formatChapterTitle(2, '第二章 铜钱与脚印')).toBe('第二章 铜钱与脚印');
  });

  it('训练沙盒继承的源章号（第 134 章）不被 sandbox chapter_number 覆盖', () => {
    expect(formatChapterTitle(1, '第 134 章')).toBe('第一百三十四章');
    expect(formatChapterTitle(1, '第 134 章 魑魅魍魉横行')).toBe('第一百三十四章 魑魅魍魉横行');
  });

  it('系统占位跟当前 chapter_number，修正分章后的过期标题', () => {
    expect(formatChapterTitle(8, '第6章')).toBe('第八章');
    expect(formatChapterTitle(12, '第12章')).toBe('第十二章');
    // 中文占位视为作者/源标题，保留原章号。
    expect(formatChapterTitle(8, '第八章')).toBe('第八章');
    expect(formatChapterTitle(8, '第一章')).toBe('第一章');
  });

  it('空标题 → 纯章号', () => {
    expect(formatChapterTitle(1, '')).toBe('第一章');
    expect(formatChapterTitle(12, null)).toBe('第十二章');
  });
});

describe('displayChapterTitle', () => {
  it('null → 空串', () => {
    expect(displayChapterTitle(null)).toBe('');
    expect(displayChapterTitle(undefined)).toBe('');
  });

  it('空标题 → 中文数字章号', () => {
    expect(displayChapterTitle({ chapter_number: 1, title: '' })).toBe('第一章');
    expect(displayChapterTitle({ chapter_number: 2, title: '  ' })).toBe('第二章');
    expect(displayChapterTitle({ chapter_number: 3, title: null })).toBe('第三章');
    expect(displayChapterTitle({ chapter_number: 12 })).toBe('第十二章');
  });

  it('真实标题 → 章号 + 标题', () => {
    expect(displayChapterTitle({ chapter_number: 1, title: ' 开端 ' })).toBe('第一章 开端');
    expect(displayChapterTitle({ chapter_number: 3, title: '临江夜雨' })).toBe('第三章 临江夜雨');
    expect(displayChapterTitle({ chapter_number: 1, title: '第一章 开端' })).toBe('第一章 开端');
  });
});
