import { useState, useEffect, useCallback, useRef } from 'react';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { createLogger } from '@/utils/logger';
import { loggedInvoke } from '@/services/tauri';

const updaterLogger = createLogger('hooks:useUpdater');

/** 忽略版本冷却期：7天（毫秒） */
const DISMISS_COOLDOWN_MS = 7 * 24 * 60 * 60 * 1000;
/** 自动检测间隔：4小时（毫秒） */
const CHECK_INTERVAL_MS = 4 * 60 * 60 * 1000;

export interface UpdateInfo {
  version: string;
  notes: string;
  pub_date: string;
  signature: string;
}

export interface CheckUpdateResult {
  has_update: boolean;
  current_version: string;
  latest_version: string | null;
  update_info: UpdateInfo | null;
}

export interface UpdateDownloadProgress {
  downloaded: number;
  total: number | null;
  percentage: number;
}

export interface UseUpdaterReturn {
  currentVersion: string;
  hasUpdate: boolean;
  latestVersion: string | null;
  updateInfo: UpdateInfo | null;
  isChecking: boolean;
  isInstalling: boolean;
  downloadProgress: UpdateDownloadProgress | null;
  error: string | null;
  checkUpdate: () => Promise<void>;
  installUpdate: () => Promise<void>;
  dismissUpdate: () => void;
}

/** 检查指定版本是否处于忽略冷却期 */
function isVersionDismissed(version: string): boolean {
  try {
    const stored = localStorage.getItem('storymoss_dismissed_update');
    if (!stored) return false;
    const { version: dismissedVersion, timestamp } = JSON.parse(stored);
    if (dismissedVersion !== version) return false;
    const elapsed = Date.now() - timestamp;
    return elapsed < DISMISS_COOLDOWN_MS;
  } catch {
    return false;
  }
}

/** 记录忽略版本（7天冷却） */
function dismissVersion(version: string) {
  localStorage.setItem(
    'storymoss_dismissed_update',
    JSON.stringify({ version, timestamp: Date.now() })
  );
}

export function useUpdater(autoCheck: boolean = true): UseUpdaterReturn {
  const [currentVersion, setCurrentVersion] = useState<string>('');
  const [hasUpdate, setHasUpdate] = useState<boolean>(false);
  const [latestVersion, setLatestVersion] = useState<string | null>(null);
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [isChecking, setIsChecking] = useState<boolean>(false);
  const [isInstalling, setIsInstalling] = useState<boolean>(false);
  const [downloadProgress, setDownloadProgress] = useState<UpdateDownloadProgress | null>(null);
  const [error, setError] = useState<string | null>(null);

  const dismissedRef = useRef<string | null>(null);
  const progressUnlistenRef = useRef<UnlistenFn | null>(null);
  const completeUnlistenRef = useRef<UnlistenFn | null>(null);

  // 获取当前版本
  useEffect(() => {
    loggedInvoke<string>('get_current_version')
      .then(setCurrentVersion)
      .catch(err => updaterLogger.error('Failed to get current version', { error: err }));
  }, []);

  // 监听下载进度事件
  useEffect(() => {
    const setupListeners = async () => {
      try {
        progressUnlistenRef.current = await listen<UpdateDownloadProgress>(
          'update-download-progress',
          event => {
            setDownloadProgress(event.payload);
            updaterLogger.debug(
              `[Updater] Download progress: ${event.payload.percentage.toFixed(1)}%`
            );
          }
        );
        completeUnlistenRef.current = await listen('update-download-complete', () => {
          updaterLogger.info('[Updater] Download completed, app will restart');
        });
      } catch (e) {
        updaterLogger.error('Failed to setup update progress listeners', { error: e });
      }
    };
    setupListeners();

    return () => {
      progressUnlistenRef.current?.();
      completeUnlistenRef.current?.();
    };
  }, []);

  // 检查更新
  const checkUpdate = useCallback(async () => {
    setIsChecking(true);
    setError(null);
    setDownloadProgress(null);

    try {
      const result = await loggedInvoke<CheckUpdateResult>('check_update');

      // 如果版本处于忽略冷却期，不提示
      if (result.has_update && result.latest_version) {
        if (isVersionDismissed(result.latest_version)) {
          updaterLogger.debug(`[Updater] Version ${result.latest_version} dismissed, skipping`);
          setHasUpdate(false);
          setLatestVersion(null);
          setUpdateInfo(null);
          dismissedRef.current = result.latest_version;
          return;
        }
      }

      setHasUpdate(result.has_update);
      setLatestVersion(result.latest_version);
      setUpdateInfo(result.update_info);

      if (result.has_update) {
        updaterLogger.debug(`[Updater] New version available: ${result.latest_version}`);
      }
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      setError(errorMessage);
      updaterLogger.error('[Updater] Check update failed', { error: err });
    } finally {
      setIsChecking(false);
    }
  }, []);

  // 安装更新
  const installUpdate = useCallback(async () => {
    if (!hasUpdate) return;

    setIsInstalling(true);
    setError(null);

    try {
      await loggedInvoke<unknown>('install_update');
      // 如果安装成功，应用会重启，这里不会执行到
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      setError(errorMessage);
      setIsInstalling(false);
      setDownloadProgress(null);
      updaterLogger.error('[Updater] Install update failed', { error: err });
    }
  }, [hasUpdate]);

  // 忽略更新
  const dismissUpdate = useCallback(() => {
    setHasUpdate(false);
    setDownloadProgress(null);
    if (latestVersion) {
      dismissVersion(latestVersion);
      dismissedRef.current = latestVersion;
    }
  }, [latestVersion]);

  // 自动检查更新
  useEffect(() => {
    if (autoCheck) {
      // 启动时延迟 5 秒检查（避免启动时网络竞争）
      const startupTimer = setTimeout(() => {
        checkUpdate();
      }, 5000);

      // 每 4 小时检查一次
      const interval = setInterval(checkUpdate, CHECK_INTERVAL_MS);

      return () => {
        clearTimeout(startupTimer);
        clearInterval(interval);
      };
    }
  }, [autoCheck, checkUpdate]);

  return {
    currentVersion,
    hasUpdate,
    latestVersion,
    updateInfo,
    isChecking,
    isInstalling,
    downloadProgress,
    error,
    checkUpdate,
    installUpdate,
    dismissUpdate,
  };
}
