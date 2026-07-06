import { describe, it, expect } from 'vitest';
import { countWords, autoFormatText, formatDate, formatNumber, truncateText } from '../format';

describe('countWords', () => {
  it('should count Chinese characters', () => {
    expect(countWords('今天天气很好')).toBe(6);
  });

  it('should count English words', () => {
    expect(countWords('Hello world test')).toBe(3);
  });

  it('should count mixed Chinese and English text', () => {
    expect(countWords('Hello 世界 this 是 a 测试')).toBe(8);
  });

  it('should return 0 for empty string', () => {
    expect(countWords('')).toBe(0);
  });

  it('should count punctuation correctly', () => {
    expect(countWords('你好，世界！Hello world.')).toBe(6);
  });
});

describe('autoFormatText', () => {
  it('should return empty string for empty input', () => {
    expect(autoFormatText('')).toBe('');
    expect(autoFormatText('   ')).toBe('');
  });

  it('should format text with double newlines into paragraphs', () => {
    const input = '第一段内容。\n\n第二段内容。';
    const result = autoFormatText(input);
    expect(result).toContain('<p>');
    expect(result).toContain('</p>');
  });

  it('should normalize quotes in text', () => {
    const input = '"你好"';
    const result = autoFormatText(input);
    expect(result).toContain('「');
    expect(result).toContain('」');
  });

  it('should return empty string for whitespace-only input', () => {
    expect(autoFormatText('   \n\n   ')).toBe('');
  });

  it('should NOT double content for plain Chinese text without blank-line separators', () => {
    // 模拟 Genesis 第一章：纯中文长文，单换行或无换行，走 splitChineseSentences 分支。
    // 复现 v0.26.16 实测 bug：1446 字输入被 autoFormatText 产出 ~3060 字 HTML（双倍）。
    const sentences: string[] = [];
    for (let i = 0; i < 30; i++) {
      sentences.push(`这是第${i + 1}个测试句子，用于验证自动排版不会把内容翻倍。`);
    }
    const input = sentences.join('');
    const result = autoFormatText(input);
    const resultPlain = result.replace(/<[^>]+>/g, '').replace(/\s+/g, '');
    const inputPlain = input.replace(/\s+/g, '');
    // 关键契约：排版后纯文本长度必须 ≈ 输入纯文本长度，不能 2×。
    expect(resultPlain.length).toBeLessThanOrEqual(inputPlain.length + 5);
    expect(resultPlain.length).toBeGreaterThanOrEqual(inputPlain.length - 5);
    // 显式断言不存在双倍：结果不应包含两份输入。
    expect(resultPlain).not.toBe(inputPlain + inputPlain);
  });

  it('splitChineseSentences path: single-sentence input should not be duplicated', () => {
    const input = '这是一句完整的话。';
    const result = autoFormatText(input);
    const resultPlain = result.replace(/<[^>]+>/g, '').replace(/\s+/g, '');
    expect(resultPlain).toBe(input.replace(/\s+/g, ''));
  });

  it('splitChineseSentences path: multi-sentence input without blank lines should preserve content once', () => {
    const input = '第一句结束了。第二句也结束了。第三句同样结束。';
    const result = autoFormatText(input);
    const resultPlain = result.replace(/<[^>]+>/g, '').replace(/\s+/g, '');
    expect(resultPlain).toBe(input.replace(/\s+/g, ''));
  });
});

describe('formatDate', () => {
  it('should format date string to zh-CN locale', () => {
    const result = formatDate('2024-01-15');
    expect(result).toContain('2024');
    expect(result).toContain('15');
  });
});

describe('formatNumber', () => {
  it('should return number as string when below 1000', () => {
    expect(formatNumber(500)).toBe('500');
  });

  it('should format number with k when >= 1000', () => {
    expect(formatNumber(1500)).toBe('1.5k');
  });
});

describe('truncateText', () => {
  it('should return original text if within max length', () => {
    expect(truncateText('short', 10)).toBe('short');
  });

  it('should truncate text and append ellipsis', () => {
    expect(truncateText('hello world', 5)).toBe('hello...');
  });
});
