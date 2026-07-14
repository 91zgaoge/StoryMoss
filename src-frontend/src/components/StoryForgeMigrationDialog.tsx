import { useState, useEffect, useCallback } from 'react';
import { listen } from '@tauri-apps/api/event';
import { relaunch } from '@tauri-apps/plugin-process';
import { loggedInvoke } from '@/services/tauri';
import { createLogger } from '@/utils/logger';

const logger = createLogger('ui:StoryForgeMigrationDialog');

interface MigrationPromptPayload {
  source_path: string;
}

interface MigrationResult {
  success: boolean;
  message: string;
  needs_restart: boolean;
}

type DialogState =
  | { kind: 'prompt' }
  | { kind: 'migrating' }
  | { kind: 'success'; message: string }
  | { kind: 'error'; message: string };

export function StoryForgeMigrationDialog() {
  const [open, setOpen] = useState(false);
  const [state, setState] = useState<DialogState | null>(null);
  const [sourcePath, setSourcePath] = useState('');

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setup = async () => {
      try {
        unlisten = await listen<MigrationPromptPayload>(
          'storyforge-migration-prompt',
          event => {
            setSourcePath(event.payload.source_path);
            setState({ kind: 'prompt' });
            setOpen(true);
          }
        );
      } catch (e) {
        logger.error('Failed to setup migration prompt listener', { error: e });
      }
    };

    setup();
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  const handleMigrate = useCallback(async () => {
    setState({ kind: 'migrating' });
    try {
      const result = await loggedInvoke<MigrationResult>('migrate_storyforge_data');
      if (result.success && result.needs_restart) {
        setState({ kind: 'success', message: result.message });
      } else {
        // Migration completed without requiring a restart; close the dialog.
        setOpen(false);
        setState(null);
      }
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      logger.error('StoryForge migration failed', { error: e });
      setState({ kind: 'error', message });
    }
  }, []);

  const handleSkip = useCallback(async () => {
    try {
      await loggedInvoke<void>('mark_migration_skipped');
    } catch (e) {
      logger.error('Failed to mark migration skipped', { error: e });
    } finally {
      setOpen(false);
      setState(null);
    }
  }, []);

  const handleRestart = useCallback(async () => {
    try {
      await relaunch();
    } catch (e) {
      logger.error('Failed to relaunch application', { error: e });
    }
  }, []);

  if (!open || state === null) {
    return null;
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4">
      <div className="w-full max-w-md rounded-lg border border-cinema-700 bg-cinema-900 p-6 shadow-xl">
        {state.kind === 'prompt' && (
          <>
            <h2 className="mb-2 text-lg font-semibold text-cinema-100">
              检测到旧版 StoryForge 数据
            </h2>
            <p className="mb-4 text-sm text-cinema-300">
              是否将配置、故事和数据库全部导入到 StoryMoss？导入后原 StoryForge 数据仍会保留。
            </p>
            <p className="mb-6 text-xs text-cinema-400 break-all">
              来源：{sourcePath}
            </p>
            <div className="flex justify-end gap-3">
              <button
                type="button"
                onClick={handleSkip}
                className="rounded-md px-4 py-2 text-sm font-medium text-cinema-300 hover:bg-cinema-800"
              >
                跳过
              </button>
              <button
                type="button"
                onClick={handleMigrate}
                className="rounded-md bg-emerald-600 px-4 py-2 text-sm font-medium text-white hover:bg-emerald-500"
              >
                立即导入
              </button>
            </div>
          </>
        )}

        {state.kind === 'migrating' && (
          <div className="py-8 text-center text-cinema-200">
            <p className="mb-2 text-base font-medium">正在迁移数据…</p>
            <p className="text-sm text-cinema-400">请勿关闭应用</p>
          </div>
        )}

        {state.kind === 'success' && (
          <>
            <h2 className="mb-2 text-lg font-semibold text-emerald-400">
              迁移完成
            </h2>
            <p className="mb-4 text-sm text-cinema-200">{state.message}</p>
            <p className="mb-6 text-sm text-cinema-300">
              需要重启应用以使用迁移后的数据初始化数据库。
            </p>
            <div className="flex justify-end">
              <button
                type="button"
                onClick={handleRestart}
                className="rounded-md bg-emerald-600 px-4 py-2 text-sm font-medium text-white hover:bg-emerald-500"
              >
                立即重启
              </button>
            </div>
          </>
        )}

        {state.kind === 'error' && (
          <>
            <h2 className="mb-2 text-lg font-semibold text-red-400">
              迁移失败
            </h2>
            <p className="mb-6 text-sm text-cinema-200">{state.message}</p>
            <div className="flex justify-end gap-3">
              <button
                type="button"
                onClick={handleSkip}
                className="rounded-md px-4 py-2 text-sm font-medium text-cinema-300 hover:bg-cinema-800"
              >
                跳过
              </button>
              <button
                type="button"
                onClick={() => setState({ kind: 'prompt' })}
                className="rounded-md bg-emerald-600 px-4 py-2 text-sm font-medium text-white hover:bg-emerald-500"
              >
                重试
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
