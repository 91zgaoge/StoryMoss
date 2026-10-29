import { describe, expect, it, vi, beforeEach } from 'vitest';

const { errorSpy } = vi.hoisted(() => ({ errorSpy: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@/utils/logger', () => ({
  createLogger: () => ({
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: errorSpy,
  }),
}));

import { invoke } from '@tauri-apps/api/core';
import { loggedInvoke } from '../core';

const lastErrorMessage = () => String(errorSpy.mock.calls[0]?.[0] ?? '');

describe('loggedInvoke 失败日志', () => {
  beforeEach(() => {
    errorSpy.mockReset();
    vi.mocked(invoke).mockReset();
  });

  it('字符串拒绝值（Tauri 命令不存在等）原样进日志行', async () => {
    vi.mocked(invoke).mockRejectedValue('Command foo not found');

    await expect(loggedInvoke('foo')).rejects.toBe('Command foo not found');

    expect(errorSpy).toHaveBeenCalledTimes(1);
    expect(lastErrorMessage()).toContain('✗ foo failed');
    expect(lastErrorMessage()).toContain('Command foo not found');
  });

  it('Error 对象取 message', async () => {
    vi.mocked(invoke).mockRejectedValue(new Error('数据库已锁定'));

    await expect(loggedInvoke('save_scene')).rejects.toBeInstanceOf(Error);

    expect(lastErrorMessage()).toContain('数据库已锁定');
  });

  it('结构化错误序列化进日志行', async () => {
    vi.mocked(invoke).mockRejectedValue({ code: 'NotFound', message: '故事不存在' });

    await expect(loggedInvoke('get_canonical_state')).rejects.toEqual({
      code: 'NotFound',
      message: '故事不存在',
    });

    expect(lastErrorMessage()).toContain('故事不存在');
  });

  it('超长原因截断，避免刷屏', async () => {
    vi.mocked(invoke).mockRejectedValue('x'.repeat(1200));

    await expect(loggedInvoke('big')).rejects.toBe('x'.repeat(1200));

    expect(lastErrorMessage()).toContain('...');
    expect(lastErrorMessage().length).toBeLessThan(400);
  });
});
