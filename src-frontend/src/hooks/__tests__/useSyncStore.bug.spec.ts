/**
 * useSyncStore.bug.spec.ts — v5.7 bugfix exploration
 *
 * **Validates: Requirements 1.9** (frontend side of C_1_9)
 *
 * CRITICAL: 此测试在 **未修复** 代码上必须 PASS
 * —— "pass" 语义是"探索成功"（即 bug 确实存在）。
 * 修复 useSyncStore 后（Task 4.5），此测试应翻转为 FAIL。
 *
 * 对应 bug 条件:
 *   C_1_9 frontend: DataRefresh { resourceType: 'payoffLedger' }
 *                   永远不会触发 invalidateQueries(['payoff-ledger', ...])
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import fc from 'fast-check';

// =========================================================
// 模拟 @tauri-apps/api/event.listen + @tanstack/react-query
// =========================================================

type Listener = (event: { payload: unknown }) => void;
const listeners = new Map<string, Listener>();

vi.mock('@tauri-apps/api/event', () => {
  return {
    listen: async (channel: string, cb: Listener) => {
      listeners.set(channel, cb);
      return () => {
        listeners.delete(channel);
      };
    },
  };
});

// 记录所有 invalidateQueries 调用
const invalidateQueriesCalls: unknown[][] = [];
const removeQueriesCalls: unknown[][] = [];

const mockQueryClient = {
  invalidateQueries: (args: unknown) => {
    invalidateQueriesCalls.push([args]);
  },
  removeQueries: (args: unknown) => {
    removeQueriesCalls.push([args]);
  },
};

vi.mock('@tanstack/react-query', () => {
  return {
    useQueryClient: () => mockQueryClient,
  };
});

// React Hook 测试需要 renderHook —— 使用 @testing-library/react
import { renderHook } from '@testing-library/react';
// 动态 import 以便 mock 先于求值生效
let useSyncStore: typeof import('../useSyncStore').useSyncStore;

beforeEach(async () => {
  listeners.clear();
  invalidateQueriesCalls.length = 0;
  removeQueriesCalls.length = 0;
  const mod = await import('../useSyncStore');
  useSyncStore = mod.useSyncStore;
});

afterEach(() => {
  listeners.clear();
});

async function dispatchSyncEvent(type: string, payload: Record<string, unknown>) {
  // 等待 useEffect 的 setup 完成（async listen）
  await new Promise((r) => setTimeout(r, 0));
  const cb = listeners.get('sync-event');
  if (cb) {
    cb({ payload: { type, payload } });
  }
}

function hasInvalidationForKey(keyPrefix: string): boolean {
  return invalidateQueriesCalls.some((call) => {
    const arg = call[0] as { queryKey?: unknown[] } | unknown[] | undefined;
    let key: unknown[] | undefined;
    if (Array.isArray(arg)) {
      key = arg;
    } else if (arg && typeof arg === 'object' && 'queryKey' in arg) {
      key = (arg as { queryKey?: unknown[] }).queryKey;
    }
    return Array.isArray(key) && key[0] === keyPrefix;
  });
}

describe('useSyncStore bug exploration (C_1_9 frontend)', () => {
  it('DataRefresh { resourceType: "payoffLedger" } NEVER invalidates ["payoff-ledger", ...]', async () => {
    await fc.assert(
      fc.asyncProperty(
        fc.string({ minLength: 1, maxLength: 32 }).filter((s: string) => !!s.trim()),
        async (storyId: string) => {
          invalidateQueriesCalls.length = 0;
          listeners.clear();

          const { unmount } = renderHook(() => useSyncStore());

          await dispatchSyncEvent('dataRefresh', {
            storyId,
            resourceType: 'payoffLedger',
          });

          // 给事件 tick 一轮，确保 case 分支命中
          await new Promise((r) => setTimeout(r, 0));

          // 断言：未修复代码中 case 'payoffLedger' 不存在
          // → 不会 invalidate 'payoff-ledger' 前缀的 queryKey
          const saw = hasInvalidationForKey('payoff-ledger');

          unmount();

          // 期望 !saw == true（bug 存在）
          // 如果 saw == true（bug 修复了），这里会 return false 让 fast-check 报错
          return !saw;
        },
      ),
      { numRuns: 8 },
    );
  });
});
