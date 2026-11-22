/**
 * v0.99.13：CAS 基线必须与 scene 绑定。
 *
 * 真机证据（v0.99.10 日志包 db37e769）：10 次 `scene_conflict` 全部发生在
 * 切章后 10–50ms，冲突对象都是上一章 scene。旧实现把基线存成一段裸正文，
 * `persistSceneContent` 在 `await prev` 之后才读共享 ref；切章时 selectChapter
 * 已把 ref 换成新章正文 → 上一章的在途保存拿新章指纹去校验旧 scene，后端
 * 正确拒绝，前端却误报「另一窗口已更新」并丢掉上一章最后几次编辑。
 *
 * 同时锁住两条相邻不变量：
 *  - 同一 scene 的保存仍必须携带 DB 基线指纹（另一窗口改动可检出）；
 *  - 视觉空文档（`<p></p>` 等）不写库、不改基线（后端 v0.93.16 会忽略正文
 *    但仍返回命中 1 行，旧实现会把基线写成空稿，之后每次保存都冲突）。
 */
import React, { useRef } from 'react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, act, waitFor } from '@testing-library/react';
import useScenePersistence, { type SceneBaseline } from '../hooks/useScenePersistence';
import { sceneContentHash } from '../updateSceneIpc';
import { loggedInvoke } from '@/services/tauri';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve(undefined)),
}));

vi.mock('@/services/tauri', () => ({
  loggedInvoke: vi.fn(() => Promise.resolve(1)),
}));

const DB_OLD = '<p>DB 旧正文</p>';
const DB_NEW = '<p>新章 DB 正文</p>';

type PersistFn = (sceneId: string, content: string, retryCount?: number) => Promise<void>;

const api: {
  persist?: PersistFn;
  baseline?: React.MutableRefObject<SceneBaseline | null>;
} = {};

function Harness() {
  const editorRef = useRef(null);
  const latestContentRef = useRef('');
  const justSavedRef = useRef(0);
  const baselineRef = useRef<SceneBaseline | null>({ sceneId: 'scene-old', content: DB_OLD });
  const { persistSceneContent } = useScenePersistence({
    editorRef: editorRef as never,
    latestContentRef,
    justSavedRef,
    sceneBaselineContentRef: baselineRef,
    setIsSaved: () => {},
  });
  api.persist = persistSceneContent;
  api.baseline = baselineRef;
  return null;
}

const updateSceneCalls = () =>
  vi.mocked(loggedInvoke).mock.calls.filter(([cmd]) => cmd === 'update_scene');

describe('CAS 基线 scene 绑定（v0.99.13）', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.persist = undefined;
    api.baseline = undefined;
  });

  it('切章后旧 scene 的在途保存仍使用调用时刻的基线，且不覆盖新章基线', async () => {
    let resolveFirst!: (v: number) => void;
    const firstPending = new Promise<number>(r => {
      resolveFirst = r;
    });
    let invoked = 0;
    vi.mocked(loggedInvoke).mockImplementation((cmd: string) => {
      if (cmd === 'update_scene') {
        invoked += 1;
        // 第一笔保存挂在 IPC 上，制造「在途 + 排队」的真实串行链
        return invoked === 1 ? firstPending : Promise.resolve(1);
      }
      return Promise.resolve(undefined);
    });

    render(<Harness />);

    act(() => {
      void api.persist!('scene-old', '<p>第一稿</p>');
    });
    await waitFor(() => expect(updateSceneCalls().length).toBe(1));
    act(() => {
      void api.persist!('scene-old', '<p>第二稿</p>');
    });

    // 切章：selectChapter 把基线换成新章 DB 正文（真机同毫秒发生）
    api.baseline!.current = { sceneId: 'scene-new', content: DB_NEW };

    await act(async () => {
      resolveFirst(1);
      await waitFor(() => expect(updateSceneCalls().length).toBe(2));
    });

    const calls = updateSceneCalls();
    expect((calls[0][1] as { expected_content_hash: string }).expected_content_hash).toBe(
      sceneContentHash(DB_OLD)
    );
    // 修复前：第二笔在 await 之后读到「第一稿」或「新章正文」指纹 → 与 DB 不符 → 误判冲突
    expect((calls[1][1] as { expected_content_hash: string }).expected_content_hash).toBe(
      sceneContentHash(DB_OLD)
    );
    // 旧 scene 的成功保存不得把新章基线改回旧 scene
    expect(api.baseline!.current).toEqual({ sceneId: 'scene-new', content: DB_NEW });
  });

  it('同一 scene 的保存仍携带 DB 基线指纹（另一窗口改动可检出）', async () => {
    render(<Harness />);

    await act(async () => {
      await api.persist!('scene-old', '<p>我的改稿</p>');
    });

    const calls = updateSceneCalls();
    expect(calls.length).toBe(1);
    expect((calls[0][1] as { expected_content_hash: string }).expected_content_hash).toBe(
      sceneContentHash(DB_OLD)
    );
    // 成功后基线刷新为刚写入的内容，下一次保存以它为准
    expect(api.baseline!.current).toEqual({ sceneId: 'scene-old', content: '<p>我的改稿</p>' });
  });

  it('视觉空文档不写库、不改基线（后端会忽略正文但仍算命中 1 行）', async () => {
    render(<Harness />);

    await act(async () => {
      await api.persist!('scene-old', '<p></p>');
      await api.persist!('scene-old', '<p><br></p>');
      await api.persist!('scene-old', '<p>&nbsp;</p>');
    });

    expect(updateSceneCalls().length).toBe(0);
    expect(api.baseline!.current).toEqual({ sceneId: 'scene-old', content: DB_OLD });
  });

  it('基线属于其它 scene 时不发送该指纹（宁可不做 CAS 也不误判冲突）', async () => {
    render(<Harness />);
    api.baseline!.current = { sceneId: 'scene-new', content: DB_NEW };

    await act(async () => {
      await api.persist!('scene-old', '<p>切章残留正文</p>');
    });

    const calls = updateSceneCalls();
    expect(calls.length).toBe(1);
    expect((calls[0][1] as { expected_content_hash: string | null }).expected_content_hash).toBe(
      null
    );
  });
});
