import { describe, it, expect } from 'vitest';
import { genesisMainTimeoutSeconds, watchdogTimeoutSeconds } from '../genesisTimeout';

describe('genesisMainTimeoutSeconds (handleSmartGeneration 主超时)', () => {
  it('创世路径(isBootstrap=true) = 后端 + 30s 缓冲', () => {
    expect(genesisMainTimeoutSeconds(600, 200, true)).toBe(630);
    expect(genesisMainTimeoutSeconds(1800, 200, true)).toBe(1830);
    expect(genesisMainTimeoutSeconds(300, 120, true)).toBe(330);
  });

  it('续写路径(isBootstrap=false) = 前端超时', () => {
    expect(genesisMainTimeoutSeconds(600, 200, false)).toBe(200);
    expect(genesisMainTimeoutSeconds(1800, 900, false)).toBe(900);
  });

  it('未设置时使用默认 600s', () => {
    expect(genesisMainTimeoutSeconds(undefined, undefined, true)).toBe(630);
    expect(genesisMainTimeoutSeconds(undefined, undefined, false)).toBe(600);
  });

  it('创世路径忽略 frontend_timeout_secs（只用后端 + 30s）', () => {
    // 即使前端超时设很小，创世仍用后端 + 30s，保证后端先返回
    expect(genesisMainTimeoutSeconds(600, 60, true)).toBe(630);
    expect(genesisMainTimeoutSeconds(1200, 60, true)).toBe(1230);
  });
});

describe('watchdogTimeoutSeconds (isGenerating 看门狗)', () => {
  it('创世进行中(isBootstrapInProgress=true) = 后端 + 30s 缓冲', () => {
    expect(watchdogTimeoutSeconds(600, 200, true)).toBe(630);
    expect(watchdogTimeoutSeconds(1800, 200, true)).toBe(1830);
  });

  it('非创世(isBootstrapInProgress=false) = 前端超时', () => {
    expect(watchdogTimeoutSeconds(600, 200, false)).toBe(200);
    expect(watchdogTimeoutSeconds(1800, 900, false)).toBe(900);
  });

  it('未设置时使用默认 600s', () => {
    expect(watchdogTimeoutSeconds(undefined, undefined, true)).toBe(630);
    expect(watchdogTimeoutSeconds(undefined, undefined, false)).toBe(600);
  });

  it('创世窗口内（generating/delivered）均走后端 + 30s，与主超时一致', () => {
    // 看门狗与主超时数值必须一致，避免看门狗先于主超时杀掉创世
    for (const be of [300, 600, 1200, 1800]) {
      expect(watchdogTimeoutSeconds(be, 200, true)).toBe(genesisMainTimeoutSeconds(be, 200, true));
    }
  });
});
