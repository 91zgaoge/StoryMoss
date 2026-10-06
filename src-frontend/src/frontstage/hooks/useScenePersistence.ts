/**
 * 场景持久化链 — 从 FrontstageApp.tsx 机械抽取（零行为变更）。
 *
 * 负责：
 *  - persistSceneContent：序列化 DB 写入——所有 update_scene 必经此函数，按调用顺序串行提交；
 *  - flushSceneSave：取消待执行的防抖保存，立即将最新内容落库；
 *  - 失败可见态与封顶退避重试（2s/10s/30s），顶栏「保存失败，点击重试」入口。
 *
 * 注：logToBackend / frontstageLogger 与 FrontstageApp.tsx 中的同名常量同源同配置，
 * 抽取时保留副本以维持日志 phase 与 logger 名不变（零行为变更）。
 */

import { useCallback, useEffect, useRef, useState } from 'react';
import type { MutableRefObject, RefObject } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { loggedInvoke } from '@/services/tauri';
import { createLogger } from '@/utils/logger';
import { cancelAutoSave } from '../autoSave';
import { buildUpdateSceneIpcArgs } from '../updateSceneIpc';
import { useFrontstageStore } from '../store/frontstageStore';
import type { RichTextEditorRef } from '../components/RichTextEditor';

const frontstageLogger = createLogger('ui:FrontstageApp');

// v0.23.89: 前端关键事件直接写入后端工作流日志，便于无 devtools 时定位问题
const logToBackend = (phase: string, message: string, details?: Record<string, unknown>) => {
  try {
    invoke('log_frontend_event', {
      phase,
      message,
      details: details ?? {},
    }).catch(() => {});
  } catch {
    // ignore
  }
};

// v0.33.x: persistSceneContent 失败重试退避（2s/10s/30s），重试耗尽后置 saveError 可见态；
// 重试出火时若 store sceneId 已切换（如自动分章）则 no-op，避免旧全文回写已截断的旧 scene
const SAVE_RETRY_DELAYS_MS = [2000, 10000, 30000];

/**
 * v0.59.2：编辑器「视觉空文档」判定。
 *
 * ProseMirror 的空文档序列化是 `<p></p>` / `<p><br></p>` 这类**非空字符串**，
 * 早先的 `if (!content) return` 只挡得住 `''`，挡不住「正文尚未到达时编辑器
 * 自带的空文档」——它会被 2s 防抖保存原样落库，覆盖后端已有正文
 * （`e2e/frontstage-editing.spec.ts`「自动保存持久化」可稳定复现）。
 */
export function isEmptyEditorHtml(html: string): boolean {
  return (
    html
      .replace(/<[^>]*>/g, '')
      .replace(/&nbsp;/g, ' ')
      .trim().length === 0
  );
}

export interface UseScenePersistenceParams {
  /** 编辑器引用——flushSceneSave 直接读取编辑器实际 HTML，而非 latestContentRef */
  editorRef: RefObject<RichTextEditorRef>;
  /** 组件内多处读写的最新内容快照 */
  latestContentRef: MutableRefObject<string>;
  /** 组件内 onChapterUpdated / ensureUntitledStory 也读写的「刚保存」时间戳 */
  justSavedRef: MutableRefObject<number>;
  /** store 保存状态写入（组件内包装 setSaveStatus） */
  setIsSaved: (saved: boolean) => void;
}

export default function useScenePersistence({
  editorRef,
  latestContentRef,
  justSavedRef,
  setIsSaved,
}: UseScenePersistenceParams) {
  // v0.30.34: 序列化场景持久化链 - 确保 update_scene 调用串行执行，消除
  // 并发全量覆写竞态（last-write-wins：较早的小内容覆写较晚的大内容）。
  // 文思活跃连续续写时多次 appendAiContent 各自 fire-and-forget flushSceneSave，
  // 若不序列化，spawn_blocking 线程池上 SQLite 写锁获取顺序非 FIFO，
  // 较早的 flush（小内容）可能在较晚的 flush（大内容）之后提交，静默覆写。
  const saveChainRef = useRef<Promise<void>>(Promise.resolve());

  // v0.33.x: 保存失败可见态——重试耗尽后顶栏显示「保存失败，点击重试」，
  // 此前失败后 isSaved=false 永远停在「保存中...」，用户无从感知正文未落库。
  const [saveError, setSaveError] = useState<string | null>(null);
  // 记录本轮是否发生过失败，用于成功后的恢复日志与 saveError 清理
  const saveFailedRef = useRef(false);
  // flushSceneSave 入口日志仅在 sceneId 变化时记录一次，避免每次 flush 刷量
  const lastFlushLoggedSceneIdRef = useRef<string | null>(null);
  // v0.33.x: 在途失败重试定时器——分章切换等场景可取消持有旧正文的在途重试
  const saveRetryTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // v0.59.2: 载入期空写保护。后端正文（非空）刚载入时布防；布防期内遇到
  // 「视觉空文档」的保存一律跳过并记日志——那是编辑器挂载自带的空 doc，
  // 不是用户清空。一旦出现非空保存（说明用户真的在写），保护自动解除，
  // 因此「用户删光正文」在正常写作后仍可落库；只有「载入后未编辑就清空」
  // 这一种情况被挡住（宁可保留旧文，也不静默清空整章）。
  const loadGuardRef = useRef<{ sceneId: string; armed: boolean }>({
    sceneId: '',
    armed: false,
  });
  const markSceneContentLoaded = useCallback((sceneId: string, hadContent: boolean) => {
    loadGuardRef.current = { sceneId, armed: hadContent };
  }, []);
  // v0.33.x: 取消在途重试（分章自动切换时调用）——重试闭包持有分章前旧全文，
  // 出火后会把旧全文回写到已截断的旧 scene，造成"旧全文 + 新章溢出副本"重复
  const cancelPersistRetry = useCallback(() => {
    if (saveRetryTimerRef.current) {
      clearTimeout(saveRetryTimerRef.current);
      saveRetryTimerRef.current = null;
    }
  }, []);

  // 序列化 DB 写入：所有 update_scene 必经此函数，按调用顺序串行提交。
  const persistSceneContent = useCallback(
    async (sceneId: string, content: string, title?: string, retryCount = 0): Promise<void> => {
      if (!sceneId || !content) return;
      // v0.59.2: 载入期空写保护（见 loadGuardRef 注释）
      const guard = loadGuardRef.current;
      if (guard.armed && guard.sceneId === sceneId) {
        if (isEmptyEditorHtml(content)) {
          logToBackend(
            'frontstage:persist_skip_empty_after_load',
            'persist skipped: empty editor doc right after scene load',
            { sceneId, contentLen: content.length }
          );
          return;
        }
        // 用户已经在写：解除保护，后续清空照常落库
        guard.armed = false;
      }
      const prev = saveChainRef.current;
      let release!: () => void;
      saveChainRef.current = new Promise<void>(r => {
        release = r;
      });
      await prev;
      try {
        // v0.30.50: 读取影响行数——此前丢弃返回值，scene 不存在时后端
        // 静默 0 行更新，UI 显示"已保存"但正文从未落库，重启即丢失。
        const updated = await loggedInvoke<number>(
          'update_scene',
          buildUpdateSceneIpcArgs({ sceneId, title, content })
        );
        if (updated === 0) {
          throw new Error(`update_scene 影响 0 行（scene ${sceneId.slice(0, 8)} 不存在）`);
        }
        setIsSaved(true);
        justSavedRef.current = Date.now();
        // v0.33.x: 失败后的成功写一次恢复日志并清除可见错误态
        if (saveFailedRef.current) {
          saveFailedRef.current = false;
          setSaveError(null);
          logToBackend('frontstage:persist_recovered', 'scene persist recovered after failure', {
            sceneId,
            contentLen: content.length,
            retryCount,
          });
        }
      } catch (e) {
        // v0.33.x: 重试策略由"2s 后仅重试一次"改为封顶退避（2s/10s/30s，经
        //   SAVE_RETRY_DELAYS_MS 配置，retryCount 防无限循环），瞬时 DB 错误可自愈；
        //   重试耗尽后置 saveError 可见态（顶栏可点击重试）。
        // v0.33.x: 失败必须走 log_frontend_event 通道——frontstageLogger 的
        //   warn/error 历史版本从未落盘（见 src-tauri/src/logging.rs），仅 console 可见。
        frontstageLogger.error('Persist scene content failed', {
          error: e,
          willRetry: retryCount < SAVE_RETRY_DELAYS_MS.length,
          retryCount,
        });
        logToBackend('frontstage:persist_failed', 'persist scene content failed', {
          sceneId,
          contentLen: content.length,
          error: String(e),
          willRetry: retryCount < SAVE_RETRY_DELAYS_MS.length,
          retryCount,
        });
        saveFailedRef.current = true;
        setIsSaved(false);
        if (retryCount < SAVE_RETRY_DELAYS_MS.length) {
          const delay = SAVE_RETRY_DELAYS_MS[retryCount];
          saveRetryTimerRef.current = setTimeout(() => {
            saveRetryTimerRef.current = null;
            // v0.33.x: 跨场景重试防护——重试闭包持有调度时的旧正文，若出火时
            // store sceneId 已切换（如自动分章切到新章），继续重试会把旧全文
            // 回写到已截断的旧 scene，造成重复；此处直接 no-op。
            const currentSceneId = useFrontstageStore.getState().sceneId;
            if (currentSceneId !== sceneId) {
              logToBackend(
                'frontstage:persist_retry_skipped',
                'persist retry skipped: scene changed since schedule',
                { sceneId, currentSceneId, retryCount: retryCount + 1 }
              );
              return;
            }
            void persistSceneContent(sceneId, content, title, retryCount + 1);
          }, delay);
        } else {
          setSaveError(String(e));
        }
      } finally {
        release();
      }
    },
    []
  );

  // v0.30.33: flushSceneSave - 取消待执行的防抖保存，立即将 latestContentRef 落库。
  // v0.30.34: 改用 persistSceneContent 序列化，消除并发覆写竞态。
  // v0.30.43: 直接从编辑器读取实际 HTML，而非 latestContentRef。
  //   RichTextEditor 的 onChange 有 200ms 防抖（htmlDebounceRef），latestContentRef
  //   可能比编辑器实际内容滞后 200ms。关闭应用/切换章节时若读 latestContentRef，
  //   最后 200ms 内的输入会丢失。直接读 editorRef.getHTML() 确保保存编辑器实际内容；
  //   editorRef 不可用时回退 latestContentRef。同时回写 latestContentRef 保持一致。
  const flushSceneSave = useCallback(async (): Promise<void> => {
    cancelAutoSave();
    const sceneId = useFrontstageStore.getState().sceneId;
    // v0.33.x: 静默早退插桩——此前 sceneId 缺失/内容为空时无任何痕迹，
    // 保存链路"假死"（正文不进库、日志为零）完全无法定位。
    if (!sceneId) {
      logToBackend('frontstage:flush_skip', 'flushSceneSave skipped: no scene id', {
        reason: 'no_scene_id',
      });
      return;
    }
    const editorHtml = editorRef.current?.getHTML();
    const content = editorHtml || latestContentRef.current;
    if (!content) {
      logToBackend('frontstage:flush_skip', 'flushSceneSave skipped: empty content', {
        reason: 'empty_content',
        sceneId,
      });
      return;
    }
    // 入口日志仅在 sceneId 变化时记录，避免每次 flush 刷量（文思活跃时 flush 约 90s 一次）
    if (lastFlushLoggedSceneIdRef.current !== sceneId) {
      lastFlushLoggedSceneIdRef.current = sceneId;
      logToBackend('frontstage:flush_scene_save', 'flushSceneSave entered', {
        sceneId,
        contentLen: content.length,
      });
    }
    // 同步 latestContentRef，使后续保存基准与编辑器一致
    latestContentRef.current = content;
    await persistSceneContent(
      sceneId,
      content,
      useFrontstageStore.getState().sceneTitle ?? undefined
    );
  }, [persistSceneContent]);
  const flushSceneSaveRef = useRef(flushSceneSave);
  useEffect(() => {
    flushSceneSaveRef.current = flushSceneSave;
  }, [flushSceneSave]);
  // v0.33.x: 顶栏「保存失败，点击重试」入口——清除错误态并立即重新 flush
  const handleRetrySave = useCallback(() => {
    setSaveError(null);
    void flushSceneSaveRef.current();
  }, []);

  return {
    persistSceneContent,
    flushSceneSave,
    flushSceneSaveRef,
    handleRetrySave,
    saveError,
    cancelPersistRetry,
    markSceneContentLoaded,
  };
}
