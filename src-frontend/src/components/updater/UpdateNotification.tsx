import React from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import {
  Download,
  X,
  RefreshCw,
  AlertCircle,
  Sparkles,
  ChevronRight,
  Clock,
  CheckCircle2,
} from 'lucide-react';
import { Button } from '@/components/ui/Button';
import { cn } from '@/utils/cn';
import type { UpdateInfo, UpdateDownloadProgress } from '@/hooks/useUpdater';

interface UpdateNotificationProps {
  isOpen: boolean;
  currentVersion: string;
  latestVersion: string | null;
  updateInfo: UpdateInfo | null;
  isInstalling: boolean;
  downloadProgress: UpdateDownloadProgress | null;
  error: string | null;
  onInstall: () => void;
  onDismiss: () => void;
  onCheck: () => void;
  className?: string;
}

/**
 * 解析 release notes 为结构化分类
 * 支持格式：
 *   ### 新增 / ## 新增
 *   - xxx
 *   ### 修复 / ## 修复
 *   - yyy
 *   ### 注意 / ## 注意 / ### 变更 / ## 变更
 *   - zzz
 */
function parseReleaseNotes(notes: string): {
  features: string[];
  fixes: string[];
  breaking: string[];
  other: string[];
} {
  const result = {
    features: [] as string[],
    fixes: [] as string[],
    breaking: [] as string[],
    other: [] as string[],
  };
  if (!notes) return result;

  const lines = notes.split('\n');
  let currentCategory: 'features' | 'fixes' | 'breaking' | 'other' = 'other';

  for (const rawLine of lines) {
    const line = rawLine.trim();
    if (!line) continue;

    // 检测分类标题
    if (/^#{2,3}\s*(新增|功能|Features?)/i.test(line)) {
      currentCategory = 'features';
      continue;
    }
    if (/^#{2,3}\s*(修复|修正|Fixes?|Bugfix)/i.test(line)) {
      currentCategory = 'fixes';
      continue;
    }
    if (/^#{2,3}\s*(注意|变更|破坏性|Breaking|Changes?)/i.test(line)) {
      currentCategory = 'breaking';
      continue;
    }
    if (/^#{2,3}\s/.test(line)) {
      currentCategory = 'other';
      continue;
    }

    // 收集列表项
    const item = line.replace(/^[-*•]\s*/, '').trim();
    if (item) {
      result[currentCategory].push(item);
    }
  }

  return result;
}

export const UpdateNotification: React.FC<UpdateNotificationProps> = ({
  isOpen,
  currentVersion,
  latestVersion,
  updateInfo,
  isInstalling,
  downloadProgress,
  error,
  onInstall,
  onDismiss,
  onCheck,
  className,
}) => {
  const isDownloadInProgress =
    isInstalling && downloadProgress !== null && downloadProgress.percentage < 100;
  const isReadyToRestart =
    isInstalling && downloadProgress !== null && downloadProgress.percentage >= 100;

  const categories = React.useMemo(
    () => parseReleaseNotes(updateInfo?.notes ?? ''),
    [updateInfo?.notes]
  );

  return (
    <AnimatePresence>
      {isOpen && (
        <motion.div
          initial={{ opacity: 0, y: -50, scale: 0.95 }}
          animate={{ opacity: 1, y: 0, scale: 1 }}
          exit={{ opacity: 0, y: -50, scale: 0.95 }}
          transition={{ duration: 0.3, ease: [0.4, 0, 0.2, 1] }}
          className={cn('fixed top-4 right-4 z-50 w-96', className)}
        >
          <div className="bg-white rounded-xl shadow-2xl border border-terracotta/20 overflow-hidden">
            {/* Header */}
            <div className="bg-gradient-to-r from-terracotta to-terracotta/80 px-4 py-3 flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Sparkles className="w-5 h-5 text-white" />
                <span className="font-serif text-white font-medium">
                  {isDownloadInProgress
                    ? '正在下载更新'
                    : isReadyToRestart
                      ? '更新已就绪'
                      : '发现新版本'}
                </span>
              </div>
              <button
                onClick={onDismiss}
                disabled={isInstalling && !isReadyToRestart}
                className="text-white/80 hover:text-white transition-colors disabled:opacity-50"
              >
                <X className="w-5 h-5" />
              </button>
            </div>

            {/* Content */}
            <div className="p-4 space-y-4">
              {/* Version Info */}
              <div className="flex items-center gap-3">
                <div className="flex-1">
                  <div className="text-sm text-stone-500">当前版本</div>
                  <div className="font-mono text-stone-700">v{currentVersion}</div>
                </div>
                <ChevronRight className="w-5 h-5 text-stone-400" />
                <div className="flex-1">
                  <div className="text-sm text-stone-500">最新版本</div>
                  <div className="font-mono text-terracotta font-medium">
                    v{latestVersion || '...'}
                  </div>
                </div>
              </div>

              {/* Download Progress */}
              {isInstalling && downloadProgress && (
                <div className="space-y-2">
                  <div className="flex items-center justify-between text-sm">
                    <span className="text-stone-600">
                      {isDownloadInProgress ? '正在下载...' : '下载完成'}
                    </span>
                    <span className="font-mono text-terracotta font-medium">
                      {downloadProgress.percentage.toFixed(0)}%
                    </span>
                  </div>
                  <div className="h-2 bg-stone-100 rounded-full overflow-hidden">
                    <div
                      className="h-full bg-terracotta rounded-full transition-all duration-300"
                      style={{ width: `${downloadProgress.percentage}%` }}
                    />
                  </div>
                  {downloadProgress.total && downloadProgress.total > 0 && (
                    <div className="text-xs text-stone-400 text-right">
                      {formatBytes(downloadProgress.downloaded)} /{' '}
                      {formatBytes(downloadProgress.total)}
                    </div>
                  )}
                </div>
              )}

              {/* Structured Release Notes */}
              {updateInfo?.notes && !isInstalling && (
                <div className="bg-stone-50 rounded-lg p-3 max-h-40 overflow-y-auto space-y-2">
                  {categories.features.length > 0 && (
                    <div>
                      <div className="text-xs font-semibold text-green-700 mb-1 flex items-center gap-1">
                        <Sparkles className="w-3 h-3" /> 新增
                      </div>
                      <ul className="space-y-0.5">
                        {categories.features.slice(0, 3).map((item, i) => (
                          <li
                            key={`f-${i}`}
                            className="text-xs text-stone-600 pl-2 border-l-2 border-green-300"
                          >
                            {item}
                          </li>
                        ))}
                        {categories.features.length > 3 && (
                          <li className="text-xs text-stone-400 pl-2">
                            +{categories.features.length - 3} 项更多...
                          </li>
                        )}
                      </ul>
                    </div>
                  )}
                  {categories.fixes.length > 0 && (
                    <div>
                      <div className="text-xs font-semibold text-blue-700 mb-1 flex items-center gap-1">
                        <CheckCircle2 className="w-3 h-3" /> 修复
                      </div>
                      <ul className="space-y-0.5">
                        {categories.fixes.slice(0, 2).map((item, i) => (
                          <li
                            key={`x-${i}`}
                            className="text-xs text-stone-600 pl-2 border-l-2 border-blue-300"
                          >
                            {item}
                          </li>
                        ))}
                        {categories.fixes.length > 2 && (
                          <li className="text-xs text-stone-400 pl-2">
                            +{categories.fixes.length - 2} 项更多...
                          </li>
                        )}
                      </ul>
                    </div>
                  )}
                  {categories.breaking.length > 0 && (
                    <div>
                      <div className="text-xs font-semibold text-orange-700 mb-1 flex items-center gap-1">
                        <AlertCircle className="w-3 h-3" /> 注意
                      </div>
                      <ul className="space-y-0.5">
                        {categories.breaking.map((item, i) => (
                          <li
                            key={`b-${i}`}
                            className="text-xs text-stone-600 pl-2 border-l-2 border-orange-300"
                          >
                            {item}
                          </li>
                        ))}
                      </ul>
                    </div>
                  )}
                  {categories.features.length === 0 &&
                    categories.fixes.length === 0 &&
                    categories.breaking.length === 0 && (
                      <div className="text-sm text-stone-600 whitespace-pre-wrap">
                        {updateInfo.notes.slice(0, 500)}
                        {updateInfo.notes.length > 500 && '...'}
                      </div>
                    )}
                </div>
              )}

              {/* Error */}
              {error && (
                <div className="flex items-start gap-2 text-red-600 bg-red-50 p-3 rounded-lg">
                  <AlertCircle className="w-4 h-4 mt-0.5 flex-shrink-0" />
                  <div className="text-sm">{error}</div>
                </div>
              )}

              {/* Actions */}
              <div className="flex gap-2">
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={onCheck}
                  disabled={isInstalling}
                  className="flex-1"
                >
                  <RefreshCw className="w-4 h-4 mr-2" />
                  刷新
                </Button>
                {!isInstalling ? (
                  <Button
                    size="sm"
                    onClick={onInstall}
                    className="flex-1 bg-terracotta hover:bg-terracotta/90 text-white"
                  >
                    <Download className="w-4 h-4 mr-2" />
                    立即更新
                  </Button>
                ) : isReadyToRestart ? (
                  <Button
                    size="sm"
                    onClick={onInstall}
                    className="flex-1 bg-green-600 hover:bg-green-700 text-white"
                  >
                    <CheckCircle2 className="w-4 h-4 mr-2" />
                    重启安装
                  </Button>
                ) : (
                  <Button
                    size="sm"
                    disabled
                    className="flex-1 bg-terracotta/50 text-white cursor-not-allowed"
                  >
                    <RefreshCw className="w-4 h-4 mr-2 animate-spin" />
                    下载中...
                  </Button>
                )}
              </div>

              {/* Note */}
              <p className="text-xs text-stone-400 text-center flex items-center justify-center gap-1">
                <Clock className="w-3 h-3" />
                {!isInstalling
                  ? '更新将在下载完成后自动重启应用'
                  : isReadyToRestart
                    ? '点击"重启安装"完成更新'
                    : '请勿关闭应用，下载完成后将自动重启'}
              </p>
            </div>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
};

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
}

export default UpdateNotification;
