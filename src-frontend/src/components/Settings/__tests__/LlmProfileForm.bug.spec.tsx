/**
 * LlmProfileForm.bug.spec.tsx — v5.7 bugfix exploration
 *
 * **Validates: Requirements 1.11** (C_1_11)
 *
 * CRITICAL: 此测试在 **未修复** 代码上必须 PASS —— "pass" = 探索成功 = bug 确实存在。
 * 修复 (Task 7.2) 后，此测试应翻转为 FAIL。
 *
 * 对应 bug 条件:
 *   C_1_11: UI 允许用户创建 `image` 类型 LLM Profile，
 *           不给出"实验性 / 暂未实现"警告，也不在表单层阻止提交；
 *           后端 `test_model_connection` 才硬编码返回"图像生成模型暂未实现"。
 *
 * 由于当前 `ModelModal` 组件嵌入在 `src/pages/Settings.tsx` 内（未拆出独立
 * `LlmProfileForm.tsx` 组件），我们通过 **源码静态分析** 来坐实 bug：
 * Settings.tsx 中 `image` 类型的 tab / form 区域既不含 disabled 标记，
 * 也不含 "实验性" / "暂未实现" / "unsupported" 文案。
 */

import { describe, it, expect } from 'vitest';
import * as fs from 'fs';
import * as path from 'path';

function readSettingsSource(): string {
  const candidates = [
    path.resolve(process.cwd(), 'src/pages/Settings.tsx'),
    path.resolve(process.cwd(), '..', 'src-frontend/src/pages/Settings.tsx'),
    path.resolve(__dirname, '../../../pages/Settings.tsx'),
  ];
  for (const c of candidates) {
    if (fs.existsSync(c)) {
      return fs.readFileSync(c, 'utf-8');
    }
  }
  throw new Error(`Could not locate Settings.tsx. Tried: ${candidates.join(' | ')}`);
}

function readLlmTypesSource(): string {
  const candidates = [
    path.resolve(process.cwd(), 'src/types/llm.ts'),
    path.resolve(process.cwd(), '..', 'src-frontend/src/types/llm.ts'),
    path.resolve(__dirname, '../../../types/llm.ts'),
  ];
  for (const c of candidates) {
    if (fs.existsSync(c)) {
      return fs.readFileSync(c, 'utf-8');
    }
  }
  throw new Error(`Could not locate types/llm.ts. Tried: ${candidates.join(' | ')}`);
}

describe('LlmProfileForm bug exploration (C_1_11)', () => {
  it('image provider_type is still declared as a valid ModelType (bug surface)', () => {
    // 先确认 "image" 确实是合法的 ModelType —— UI 才有入口创建 image Profile
    const llmTypes = readLlmTypesSource();
    expect(
      llmTypes.includes("'image'") || llmTypes.includes('"image"'),
    ).toBe(true);
  });

  it('Settings.tsx does NOT warn / block image-type profile creation', () => {
    const src = readSettingsSource();

    // 1) 确认 image tab 存在
    expect(
      src.includes("activeTab === 'image'") ||
        src.includes('activeTab==="image"'),
    ).toBe(true);

    // 2) 不应存在任何"实验性 / 暂未实现 / 敬请期待 / unsupported / unsupported_type / dead_end"
    //    级别的文案或守卫。如果任何一个出现，说明 UI 已经修复了死胡同。
    const warningMarkers = [
      '实验性',
      '暂未实现',
      '敬请期待',
      'unsupported_type',
      'unsupported type',
      'image_disabled',
      'image-disabled',
      'image_experimental',
    ];
    const found = warningMarkers.filter((m) => src.includes(m));
    // 期望 found 为空 → bug 成立（UI 没警告）
    expect(found).toEqual([]);
  });

  it('Settings.tsx image tab has no provider_type gate blocking submit', () => {
    const src = readSettingsSource();

    // onSubmit 函数里不应因为 type === 'image' 而走 toast / early-return 分支
    // 简单做法：统计 `type === 'image'` 出现次数，然后检查周围上下文是否含 toast/阻止
    const marker = "type === 'image'";
    const positions: number[] = [];
    let idx = 0;
    while ((idx = src.indexOf(marker, idx)) !== -1) {
      positions.push(idx);
      idx += marker.length;
    }

    // 至少存在一处（渲染分支）
    expect(positions.length).toBeGreaterThan(0);

    // 针对每一处，取前后 200 字符窗口，检查是否存在 toast.error 或 early return
    let guardedCount = 0;
    for (const p of positions) {
      const window = src.slice(Math.max(0, p - 100), Math.min(src.length, p + 300));
      if (
        window.includes('toast.error') ||
        window.includes('return;  // block image') ||
        window.includes('return; // block image') ||
        window.includes('image 类型暂未实现')
      ) {
        guardedCount += 1;
      }
    }
    // 期望所有 image 分支都未加守卫
    expect(guardedCount).toBe(0);
  });
});
