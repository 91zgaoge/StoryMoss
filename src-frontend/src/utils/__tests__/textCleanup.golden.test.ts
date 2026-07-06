import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { trimSelfRepetition } from '../textCleanup';

// v0.26.19 Phase 3.3: 跨层共享 trim golden fixture。
//
// 此测试加载仓库根 `tests/fixtures/trim_golden.json`（与 Rust
// `trim_self_repetition_matches_shared_golden_fixture` 测试加载的同一文件），
// 对每条用例断言 TS `trimSelfRepetition` 输出与 expected 一致。
//
// 双跑通过即证明 Rust `trim_self_repetition` 与 TS `trimSelfRepetition` 对同输入
// 同输出（跨层一致性契约）。这是 v0.26.16「Rust trim 对齐前端 KMP」的回归守卫——
// 任一实现漂移都会让对应层的 golden 测试失败。

interface TrimGoldenCase {
  id: string;
  description?: string;
  input: string;
  expected: string;
}

const __dirname = dirname(fileURLToPath(import.meta.url));
// 从 src-frontend/src/utils/__tests/ 回到仓库根：../../../../
// → src-frontend/src/utils/ → src-frontend/src/ → src-frontend/ → <root>
const fixturePath = resolve(
  __dirname,
  '..',
  '..',
  '..',
  '..',
  'tests',
  'fixtures',
  'trim_golden.json'
);
const fixtureJson = readFileSync(fixturePath, 'utf-8');
const cases: TrimGoldenCase[] = JSON.parse(fixtureJson);

describe('trimSelfRepetition — shared golden fixture (cross-layer with Rust)', () => {
  it('fixture loads with at least 7 cases', () => {
    expect(cases.length).toBeGreaterThanOrEqual(7);
  });

  for (const c of cases) {
    it(`golden case '${c.id}' matches expected output`, () => {
      const actual = trimSelfRepetition(c.input);
      expect(actual).toBe(c.expected);
    });
  }
});
