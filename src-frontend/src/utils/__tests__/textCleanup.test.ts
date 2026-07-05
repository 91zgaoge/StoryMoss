import { describe, it, expect } from 'vitest';
import { trimSelfRepetition, isTextDuplicate } from '../textCleanup';

describe('trimSelfRepetition', () => {
  it('returns short text unchanged', () => {
    const text = '这是一个短文本。';
    expect(trimSelfRepetition(text)).toBe(text);
  });

  it('returns text without repetition unchanged', () => {
    const text =
      '清晨，一缕微弱的光线透过被单的缝隙照进来，刺痛了何子衿的眼睛。\n\n' +
      '他闭着眼睛叹了口气，翻了个身，想再次沉浸在梦中那温暖的氛围里。\n\n' +
      '何子衿是一个理想主义者，毕业于名牌大学的管理学院。';
    expect(trimSelfRepetition(text)).toBe(text);
  });

  it('removes a trailing paragraph that duplicates the first paragraph', () => {
    const middle =
      '幽暗中，窄窄的走道呈现出一道渐渐明亮的光线。在这瞬间，可以感受到一股腐烂的气味，仿佛世界的残余生物都在不断崩殖。' +
      '少年的身影从黑暗中浮现出来，手持着一根闪耀的闪光灯。他的脸上泛着惊恐的光辉。这里的阴森气渐渐压迫了他，他知道如果没有完成当前的任务，他将讨厌到极致的生活甚至更加恶劣。' +
      '少年的目标是抓取一个正在勃勃生长的菌菇。这种菌菇在这个恶魔世界中具有重要的价值。他在黑暗中挑选了一条窄通道，深深地沟通着阴森潮湿的地下。' +
      '他迈着匆促的步伐向前，闪光灯切分着黑暗。突然，他感到湿润的触感扯住了他的胸膛。紧接着，他听到一个尖锐的咆哮。';
    const repeat =
      '尽管他已经成功抓取了菌菇，但他知道，这只是开始。在这个残酷的世界里，一个成功，也只是催生了更多的挑战。';
    const text = `${repeat}\n\n${middle}\n\n${repeat}`;
    const result = trimSelfRepetition(text);
    expect(result).not.toContain(repeat + '\n\n' + repeat);
    expect(result.startsWith(repeat)).toBe(true);
    expect(result.endsWith(repeat)).toBe(false);
    expect(result).toContain(middle);
  });

  it('keeps only one copy when the entire text is duplicated', () => {
    const copy =
      '他穿过废墟，脚步在碎石上发出轻微的响动。天空是铅灰色的，空气中弥漫着焦灼的味道。\n\n' +
      '远处传来一阵低沉的轰鸣，他停下脚步，握紧了手中的武器。';
    const text = copy + '\n\n' + copy;
    const result = trimSelfRepetition(text);
    expect(result).toBe(copy);
  });

  it('trims a long repeated suffix inside a single paragraph', () => {
    const prefix =
      '在这个残酷的世界里，一个成功，也只是催生了更多的挑战。少年的目标是抓取一个正在勃勃生长的菌菇。';
    const middle = '他穿过狭窄的通道，避开那些潜伏在黑暗中的危险。';
    const text = prefix + middle + prefix;
    const result = trimSelfRepetition(text);
    expect(result).toBe(prefix + middle);
  });

  it('ignores short accidental prefix-suffix matches', () => {
    const text = '他走进了房间。屋里的陈设很简单，只有一张桌子和一把椅子。他坐了下来。';
    expect(trimSelfRepetition(text)).toBe(text);
  });

  it('does not break on HTML tags and leaves short repeats untouched', () => {
    const repeat = '<p>开头段落重复内容。</p>';
    const middle = '<p>中间的正常内容。</p>';
    const text = repeat + middle + repeat;
    const result = trimSelfRepetition(text);
    expect(result).toBe(text);
  });

  // v0.26.15 新增：截图中“末尾连续 k 段重复开头 k 段”的模式
  it('removes trailing k paragraphs that duplicate the first k paragraphs', () => {
    const p1 = '他不知道自己多少岁，但这种生活让他感受到时间的流逝。';
    const p2 = '当他的狗伴催促他抬头时，他顿悟了自己的位置。';
    const p3 = '这不是他的生活的终局，他要从这片凋零的地平线中夺回生命的意义。';
    const p4 = '辽东荒凉之中，一片狭窄的谷丘偶然掩蔽了世界的残留。';
    const text = [p1, p2, p3, p4, p1, p2, p3].join('\n\n');
    const result = trimSelfRepetition(text);
    expect(result).toBe([p1, p2, p3, p4].join('\n\n'));
  });

  // v0.26.15 新增：单一段落内部包含前面多段拼接的重复
  it('trims repeated prefix block appended in the last paragraph', () => {
    const p1 =
      '他不知道自己多少岁，但这种生活让他感受到时间的流逝。疾风中的寂寞催作了他的心理崩溃。';
    const p2 = '当他的狗伴在他身前伸出一根粗糙的嘴，催促他抬头时，他顿悟了自己的位置。';
    const p3 = '这不是他的生活的终局，他要从这片凋零的地平线中夺回生命的意义。';
    const p4prefix = '辽东荒凉之中，一片狭窄的谷丘偶然掩蔽了世界的残留。此地尽是干枯的植物。';
    const text = [p1, p2, p3, p4prefix + p1 + p2 + p3].join('\n\n');
    const result = trimSelfRepetition(text);
    expect(result).toBe([p1, p2, p3, p4prefix].join('\n\n'));
  });
});

describe('isTextDuplicate', () => {
  it('detects when generated text is contained in existing text', () => {
    const existing = '这是一个很长的故事开头，后面还有很多内容。';
    const generated = '这是一个很长的故事开头';
    expect(isTextDuplicate(existing, generated)).toBe(true);
  });

  it('returns false for unrelated texts', () => {
    expect(isTextDuplicate('故事 A', '故事 B')).toBe(false);
  });
});
