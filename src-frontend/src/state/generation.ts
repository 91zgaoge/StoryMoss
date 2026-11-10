/**
 * `generation-status` 协议边界 —— 全应用唯一的解释器。
 *
 * 重构前的状态：同一份 payload 被四处各自解释，且词汇表互相冲突
 *  - `hooks/useBackendActivityListener.ts` 用英文 `'completed'` 判断终态；
 *  - `frontstage/FrontstageApp.tsx` 用中文 `'已完成'` 判断终态；
 *  - 两处各抄了一份 `PRECISE_PHASE_PATTERNS`，用中文子串反推阶段；
 *  - 后端 `events.rs` 实际发的是中文展示文案（`GenerationPhase::as_str()`）。
 *
 * 结果：hook 侧的终态判断恒为假 —— 生成结束后面板仍显示「运行中」，
 * 只能等看门狗 / failAllRunning 兜底。这正是「同一状态，两套词汇」的代价。
 *
 * 现在：数据在唯一入口 `parseGenerationEvent` 校验一次，之后调用方只读
 * `outcome` / `label` / `progress`，不再比较 phase 字符串。新后端发 snake_case
 * id，老后端发中文 label，两者都收口到同一组 `GenerationPhaseId`。
 */

/** 生成任务的机器可读阶段。与 Rust `events::GenerationPhase` 一一对应。 */
export type GenerationPhaseId =
  | 'preparing_context'
  | 'generating_candidates'
  | 'inspecting'
  | 'rewriting'
  | 'final_output'
  | 'saving_memory'
  | 'completed'
  | 'error'
  | 'cancelled';

/** 解析失败时保持「进行中」，绝不把未知阶段误判成终态。 */
export type ParsedGenerationPhase = GenerationPhaseId | 'unknown';

export type GenerationOutcome = 'running' | 'done' | 'failed' | 'cancelled';

/** 后端 `generation-status` 事件载荷。`label` 仅新协议携带。 */
export interface GenerationStatusPayload {
  phase?: string | null;
  label?: string | null;
  progress?: number | null;
  message?: string | null;
  elapsed_ms?: number | null;
  task_id?: string | null;
  request_id?: string | null;
}

export interface GenerationEvent {
  taskId: string;
  requestId: string | null;
  phase: ParsedGenerationPhase;
  /** 面向用户的中文文案，已去掉空串/未映射的中间态。 */
  label: string;
  /** 后端原始 message（可能携带更细的上下文），保留给日志与详情。 */
  message: string;
  /** 0..1，已 clamp，NaN 归零。 */
  progress: number;
  elapsedMs: number;
  outcome: GenerationOutcome;
  terminal: boolean;
}

export const PHASE_LABEL: Record<GenerationPhaseId, string> = {
  preparing_context: '准备上下文',
  generating_candidates: '候选生成',
  inspecting: '内容审校',
  rewriting: '改写',
  final_output: '最终输出',
  saving_memory: '保存记忆',
  completed: '已完成',
  error: '出错',
  cancelled: '已取消',
};

const PHASE_OUTCOME: Record<GenerationPhaseId, GenerationOutcome> = {
  preparing_context: 'running',
  generating_candidates: 'running',
  inspecting: 'running',
  rewriting: 'running',
  final_output: 'running',
  saving_memory: 'running',
  completed: 'done',
  error: 'failed',
  cancelled: 'cancelled',
};

/**
 * 词汇归一表。键为小写/去分隔符后的 token，值为阶段 id。
 * 兼容三代写法：早期英文同义词、当前中文 label、以及新协议的 snake_case id。
 */
const PHASE_ALIASES: Record<string, GenerationPhaseId> = {
  // completed / error / cancelled 必须排在最前被精确命中，
  // 否则 '已完成' 会被老的模糊匹配吞成 '最终输出'。
  completed: 'completed',
  complete: 'completed',
  done: 'completed',
  success: 'completed',
  已完成: 'completed',
  完成: 'completed',
  error: 'error',
  failed: 'error',
  failure: 'error',
  出错: 'error',
  错误: 'error',
  失败: 'error',
  cancelled: 'cancelled',
  canceled: 'cancelled',
  cancel: 'cancelled',
  已取消: 'cancelled',
  取消: 'cancelled',

  preparing_context: 'preparing_context',
  prepare_context: 'preparing_context',
  loading_context: 'preparing_context',
  准备上下文: 'preparing_context',
  加载上下文: 'preparing_context',
  读取故事: 'preparing_context',
  读取章节: 'preparing_context',

  generating_candidates: 'generating_candidates',
  generate_candidates: 'generating_candidates',
  candidate: 'generating_candidates',
  candidates: 'generating_candidates',
  generating: 'generating_candidates',
  候选生成: 'generating_candidates',
  生成候选: 'generating_candidates',
  生成中: 'generating_candidates',
  生成: 'generating_candidates',

  inspecting: 'inspecting',
  inspect: 'inspecting',
  inspection: 'inspecting',
  review: 'inspecting',
  inspector: 'inspecting',
  内容审校: 'inspecting',
  审校: 'inspecting',
  质检: 'inspecting',

  rewriting: 'rewriting',
  rewrite: 'rewriting',
  revise: 'rewriting',
  改写: 'rewriting',
  润色改写: 'rewriting',
  润色: 'rewriting',

  final_output: 'final_output',
  finalize: 'final_output',
  final: 'final_output',
  最终输出: 'final_output',
  最终: 'final_output',

  saving_memory: 'saving_memory',
  save_memory: 'saving_memory',
  memory: 'saving_memory',
  保存记忆: 'saving_memory',
  记忆: 'saving_memory',
};

/** 子串匹配的长词优先：先匹配 'generating_candidates' 再匹配 'generating'。 */
const PHASE_PATTERNS: { needle: string; phase: GenerationPhaseId }[] = Object.entries(PHASE_ALIASES)
  .map(([needle, phase]) => ({ needle, phase }))
  .sort((a, b) => b.needle.length - a.needle.length);

function normalize(raw: string): string {
  return raw
    .trim()
    .toLowerCase()
    .replace(/[\s-]+/g, '_');
}

/**
 * 把任意后端写法解析成阶段 id。
 * 策略：去空白/lowercase 后精确命中 → 整体子串命中 → 放弃（null）。
 */
export function parseGenerationPhase(raw: string | null | undefined): GenerationPhaseId | null {
  if (!raw) return null;
  const token = normalize(raw);
  const exact = PHASE_ALIASES[token];
  if (exact) return exact;
  // 复合文案（如「改写（第 2 轮）」）：按最长别名做子串匹配
  const hit = PHASE_PATTERNS.find(p => token.includes(p.needle));
  return hit ? hit.phase : null;
}

/**
 * 展示文案映射：其他旧事件（orchestrator-step / agent-stage-update 等）仍
 * 携带中文/英文阶段串。这里复用同一张别名表，避免再抄一份 pattern 表。
 */
export function mapDisplayStage(raw: string | null | undefined): string | null {
  const phase = parseGenerationPhase(raw);
  return phase ? PHASE_LABEL[phase] : null;
}

/**
 * 校验并归一化一次 `generation-status` 事件。
 *
 * 这是唯一允许读取原始 payload 的地方：此后 `outcome` 已是布尔真理，
 * 调用方不需要（也不应该）再比较 phase 字符串。
 */
export function parseGenerationEvent(payload: GenerationStatusPayload): GenerationEvent {
  const rawPhase = typeof payload.phase === 'string' ? payload.phase : '';
  const rawLabel = typeof payload.label === 'string' ? payload.label.trim() : '';
  const message = typeof payload.message === 'string' ? payload.message.trim() : '';

  const phase: ParsedGenerationPhase =
    parseGenerationPhase(rawPhase) ?? parseGenerationPhase(message) ?? 'unknown';

  const outcome: GenerationOutcome = phase === 'unknown' ? 'running' : PHASE_OUTCOME[phase];
  const label = rawLabel || (phase === 'unknown' ? '' : PHASE_LABEL[phase]) || message || '处理中';

  const rawProgress = typeof payload.progress === 'number' ? payload.progress : 0;
  const progress = Number.isFinite(rawProgress) ? Math.min(1, Math.max(0, rawProgress)) : 0;

  return {
    taskId: payload.task_id || 'unknown',
    requestId: payload.request_id || null,
    phase,
    label,
    message: message || label,
    progress,
    elapsedMs: typeof payload.elapsed_ms === 'number' ? payload.elapsed_ms : 0,
    outcome,
    terminal: outcome !== 'running',
  };
}
