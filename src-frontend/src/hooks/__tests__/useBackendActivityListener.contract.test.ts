/**
 * v0.26.50 契约：contract-auto-progress 不得把输入栏拉成「后台运行」。
 */
import { describe, it, expect } from 'vitest';
import { shouldRegisterContractAutoProgress } from '../useBackendActivityListener';

describe('contract-auto-progress UI activity gate (v0.26.50)', () => {
  it('analyzing/generating 等 running 阶段不得注册 activity', () => {
    expect(shouldRegisterContractAutoProgress('analyzing')).toBe(false);
    expect(shouldRegisterContractAutoProgress('generating_master')).toBe(false);
    expect(shouldRegisterContractAutoProgress('saving')).toBe(false);
  });

  it('completed/error 允许收尾（不新建 running）', () => {
    expect(shouldRegisterContractAutoProgress('completed')).toBe(true);
    expect(shouldRegisterContractAutoProgress('error')).toBe(true);
  });
});
