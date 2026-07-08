import { describe, it, expect } from 'vitest';
import { readFileSync } from 'fs';
import { resolve } from 'path';

import {
  parseGenesisStepsJson,
  countGenesisErrors,
  computeGenesisDisplaySteps,
  mergeGenesisProgress,
  GENESIS_STEP_NAMES,
  hasGenesisErrors,
  sortGenesisErrors,
  computeGenesisProgressPercent,
  type GenesisStepError,
} from '../genesisSteps';

interface TestGenesisRun {
  id: string;
  story_id?: string;
  session_id: string;
  premise: string;
  status: string;
  current_step?: string;
  current_step_number: number;
  total_steps: number;
  steps_json: string;
  error_message?: string;
  created_at: string;
  updated_at: string;
}

const fixturePath = resolve(__dirname, '../../../../tests/fixtures/genesis_run_steps_sample.json');
const fixture: TestGenesisRun = JSON.parse(readFileSync(fixturePath, 'utf8'));

describe('genesisSteps', () => {
  describe('parseGenesisStepsJson', () => {
    it('parses errors and per-step overrides from fixture', () => {
      const { stepMap, errors } = parseGenesisStepsJson(fixture.steps_json);

      expect(errors).toHaveLength(2);
      expect(errors[0]).toEqual({
        step: '构建世界与骨架',
        message: '世界观规则更新失败: db locked',
        severity: 'warning',
      });
      expect(errors[1]).toEqual({
        step: '播种故事合同',
        message: 'MASTER_SETTING 合同创建失败: invalid story_id',
        severity: 'error',
      });

      expect(stepMap['构思故事']).toEqual({
        status: 'completed',
        message: '故事概念已生成：《霓虹侦探》',
        output: undefined,
      });
      expect(stepMap['选择创作策略']).toEqual({
        status: 'completed',
        message: '已选择创作策略',
        output: undefined,
      });
      expect(stepMap['撰写开篇']).toEqual({
        status: 'completed',
        message: '第一章正文已生成',
        output: undefined,
      });
    });

    it('returns empty results for null or empty input', () => {
      expect(parseGenesisStepsJson(undefined)).toEqual({ stepMap: {}, errors: [] });
      expect(parseGenesisStepsJson('')).toEqual({ stepMap: {}, errors: [] });
      expect(parseGenesisStepsJson('{}')).toEqual({ stepMap: {}, errors: [] });
    });

    it('survives malformed JSON', () => {
      expect(parseGenesisStepsJson('not json')).toEqual({ stepMap: {}, errors: [] });
    });
  });

  describe('countGenesisErrors', () => {
    it('counts warnings and errors separately', () => {
      const counts = countGenesisErrors(fixture.steps_json);
      expect(counts.total).toBe(2);
      expect(counts.warnings).toBe(1);
      expect(counts.errors).toBe(1);
    });

    it('returns zero for empty input', () => {
      expect(countGenesisErrors(undefined)).toEqual({
        total: 0,
        warnings: 0,
        errors: 0,
      });
    });
  });

  describe('hasGenesisErrors', () => {
    it('detects errors in steps_json', () => {
      expect(hasGenesisErrors(fixture.steps_json)).toBe(true);
    });

    it('returns false when no errors recorded', () => {
      expect(hasGenesisErrors('{}')).toBe(false);
    });
  });

  describe('computeGenesisDisplaySteps', () => {
    it('uses canonical 8 backend step names', () => {
      const steps = computeGenesisDisplaySteps(fixture);
      expect(steps).toHaveLength(8);
      expect(steps.map(s => s.name)).toEqual(GENESIS_STEP_NAMES);
    });

    it('marks all steps completed for a completed run', () => {
      const steps = computeGenesisDisplaySteps(fixture);
      expect(steps.every(s => s.status === 'completed')).toBe(true);
      expect(steps[0].message).toBe('故事概念已生成：《霓虹侦探》');
    });

    it('marks prior steps completed and current step running', () => {
      const run: TestGenesisRun = {
        ...fixture,
        status: 'running',
        current_step: '选择创作策略',
        current_step_number: 2,
      };
      const steps = computeGenesisDisplaySteps(run);

      expect(steps[0].status).toBe('completed');
      expect(steps[1].status).toBe('running');
      expect(steps[1].name).toBe('选择创作策略');
      expect(steps.slice(2).every(s => s.status === 'pending')).toBe(true);
    });

    it('marks current step failed for a failed run', () => {
      const run: TestGenesisRun = {
        ...fixture,
        status: 'failed',
        current_step: '构建世界与骨架',
        current_step_number: 4,
      };
      const steps = computeGenesisDisplaySteps(run);

      expect(steps.slice(0, 3).every(s => s.status === 'completed')).toBe(true);
      expect(steps[3].status).toBe('failed');
      expect(steps[3].name).toBe('构建世界与骨架');
      expect(steps.slice(4).every(s => s.status === 'pending')).toBe(true);
    });

    it('uses step overrides from steps_json when status heuristic is absent', () => {
      const run: TestGenesisRun = {
        ...fixture,
        status: 'unknown_status',
        current_step: undefined,
      };
      const steps = computeGenesisDisplaySteps(run);

      expect(steps[0].status).toBe('completed');
      expect(steps[1].status).toBe('completed');
      expect(steps[2].status).toBe('completed');
      expect(steps[3].status).toBe('pending');
    });
  });

  describe('mergeGenesisProgress', () => {
    it('updates the matching step from a live progress event', () => {
      const steps = computeGenesisDisplaySteps({
        ...fixture,
        status: 'running',
        current_step: '构建世界与骨架',
      });

      const merged = mergeGenesisProgress(steps, {
        stepName: '构建世界与骨架',
        status: 'running',
        message: '正在生成世界观...',
        progressPercent: 42,
      });

      const target = merged.find(s => s.name === '构建世界与骨架');
      expect(target?.status).toBe('running');
      expect(target?.message).toBe('正在生成世界观...');
    });

    it('ignores progress events for unknown steps', () => {
      const steps = computeGenesisDisplaySteps(fixture);
      const merged = mergeGenesisProgress(steps, {
        stepName: '不存在的步骤',
        status: 'running',
        message: '...',
      });
      expect(merged).toEqual(steps);
    });
  });

  describe('computeGenesisProgressPercent', () => {
    it('returns 100 when all steps completed', () => {
      const steps = computeGenesisDisplaySteps(fixture);
      expect(computeGenesisProgressPercent(steps)).toBe(100);
    });

    it('returns a partial percentage when a step is running', () => {
      const steps = computeGenesisDisplaySteps({
        ...fixture,
        status: 'running',
        current_step: '选择创作策略',
      });
      const pct = computeGenesisProgressPercent(steps);
      expect(pct).toBeGreaterThan(0);
      expect(pct).toBeLessThan(100);
    });

    it('returns 0 for an empty step list', () => {
      expect(computeGenesisProgressPercent([])).toBe(0);
    });
  });

  describe('sortGenesisErrors', () => {
    it('sorts errors before warnings and by canonical step order', () => {
      const errors: GenesisStepError[] = [
        { step: '埋设伏笔', message: 'a', severity: 'warning' },
        { step: '构思故事', message: 'b', severity: 'warning' },
        { step: '知识图谱', message: 'c', severity: 'error' },
        { step: '选择创作策略', message: 'd', severity: 'error' },
      ];
      const sorted = sortGenesisErrors(errors);
      expect(sorted.map(e => `${e.step}:${e.severity}`)).toEqual([
        '选择创作策略:error',
        '知识图谱:error',
        '构思故事:warning',
        '埋设伏笔:warning',
      ]);
    });
  });
});
