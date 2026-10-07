import { describe, it, expect } from 'vitest';
import {
  countWords,
  autoFormatText,
  formatDate,
  formatNumber,
  truncateText,
  mergeHangingClosingPunct,
  mergeLoneClosingPunctParagraphs,
  mergeLeadingClosingPunctParagraphs,
  textToParagraphsHtml,
} from '../format';

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

  it('plain-text path: hanging closing quote on its own line is merged back', () => {
    // LLM 软换行把闭合引号单独成行，分段前必须并回上一行
    const input =
      '他缓缓地喊道：「快走吧，别再回头看了。\n」\n\n第二段其实也很长，超过十五个字符没问题。';
    const result = autoFormatText(input);
    expect(result).not.toContain('<p>」</p>');
    expect(result).toContain('」</p>');
    expect(result).toContain('别再回头看了。」');
  });

  it('blank-line path: indented hanging closing quote is not its own paragraph', () => {
    // 真机：对话句号后空行 + 段首全角缩进 + 下引号单独成段（带 2em 缩进）
    const input =
      '苏福贵脸色微变，低声道：“王爷，外头有动静，似乎是……京城的禁军。\n\n　　”\n\n景亲王眼中闪过一丝凝重，对苏亦铁道：“王爷且先避一避。”';
    const result = autoFormatText(input);
    expect(result).not.toMatch(/<p>\s*”\s*<\/p>/);
    expect(result).toContain('京城的禁军。”');
  });

  it('passthrough path: existing <p>"</p> lone closing-punct paragraph is merged', () => {
    // DB 存量 HTML 中的孤闭合引号段落，在显示层并入上一段
    const input =
      '<p>他控制着局面，缓缓说道：「就这样吧。</p><p>”</p><p>第二段有足够长的内容保留在此。</p>';
    const result = autoFormatText(input);
    expect(result).not.toContain('<p>”</p>');
    expect(result).toContain('就这样吧。”</p>');
    expect(result).toContain('<p>第二段有足够长的内容保留在此。</p>');
  });

  it('passthrough path: ideographic-indented lone closing quote is merged', () => {
    const input =
      '<p>苏福贵脸色微变，低声道：“王爷，外头有动静，似乎是……京城的禁军。</p><p>　　”</p><p>景亲王眼中闪过一丝凝重。</p>';
    const result = autoFormatText(input);
    expect(result).not.toMatch(/<p>\s*”\s*<\/p>/);
    expect(result).not.toContain('<p>　　”</p>');
    expect(result).toContain('京城的禁军。”');
  });

  // 真机《帝国的烟火》第 2 章：无空行的连续段落走「智能句子拆分」，
  // 句末标点处的切分把收尾引号切成下一句 → 段落正好在那一处断开 → 段首孤引号
  // （V128 的「整段仅闭合标点」规则覆盖不到，引号后面还跟着正文）。
  it('sentence-split path: 句末引号不被切到下一段', () => {
    const input =
      '景亲王嘴角勾起一抹极淡的弧度，那笑容冰冷刺骨。\n' +
      '“苏世子，本亲王给你一炷香的时间。”他身体前倾，压低了声音，只有苏亦铁能听见，“想清楚，你要保的，是苏家的名声，还是你苏家的命。”\n' +
      '大堂内的空气仿佛凝固成了实质，沉重得让人窒息。苏亦铁攥紧了拳头，指甲深深嵌入掌心，鲜血渗出，却感觉不到疼痛。他看着景亲王那双毫无温度的眼睛，第一次真切地感受到了什么是权力的碾压。\n' +
      '父亲的那一拳，打碎了公主，也打碎了苏家最后的屏障。\n' +
      '而现在，猎手已经入场，猎物无路可退。';
    const result = autoFormatText(input);
    expect(result).not.toMatch(/<\/p><p>”/);
    expect(result).toContain('还是你苏家的命。”</p>');
    expect(result).toContain('<p>大堂内的空气仿佛凝固成了实质');
  });

  it('passthrough path: 段首孤引号（引号后带正文）并回上一段', () => {
    const input = '<p>…还是你苏家的命。</p><p>”\n大堂内的空气仿佛凝固成了实质。</p>';
    const result = autoFormatText(input);
    expect(result).not.toMatch(/<\/p><p>”/);
    expect(result).toContain('你苏家的命。”</p>');
    expect(result).toContain('<p>大堂内的空气仿佛凝固成了实质。</p>');
  });
});

describe('mergeHangingClosingPunct', () => {
  it('merges a closing quote hanging on its own line back to previous line', () => {
    expect(mergeHangingClosingPunct('他喊道："快走吧。\n"')).toBe('他喊道："快走吧。"');
    expect(mergeHangingClosingPunct("……控制'。\n”")).toBe("……控制'。”");
    expect(mergeHangingClosingPunct('段落结束\n')).toBe('段落结束\n');
  });

  it('merges across multiple consecutive newlines', () => {
    expect(mergeHangingClosingPunct('上一行\n\n\n」')).toBe('上一行」');
  });

  it('merges when whitespace or ideographic indent sits between newline and closing quote', () => {
    expect(mergeHangingClosingPunct('京城的禁军。\n\n　　”')).toBe('京城的禁军。”');
    expect(mergeHangingClosingPunct('莫要再自误了。\n ”')).toBe('莫要再自误了。”');
    expect(mergeHangingClosingPunct('上一行\n\u200b”')).toBe('上一行”');
  });

  it('merges every closing-direction punct in the set', () => {
    for (const c of ['"', "'", '’', '”', '」', '』', '）', '】', '》', '〉', ']', '}']) {
      expect(mergeHangingClosingPunct(`上一行\n${c}`), `closing ${c}`).toBe(`上一行${c}`);
    }
  });

  it('does NOT merge opening-direction punct', () => {
    for (const c of ['「', '『', '（', '【', '《', '〈', '[', '{']) {
      expect(mergeHangingClosingPunct(`上一行\n${c}对话`), `opening ${c}`).toBe(`上一行\n${c}对话`);
    }
  });

  it('does NOT merge when there is no previous line', () => {
    expect(mergeHangingClosingPunct('\n"开头')).toBe('\n"开头');
    expect(mergeHangingClosingPunct('\n\n”开头')).toBe('\n\n”开头');
  });

  it('returns empty input unchanged', () => {
    expect(mergeHangingClosingPunct('')).toBe('');
  });
});

describe('mergeLoneClosingPunctParagraphs', () => {
  it('merges a paragraph containing only a closing quote into the previous one', () => {
    expect(mergeLoneClosingPunctParagraphs('<p>他控制着局面。</p><p>”</p>')).toBe(
      '<p>他控制着局面。”</p>'
    );
  });

  it('merges entity-form lone closing punct paragraphs', () => {
    expect(mergeLoneClosingPunctParagraphs('<p>段落。</p><p>&rdquo;</p>')).toBe(
      '<p>段落。&rdquo;</p>'
    );
    expect(mergeLoneClosingPunctParagraphs('<p>段落。</p><p>&quot;</p>')).toBe(
      '<p>段落。&quot;</p>'
    );
    expect(mergeLoneClosingPunctParagraphs('<p>段落。</p><p>&#x201D;</p>')).toBe(
      '<p>段落。&#x201D;</p>'
    );
    expect(mergeLoneClosingPunctParagraphs('<p>段落。</p><p>&#8221;</p>')).toBe(
      '<p>段落。&#8221;</p>'
    );
  });

  it('merges lone punct paragraph with surrounding whitespace inside', () => {
    expect(mergeLoneClosingPunctParagraphs('<p>段落。</p><p> ” </p>')).toBe('<p>段落。”</p>');
    expect(mergeLoneClosingPunctParagraphs('<p>段落。</p><p>　　”</p>')).toBe('<p>段落。”</p>');
  });

  it('merges consecutive lone closing-punct paragraphs', () => {
    expect(mergeLoneClosingPunctParagraphs('<p>甲。</p><p>”</p><p>’</p>')).toBe('<p>甲。”’</p>');
  });

  it('does NOT merge when there is no previous paragraph', () => {
    expect(mergeLoneClosingPunctParagraphs('<p>”</p><p>段落。</p>')).toBe('<p>”</p><p>段落。</p>');
  });

  it('does NOT merge paragraphs that contain more than closing punct', () => {
    expect(mergeLoneClosingPunctParagraphs('<p>甲。</p><p>”他说</p>')).toBe(
      '<p>甲。</p><p>”他说</p>'
    );
    // 纯空白段（无闭合标点）不动
    expect(mergeLoneClosingPunctParagraphs('<p>甲。</p><p> </p>')).toBe('<p>甲。</p><p> </p>');
  });

  it('returns input without <p> tags unchanged', () => {
    expect(mergeLoneClosingPunctParagraphs('纯文本\n"无段落')).toBe('纯文本\n"无段落');
    expect(mergeLoneClosingPunctParagraphs('')).toBe('');
  });
});

describe('mergeLeadingClosingPunctParagraphs', () => {
  // 真机《帝国的烟火》第 2 章形态：句末引号被切到下一段段首，独占一行
  it('merges a paragraph-leading closing quote back to the previous paragraph', () => {
    expect(
      mergeLeadingClosingPunctParagraphs(
        '<p>…还是你苏家的命。</p><p>”\n大堂内的空气仿佛凝固成了实质。</p>'
      )
    ).toBe('<p>…还是你苏家的命。”</p><p>大堂内的空气仿佛凝固成了实质。</p>');
  });

  it('merges entity-form leading closing punct', () => {
    expect(mergeLeadingClosingPunctParagraphs('<p>甲。</p><p>&rdquo;\n乙。</p>')).toBe(
      '<p>甲。&rdquo;</p><p>乙。</p>'
    );
    expect(mergeLeadingClosingPunctParagraphs('<p>甲。</p><p>&#x201D;乙。</p>')).toBe(
      '<p>甲。&#x201D;</p><p>乙。</p>'
    );
  });

  it('merges a paragraph that is only leading closing punct', () => {
    expect(mergeLeadingClosingPunctParagraphs('<p>甲。</p><p>”</p>')).toBe('<p>甲。”</p>');
    expect(mergeLeadingClosingPunctParagraphs('<p>甲。</p><p>”</p><p>’</p>')).toBe('<p>甲。”’</p>');
  });

  it('merges every directional closing punct in the set', () => {
    for (const c of ['”', '’', '」', '』', '）', '】', '》', '〉']) {
      expect(
        mergeLeadingClosingPunctParagraphs(`<p>甲。</p><p>${c}\n乙。</p>`),
        `closing ${c}`
      ).toBe(`<p>甲。${c}</p><p>乙。</p>`);
    }
  });

  it('does NOT move ASCII straight quotes (may be an opening quote)', () => {
    expect(mergeLeadingClosingPunctParagraphs('<p>甲。</p><p>"\n他说。</p>')).toBe(
      '<p>甲。</p><p>"\n他说。</p>'
    );
    expect(mergeLeadingClosingPunctParagraphs("<p>甲。</p><p>'\n他说。</p>")).toBe(
      "<p>甲。</p><p>'\n他说。</p>"
    );
  });

  it('does NOT move opening-direction punct', () => {
    for (const c of ['「', '『', '（', '【', '《', '〈', '“', '‘']) {
      expect(mergeLeadingClosingPunctParagraphs(`<p>甲。</p><p>${c}乙。</p>`), `opening ${c}`).toBe(
        `<p>甲。</p><p>${c}乙。</p>`
      );
    }
  });

  it('does NOT merge when there is no previous paragraph', () => {
    expect(mergeLeadingClosingPunctParagraphs('<p>”\n甲。</p>')).toBe('<p>”\n甲。</p>');
  });

  it('returns input without <p> tags unchanged', () => {
    expect(mergeLeadingClosingPunctParagraphs('纯文本\n”无段落')).toBe('纯文本\n”无段落');
    expect(mergeLeadingClosingPunctParagraphs('')).toBe('');
  });
});

describe('textToParagraphsHtml', () => {
  it('wraps each non-empty line in <p> and merges hanging closing punct', () => {
    expect(textToParagraphsHtml('第一行\n他说："你好。\n"\n第三行')).toBe(
      '<p>第一行</p><p>他说："你好。"</p><p>第三行</p>'
    );
  });

  it('skips empty lines', () => {
    expect(textToParagraphsHtml('甲\n\n\n乙')).toBe('<p>甲</p><p>乙</p>');
  });

  it('wraps a single line', () => {
    expect(textToParagraphsHtml('单行')).toBe('<p>单行</p>');
  });

  it('returns empty string for empty/blank input', () => {
    expect(textToParagraphsHtml('')).toBe('');
    expect(textToParagraphsHtml('\n\n')).toBe('');
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
