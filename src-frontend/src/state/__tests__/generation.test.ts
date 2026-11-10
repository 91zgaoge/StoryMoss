import { describe, it, expect } from 'vitest';
import {
  parseGenerationEvent,
  parseGenerationPhase,
  PHASE_LABEL,
  type GenerationStatusPayload,
} from '../generation';

const event = (patch: GenerationStatusPayload = {}) =>
  parseGenerationEvent({
    phase: 'generating_candidates',
    progress: 0.5,
    message: '生成候选中',
    task_id: 't1',
    ...patch,
  });

describe('parseGenerationPhase —— 一套词汇表收口三代协议写法', () => {
  it('新协议 snake_case id 原样命中', () => {
    expect(parseGenerationPhase('preparing_context')).toBe('preparing_context');
    expect(parseGenerationPhase('generating_candidates')).toBe('generating_candidates');
    expect(parseGenerationPhase('saving_memory')).toBe('saving_memory');
  });

  it('当前后端发送的中文 label 命中', () => {
    expect(parseGenerationPhase('准备上下文')).toBe('preparing_context');
    expect(parseGenerationPhase('候选生成')).toBe('generating_candidates');
    expect(parseGenerationPhase('内容审校')).toBe('inspecting');
    expect(parseGenerationPhase('保存记忆')).toBe('saving_memory');
  });

  it('「已完成」是终态 completed，而不是被模糊匹配吞成 final_output', () => {
    // 回归守卫：老实现里 '已完成' 同时出现在 final_output 的关键字表里，
    // 终态判断因此永远落空，主活动卡在 running。
    expect(parseGenerationPhase('已完成')).toBe('completed');
    expect(parseGenerationPhase('completed')).toBe('completed');
    expect(parseGenerationPhase('done')).toBe('completed');
  });

  it('出错/取消也是终态', () => {
    expect(parseGenerationPhase('出错')).toBe('error');
    expect(parseGenerationPhase('已取消')).toBe('cancelled');
    expect(parseGenerationPhase('cancelled')).toBe('cancelled');
  });

  it('复合文案按最长别名子串匹配', () => {
    expect(parseGenerationPhase('改写（第 2 轮）')).toBe('rewriting');
    expect(parseGenerationPhase('Inspector 审校（评分 88%）')).toBe('inspecting');
    expect(parseGenerationPhase('生成/质检')).toBe('generating_candidates');
  });

  it('无法识别的阶段返回 null（交由调用方兜底，不猜）', () => {
    expect(parseGenerationPhase('')).toBeNull();
    expect(parseGenerationPhase(undefined)).toBeNull();
    expect(parseGenerationPhase('teleporting')).toBeNull();
  });
});

describe('parseGenerationEvent —— 边界校验一次，之后直接用', () => {
  it('归一化终态与进度', () => {
    const done = event({ phase: '已完成', progress: 1 });
    expect(done.phase).toBe('completed');
    expect(done.outcome).toBe('done');
    expect(done.terminal).toBe(true);
    expect(done.label).toBe('已完成');
  });

  it('未知阶段永远保持 running，不会被误判成终态', () => {
    const unknown = event({ phase: 'something_new', message: '' });
    expect(unknown.phase).toBe('unknown');
    expect(unknown.outcome).toBe('running');
    expect(unknown.terminal).toBe(false);
  });

  it('phase 缺失时用 message 反推（老事件只有文案）', () => {
    const legacy = event({ phase: undefined, message: '内容审校中' });
    expect(legacy.phase).toBe('inspecting');
    expect(legacy.outcome).toBe('running');
  });

  it('progress 做 clamp 与 NaN 归零', () => {
    expect(event({ progress: 3 }).progress).toBe(1);
    expect(event({ progress: -1 }).progress).toBe(0);
    expect(event({ progress: Number.NaN }).progress).toBe(0);
    expect(event({ progress: undefined }).progress).toBe(0);
  });

  it('新协议自带 label 时优先使用', () => {
    const e = event({ phase: 'inspecting', label: '编审打分' });
    expect(e.label).toBe('编审打分');
    expect(e.phase).toBe('inspecting');
  });

  it('缺 message/label 时回落到阶段中文名，绝不出现空状态栏', () => {
    const e = event({ phase: 'saving_memory', message: '' });
    expect(e.label).toBe(PHASE_LABEL.saving_memory);
    expect(e.message).toBe(PHASE_LABEL.saving_memory);
  });

  it('缺 task_id 时给出稳定兜底值', () => {
    expect(event({ task_id: null }).taskId).toBe('unknown');
  });
});
