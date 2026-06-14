/**
 * useSyncStore.bug.spec.ts — v5.7 bugfix regression
 *
 * **Validates: Requirements 1.9** (frontend side of C_1_9)
 *
 * 对应 bug 条件:
 *   C_1_9 frontend: DataRefresh { resource_type: 'payoffLedger' }
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
  await new Promise(r => setTimeout(r, 0));
  const cb = listeners.get('sync-event');
  if (cb) {
    cb({ payload: { type, payload } });
  }
}

function hasInvalidationForKey(keyPrefix: string): boolean {
  return invalidateQueriesCalls.some(call => {
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

// v5.6.4: Bug condition fixed — payoffLedgerUpdated case added to useSyncStore
describe('useSyncStore regression (C_1_9 frontend)', () => {
  it('DataRefresh { resource_type: "payoffLedger" } invalidates ["payoff-ledger", ...]', async () => {
    await fc.assert(
      fc.asyncProperty(
        fc.string({ minLength: 1, maxLength: 32 }).filter((s: string) => !!s.trim()),
        async (storyId: string) => {
          invalidateQueriesCalls.length = 0;
          listeners.clear();

          const { unmount } = renderHook(() => useSyncStore());

          // SyncEvent dataRefresh payload 使用 snake_case 字段名
          await dispatchSyncEvent('dataRefresh', {
            story_id: storyId,
            resource_type: 'payoffLedger',
          });

          // 给事件 tick 一轮，确保 case 分支命中
          await new Promise(r => setTimeout(r, 0));

          // 断言：case 'payoffLedger' 会 invalidate 'payoff-ledger' 前缀的 queryKey
          const saw = hasInvalidationForKey('payoff-ledger');

          unmount();

          return saw;
        }
      ),
      { numRuns: 8 }
    );
  });

  it('DataRefresh { resource_type: "all" } batches invalidation into a single invalidateQueries call', async () => {
    invalidateQueriesCalls.length = 0;
    listeners.clear();

    const { unmount } = renderHook(() => useSyncStore());

    const storyId = 'story-all-refresh';
    await dispatchSyncEvent('dataRefresh', {
      story_id: storyId,
      resource_type: 'all',
    });
    await new Promise(r => setTimeout(r, 0));

    // B1 修复：all 分支应只发起一次 predicate invalidateQueries，而非多次独立 key 失效
    expect(invalidateQueriesCalls.length).toBe(1);

    const callArg = invalidateQueriesCalls[0][0] as {
      predicate?: (query: { queryKey: unknown[] }) => boolean;
    };
    expect(callArg.predicate).toBeTypeOf('function');

    // predicate 应命中该 storyId 下的受控 key
    const shouldMatch = [
      ['scenes', storyId],
      ['characters', storyId],
      ['chapters', storyId],
      ['world_building', storyId],
      ['knowledge-graph', storyId],
    ];
    for (const key of shouldMatch) {
      expect(callArg.predicate!({ queryKey: key })).toBe(true);
    }

    // 全局 key（如 stories）无 storyId 过滤，仍应命中
    expect(callArg.predicate!({ queryKey: ['stories'] })).toBe(true);

    // 其他 storyId 不应被该事件失效
    expect(callArg.predicate!({ queryKey: ['scenes', 'other-story'] })).toBe(false);

    unmount();
  });

  it('DataRefresh { resource_type: "all", affected_resources } only invalidates specified resources', async () => {
    invalidateQueriesCalls.length = 0;
    listeners.clear();

    const { unmount } = renderHook(() => useSyncStore());

    const storyId = 'story-affected';
    await dispatchSyncEvent('dataRefresh', {
      story_id: storyId,
      resource_type: 'all',
      affected_resources: ['scenes', 'knowledgeGraph'],
    });
    await new Promise(r => setTimeout(r, 0));

    expect(invalidateQueriesCalls.length).toBe(1);

    const callArg = invalidateQueriesCalls[0][0] as {
      predicate?: (query: { queryKey: unknown[] }) => boolean;
    };
    expect(callArg.predicate).toBeTypeOf('function');

    expect(callArg.predicate!({ queryKey: ['scenes', storyId] })).toBe(true);
    expect(callArg.predicate!({ queryKey: ['knowledge-graph', storyId] })).toBe(true);
    expect(callArg.predicate!({ queryKey: ['characters', storyId] })).toBe(false);
    expect(callArg.predicate!({ queryKey: ['chapters', storyId] })).toBe(false);

    unmount();
  });
});
