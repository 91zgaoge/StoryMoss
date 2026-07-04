import { describe, it, expect } from 'vitest';
import { isTextDuplicate, normalizeForDuplicateCheck } from '../isTextDuplicate';

describe('isTextDuplicate', () => {
  const story =
    '人类在无尽的宇宙深处，漩渦星系的最外层行星上，一座被粗劲磨炼成为强硬的生存场景的大城市。';

  it('returns false when existing text is empty', () => {
    expect(isTextDuplicate('', story)).toBe(false);
  });

  it('returns false when generated text is empty', () => {
    expect(isTextDuplicate(story, '')).toBe(false);
  });

  it('returns true when existing text equals generated text', () => {
    expect(isTextDuplicate(story, story)).toBe(true);
  });

  it('returns true when existing text contains generated text with different punctuation/whitespace', () => {
    const formatted = `<p>${story}</p>`;
    const ghost = story.replace(/。/g, '。\n');
    expect(isTextDuplicate(formatted, ghost)).toBe(true);
  });

  it('returns true when generated text is a prefix of existing text', () => {
    const prefix = story.slice(0, 20);
    expect(isTextDuplicate(story, prefix)).toBe(true);
  });

  it('returns false for unrelated texts', () => {
    expect(isTextDuplicate('完全不同的内容', story)).toBe(false);
  });

  it('normalization strips HTML tags', () => {
    const html = '<p>hello <strong>world</strong></p>';
    expect(normalizeForDuplicateCheck(html)).toBe('helloworld');
  });
});
