import { renderHook, waitFor, act } from '@testing-library/react';
import { listen } from '@tauri-apps/api/event';
import { useQueryClient } from '@tanstack/react-query';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useSyncStore } from '../useSyncStore';

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
}));

vi.mock('@tanstack/react-query', () => ({
  useQueryClient: vi.fn(() => ({
    invalidateQueries: vi.fn(),
    removeQueries: vi.fn(),
  })),
}));

type EventCallback = (event: { payload: any }) => void;

describe('useSyncStore story-selected contract', () => {
  let handler: EventCallback | null = null;

  beforeEach(() => {
    handler = null;
    vi.mocked(listen).mockImplementation(((_name: string, callback: EventCallback) => {
      handler = callback;
      return Promise.resolve(() => {});
    }) as any);
  });

  it('passes seq/origin to onStorySelected (v0.79.0)', async () => {
    const onStorySelected = vi.fn();
    renderHook(() => useSyncStore({ onStorySelected }));

    await waitFor(() => {
      expect(listen).toHaveBeenCalled();
      expect(handler).not.toBeNull();
    });

    act(() => {
      handler?.({
        payload: {
          type: 'storySelected',
          payload: {
            story_id: 'story-b',
            title: '乙',
            seq: 7,
            origin: 'backstage_card',
          },
        },
      });
    });

    expect(onStorySelected).toHaveBeenCalledWith('story-b', '乙', 7, 'backstage_card');
    expect(vi.mocked(useQueryClient)).toHaveBeenCalled();
  });

  it('defaults seq/origin for legacy events', async () => {
    const onStorySelected = vi.fn();
    renderHook(() => useSyncStore({ onStorySelected }));

    await waitFor(() => expect(handler).not.toBeNull());

    act(() => {
      handler?.({
        payload: {
          type: 'storySelected',
          payload: { story_id: 'story-a', title: '甲' },
        },
      });
    });

    expect(onStorySelected).toHaveBeenCalledWith('story-a', '甲', 0, 'legacy');
  });
});
