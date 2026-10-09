import { describe, it, expect } from 'vitest';
import {
  trimSelfRepetition,
  isTextDuplicate,
  stripExistingOverlap,
  trimDanglingTail,
  sanitizeContinuationOutput,
  stripInstructionEcho,
  isGhostDeliveredInContent,
} from '../textCleanup';

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

  // v0.26.24: 散布式句子块重复——同一多句块以不同上下文在文中出现 ≥2 次。
  it('trims interspersed repeated sentence block (continuation loop)', () => {
    const block =
      '冥界的阴霾更加浓烈，在这个苦难的奋斗中，主角与恶魔的共同牢笼被沉沦在更深的冥界。幻境的沉淀，坚定的决心。一场惨烈的冒险，即将开始。';
    const lead = '他握紧恶魔的喉咙，咆哮着宣告契约的成立。';
    const mid = '深渊的阴暗中回荡着叩门的沉闷声。';
    const text = lead + block + mid + block;
    expect(trimSelfRepetition(text)).toBe(lead + block + mid);
  });

  // v0.26.24: 散布式单长句重复——同一长句（归一化 ≥ 15 字）在文中出现两次。
  it('trims interspersed single long sentence repeat', () => {
    const s = '恶魔的眼眶中闪过一丝恐惧，但在他的决心中，牢牢捆绑了他的挣脱。';
    const text = s + '他咆哮不止。' + s;
    expect(trimSelfRepetition(text)).toBe(s + '他咆哮不止。');
  });

  // v0.26.24: 短句重复不裁剪（< 15 归一化字），避免误伤首尾呼应。
  it('leaves short interspersed sentence repeat unchanged', () => {
    const text =
      '清晨的阳光洒在窗台上，新的一天开始了。我喝了一杯咖啡，准备出门。清晨的阳光洒在窗台上。';
    expect(trimSelfRepetition(text)).toBe(text);
  });
});

describe('stripExistingOverlap', () => {
  it('strips regenerated passage from existing tail (creative_workflow 2026-07-07)', () => {
    const existing =
      '冥府的牢笼牢牢锁住了他。他正在等待巅峰。\n\n恶魔的嘴唇弯曲出一个苦涎的笑，一颗棘刺般的闪烁在其眼中。我是你的牺牲，为你的愿望牺牲。';
    const generated =
      '恶魔的嘴唇弯曲出一个苦涎的笑，一颗棘刺般的闪烁在其眼中。我是你的牺牲，为你的愿望牺牲。主角深吸一口气，朝着冥界巅峰奔跑。';
    const result = stripExistingOverlap(generated, existing);
    expect(result).not.toContain('恶魔的嘴唇弯曲出一个苦涎的笑');
    expect(result).toContain('朝着冥界巅峰奔跑');
  });

  it('returns unchanged when no overlap', () => {
    const existing = '他穿过废墟，脚步在碎石上发出轻微的响动。';
    const generated = '远处传来一阵低沉的轰鸣，他停下脚步。';
    expect(stripExistingOverlap(generated, existing)).toBe(generated);
  });
});

describe('trimDanglingTail', () => {
  it('strips truncated last sentence from timeout cutoff', () => {
    const text =
      '主角与恶魔浸入到一个更深的冥境中，在那里，他们将面对更糟糕的冥府巅峰之谜。在牢笼前，恶魔停止了咬堪，牢牢捆绑在主角的手中。冥界的阴霾更。';
    const result = trimDanglingTail(text);
    expect(result).not.toContain('冥界的阴霾更');
    expect(result.endsWith('牢牢捆绑在主角的手中。')).toBe(true);
  });
});

describe('sanitizeContinuationOutput', () => {
  it('applies full pipeline for continuation output', () => {
    const block =
      '冥界的阴霾更加浓烈，在这个苦难的奋斗中，主角与恶魔的共同牢笼被沉沦在更深的冥界。幻境的沉淀，坚定的决心。一场惨烈的冒险，即将开始。';
    const existing = `前文内容。${block}`;
    const generated = `${block}新的情节在这里展开。${block}`;
    const result = sanitizeContinuationOutput(generated, existing);
    expect(result).toBe('新的情节在这里展开。');
  });
});

describe('isTextDuplicate', () => {
  it('detects when generated text is contained in existing text (>= 30 normalized chars)', () => {
    // v0.30.41: 最小长度守卫要求 >= 30 归一化字符才进行去重检查
    const existing =
      '这是一个很长的故事开头，后面还有很多内容。主角踏上了漫长而艰辛的旅途，穿越了无数的山川河流。';
    const generated = '这是一个很长的故事开头，后面还有很多内容。主角踏上了漫长而艰辛的旅途';
    expect(isTextDuplicate(existing, generated)).toBe(true);
  });

  it('returns false for unrelated texts', () => {
    expect(isTextDuplicate('故事 A', '故事 B')).toBe(false);
  });

  it('returns false for short generated text even if contained in existing (v0.30.41)', () => {
    const longNovel = '续写一段新的故事。' + '正文内容继续发展。'.repeat(50);
    expect(isTextDuplicate(longNovel, '续写')).toBe(false);
    expect(isTextDuplicate(longNovel, '续写\n黑暗。')).toBe(false);
  });
});

describe('stripInstructionEcho', () => {
  it('strips instruction echo at the beginning of generated text', () => {
    const generated = '续写\n黑暗。\n彻底的、厚重的、几乎凝固成固体的黑暗笼罩了整个世界。';
    const result = stripInstructionEcho(generated, '续写');
    expect(result.startsWith('黑暗。')).toBe(true);
    expect(result).not.toContain('续写');
  });

  it('strips instruction echo with colon separator', () => {
    const generated = '续写：\n黑暗降临，一切归于寂静。';
    const result = stripInstructionEcho(generated, '续写');
    expect(result.startsWith('黑暗降临')).toBe(true);
  });

  it('does not strip when generated text does not start with instruction', () => {
    const generated = '黑暗笼罩了整个世界，没有一丝光亮能够穿透。';
    const result = stripInstructionEcho(generated, '续写');
    expect(result).toBe(generated);
  });

  it('does not strip when user input is too short (< 2 chars)', () => {
    const generated = '写\n黑暗降临。';
    const result = stripInstructionEcho(generated, '写');
    expect(result).toBe(generated);
  });

  it('does not strip when user input is empty', () => {
    const generated = '续写\n黑暗降临。';
    const result = stripInstructionEcho(generated, '');
    expect(result).toBe(generated);
  });

  it('preserves original text when stripping would leave too little content', () => {
    const generated = '续写\n好的';
    const result = stripInstructionEcho(generated, '续写');
    expect(result).toBe(generated);
  });

  it('strips longer instruction echo', () => {
    const generated = '继续写下一章\n\n黑暗笼罩了整个世界，没有一丝光亮。';
    const result = stripInstructionEcho(generated, '继续写下一章');
    expect(result.startsWith('黑暗笼罩')).toBe(true);
  });
});

describe('isGhostDeliveredInContent（v0.65.3 切章保留未确认幽灵续文）', () => {
  // 模拟真机《帝国的烟火》分章形态：幽灵 = 旧文尾部重演 + 新续写，新章正文 = 新续写
  const REPLAY_TAIL =
    '黄衣太监收了明黄缎卷，拢进袖中，尖细的嗓音又在堂中荡开：“镇北兵符——请苏大执事速交。”他微微侧身，让出身后半步的位置。';
  const CONTINUATION =
    '苏福贵没有动。他仍跪在门边，一只手按着地上的门闩，指节泛白。“兵符在后堂。”他的声音沉得像压着一层碎石，“先王尸骨未寒，恕卑职不能交。”' +
    '黄衣太监嘴角那点似有似无的笑凝了一瞬。他收回拢着袖子的手，眼角往景亲王方向飞快一瞥，随即拔高了声调：“不交？苏大执事，这可是景亲王令旨——抗旨不遵，你是想让满堂苏氏男女一并担罪？”' +
    '门外檐下传来甲叶碰撞的脆响。送亲队伍中那三百铁甲卫终于动了，黑压压的甲影在暮色里泛着冷光，刀鞘碰着石阶，一声接一声，不急不缓地碾过来。';

  it('幽灵整体已在新章正文中 → 视为已呈现，可安全丢弃', () => {
    expect(isGhostDeliveredInContent(CONTINUATION, `开头段落。${CONTINUATION}结尾段落。`)).toBe(
      true
    );
  });

  it('幽灵 = 旧文尾部重演 + 新续写，新章正文即新续写 → 视为已呈现', () => {
    expect(isGhostDeliveredInContent(`${REPLAY_TAIL}${CONTINUATION}`, CONTINUATION)).toBe(true);
  });

  it('幽灵结尾有新章没有的文字 → 必须保留，不得静默丢弃', () => {
    const ghost = `${REPLAY_TAIL}${CONTINUATION}${'苏福贵闭了闭眼。他松开按在门闩上的手，慢慢站起来，朝景亲王深躬一礼，声音哑得几乎碎了：“卑职……领命。”'}`;
    expect(isGhostDeliveredInContent(ghost, CONTINUATION)).toBe(false);
  });

  it('新章正文与幽灵完全不同 → 保留', () => {
    expect(
      isGhostDeliveredInContent(
        CONTINUATION,
        '大雪初晴。一条用红毡铺就的长道蜿蜒地从城门直达城中心的镇北王府。'
      )
    ).toBe(false);
  });

  it('空幽灵或空正文不做「已呈现」判定（保留优先）', () => {
    expect(isGhostDeliveredInContent('', CONTINUATION)).toBe(false);
    expect(isGhostDeliveredInContent(CONTINUATION, '')).toBe(false);
  });
});
