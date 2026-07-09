import { describe, it, expect } from 'vitest';
import { displayStoryTitle, isPlaceholderTitle } from '../displayStoryTitle';

describe('displayStoryTitle', () => {
  it('无故事无正文 → 草苔', () => {
    expect(displayStoryTitle(null, false)).toBe('草苔');
  });

  it('无故事有正文 → 未命名', () => {
    expect(displayStoryTitle(null, true)).toBe('未命名');
  });

  it('空标题有正文 → 未命名', () => {
    expect(displayStoryTitle({ id: '1', title: '' }, true)).toBe('未命名');
    expect(displayStoryTitle({ id: '1', title: '  ' }, true)).toBe('未命名');
    expect(displayStoryTitle({ id: '1', title: '草苔' }, true)).toBe('未命名');
  });

  it('空标题无正文 → 草苔', () => {
    expect(displayStoryTitle({ id: '1', title: '' }, false)).toBe('草苔');
    expect(displayStoryTitle({ id: '1', title: '草苔' }, false)).toBe('草苔');
  });

  it('真实标题 → 原样 trim', () => {
    expect(displayStoryTitle({ id: '1', title: ' 星际间谍 ' }, true)).toBe('星际间谍');
    expect(displayStoryTitle({ id: '1', title: '星际间谍' }, false)).toBe('星际间谍');
  });

  it('isPlaceholderTitle', () => {
    expect(isPlaceholderTitle('')).toBe(true);
    expect(isPlaceholderTitle('  ')).toBe(true);
    expect(isPlaceholderTitle('草苔')).toBe(true);
    expect(isPlaceholderTitle('未命名')).toBe(true);
    expect(isPlaceholderTitle('我的小说')).toBe(false);
  });
});
