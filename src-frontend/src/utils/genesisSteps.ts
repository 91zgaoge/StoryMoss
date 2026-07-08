import type { GenesisRun } from '@/services/tauri';

export type GenesisStepStatus = 'pending' | 'running' | 'completed' | 'failed' | 'skipped';

export interface GenesisStepData {
  name: string;
  status: GenesisStepStatus;
  message?: string;
  output?: string;
}

export interface GenesisStepError {
  step: string;
  message: string;
  severity: 'warning' | 'error';
}

export interface GenesisProgressEvent {
  stepName: string;
  status: string;
  message: string;
  progressPercent?: number;
}

/** Backend Quick phase step names. */
export const QUICK_PHASE_STEP_NAMES = [
  '构思故事',
  '选择创作策略',
  '撰写开篇',
] as const;

/** Backend Background phase step names. */
export const BACKGROUND_PHASE_STEP_NAMES = [
  '构建世界与骨架',
  '场景规划',
  '埋设伏笔',
  '知识图谱',
  '播种故事合同',
] as const;

/** Canonical Genesis step order (Quick 3 + Background 5). */
export const GENESIS_STEP_NAMES: readonly string[] = [
  ...QUICK_PHASE_STEP_NAMES,
  ...BACKGROUND_PHASE_STEP_NAMES,
];

function normalizeStatus(value: unknown): GenesisStepStatus | undefined {
  const s = typeof value === 'string' ? value : undefined;
  if (!s) return undefined;
  switch (s) {
    case 'running':
    case 'completed':
    case 'failed':
    case 'skipped':
    case 'pending':
      return s;
    default:
      return undefined;
  }
}

function coerceSeverity(value: unknown): 'warning' | 'error' {
  return value === 'error' ? 'error' : 'warning';
}

/**
 * Parse `genesis_runs.steps_json` into per-step overrides and a list of
 * non-fatal step errors.
 *
 * Supports:
 * - `errors`: array of `{ step, message, severity }` (v0.26.19 Phase 2.2)
 * - Legacy `step_N` objects left by older clients
 * - Objects keyed by step name
 */
export function parseGenesisStepsJson(stepsJson: string | undefined | null): {
  stepMap: Record<string, Partial<GenesisStepData>>;
  errors: GenesisStepError[];
} {
  const stepMap: Record<string, Partial<GenesisStepData>> = {};
  const errors: GenesisStepError[] = [];

  if (!stepsJson) {
    return { stepMap, errors };
  }

  try {
    const parsed = JSON.parse(stepsJson);
    if (!parsed || typeof parsed !== 'object') {
      return { stepMap, errors };
    }

    if (Array.isArray(parsed.errors)) {
      for (const item of parsed.errors) {
        if (!item || typeof item !== 'object') continue;
        errors.push({
          step: String(item.step ?? ''),
          message: String(item.message ?? ''),
          severity: coerceSeverity(item.severity),
        });
      }
    }

    Object.entries(parsed).forEach(([key, value]) => {
      if (key === 'errors') return;
      if (!value || typeof value !== 'object') return;

      let name: string | undefined;
      if (key.startsWith('step_')) {
        const idx = Number(key.replace('step_', ''));
        if (!Number.isNaN(idx)) {
          name = GENESIS_STEP_NAMES[idx] ?? (value as { name?: string }).name ?? `步骤 ${idx + 1}`;
        }
      } else {
        name = key;
      }

      if (name) {
        const v = value as { status?: unknown; message?: unknown; output?: unknown };
        stepMap[name] = {
          status: normalizeStatus(v.status),
          message: typeof v.message === 'string' ? v.message : undefined,
          output: typeof v.output === 'string' ? v.output : undefined,
        };
      }
    });
  } catch {
    // Malformed JSON is treated as empty; caller can fall back to status heuristics.
  }

  return { stepMap, errors };
}

/**
 * Count non-fatal errors recorded in `steps_json`.
 */
export function countGenesisErrors(stepsJson: string | undefined | null): {
  total: number;
  warnings: number;
  errors: number;
} {
  const { errors } = parseGenesisStepsJson(stepsJson);
  return {
    total: errors.length,
    warnings: errors.filter(e => e.severity === 'warning').length,
    errors: errors.filter(e => e.severity === 'error').length,
  };
}

/**
 * Merge a live progress event into a computed step list.
 */
export function mergeGenesisProgress(
  steps: GenesisStepData[],
  progress: GenesisProgressEvent | null | undefined
): GenesisStepData[] {
  if (!progress) return steps;

  return steps.map(step => {
    if (step.name !== progress.stepName) return step;
    return {
      ...step,
      status: normalizeStatus(progress.status) ?? step.status,
      message: progress.message || step.message,
    };
  });
}

function stepNameIndex(name: string): number {
  const idx = GENESIS_STEP_NAMES.indexOf(name);
  return idx >= 0 ? idx : Number.MAX_SAFE_INTEGER;
}

function isTerminalFailure(status: string): boolean {
  return status === 'failed';
}

function isActiveRun(status: string): boolean {
  return status === 'running' || status === 'pending' || status === 'quick_done';
}

/**
 * Compute the full display step list for a Genesis run.
 *
 * The canonical 8-step order (Quick 3 + Background 5) is used regardless of how
 * backend step numbers are emitted, because the DB `current_step` stores the step *name*.
 */
export function computeGenesisDisplaySteps(
  run: GenesisRun,
  progress?: GenesisProgressEvent | null
): GenesisStepData[] {
  const total = run.total_steps ?? 0;
  const names: string[] = [];
  for (let i = 0; i < total; i++) {
    names.push(GENESIS_STEP_NAMES[i] ?? `步骤 ${i + 1}`);
  }

  const { stepMap } = parseGenesisStepsJson(run.steps_json);
  const currentName = progress?.stepName ?? run.current_step ?? undefined;
  const currentIdx = currentName ? names.indexOf(currentName) : -1;

  let steps: GenesisStepData[] = names.map((name, idx) => {
    const override = stepMap[name];
    let status: GenesisStepStatus = override?.status ?? 'pending';

    if (run.status === 'completed') {
      status = override?.status ?? 'completed';
    } else if (isTerminalFailure(run.status)) {
      if (currentIdx >= 0) {
        if (idx < currentIdx) status = 'completed';
        else if (idx === currentIdx) status = 'failed';
        else status = 'pending';
      } else if (override?.status) {
        status = override.status;
      } else {
        status = 'pending';
      }
    } else if (isActiveRun(run.status)) {
      if (currentIdx >= 0) {
        if (idx < currentIdx) status = 'completed';
        else if (idx === currentIdx) status = 'running';
        else status = 'pending';
      } else if (override?.status) {
        status = override.status;
      }
    }

    return {
      name,
      status,
      message: override?.message,
      output: override?.output,
    };
  });

  if (progress) {
    steps = mergeGenesisProgress(steps, progress);
  }

  return steps;
}

/**
 * Compute a 0-100 progress percentage from computed display steps.
 */
export function computeGenesisProgressPercent(steps: GenesisStepData[]): number {
  if (steps.length === 0) return 0;
  const completed = steps.filter(s => s.status === 'completed').length;
  const running = steps.filter(s => s.status === 'running').length;
  const partial = running > 0 ? 0.5 : 0;
  return Math.min(100, Math.round(((completed + partial) / steps.length) * 100));
}

/**
 * Return true if the run recorded any non-fatal step errors.
 */
export function hasGenesisErrors(stepsJson: string | undefined | null): boolean {
  return countGenesisErrors(stepsJson).total > 0;
}

/**
 * Sort errors so errors (high severity) appear first, then by step order.
 */
export function sortGenesisErrors(errors: GenesisStepError[]): GenesisStepError[] {
  return [...errors].sort((a, b) => {
    if (a.severity !== b.severity) {
      return a.severity === 'error' ? -1 : 1;
    }
    return stepNameIndex(a.step) - stepNameIndex(b.step);
  });
}
