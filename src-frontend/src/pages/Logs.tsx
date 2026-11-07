import { useState, useMemo, useEffect } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Card } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { PageHeader } from '@/components/ui/PageHeader';
import { Skeleton } from '@/components/ui/Skeleton';
import { cn } from '@/utils/cn';
import {
  useWorkflowLogs,
  useRecentLogs,
  useWorkflowLogPath,
  useLogDirectory,
  type WorkflowLogEntry,
} from '@/hooks/useLogs';
import {
  RefreshCw,
  Search,
  ChevronDown,
  ChevronRight,
  Terminal,
  FileText,
  Play,
  Pause,
  Copy,
  UploadCloud,
} from 'lucide-react';
import { AiCodeBlock } from '@/components/ui/ai/AiCodeBlock';
import { AiFilterChipsBar } from '@/components/ui/ai/AiFilterTable';
import toast from 'react-hot-toast';
import { useAppStore } from '@/stores/appStore';
import { Modal } from '@/components/ui/Modal';
import { loggedInvoke } from '@/services/tauri';

type LogLevel = 'ALL' | 'INFO' | 'WARN' | 'ERROR';
type LogSource = 'workflow' | 'system';

const LEVEL_COLORS: Record<string, string> = {
  INFO: 'bg-blue-500/20 text-blue-300 border-blue-500/30',
  WARN: 'bg-amber-500/20 text-amber-300 border-amber-500/30',
  ERROR: 'bg-red-500/20 text-red-300 border-red-500/30',
};

const COUNT_OPTIONS = [50, 100, 200, 500];

export function Logs() {
  const [source, setSource] = useState<LogSource>('workflow');
  const [level, setLevel] = useState<LogLevel>('ALL');
  const [search, setSearch] = useState('');
  const [count, setCount] = useState(100);
  const [autoRefresh, setAutoRefresh] = useState(true);
  const [expandedRows, setExpandedRows] = useState<Set<number>>(new Set());

  const logsSearchQuery = useAppStore(s => s.logsSearchQuery);
  const setLogsSearchQuery = useAppStore(s => s.setLogsSearchQuery);

  const workflowLogs = useWorkflowLogs(count, autoRefresh && source === 'workflow');
  const systemLogs = useRecentLogs(count, autoRefresh && source === 'system');
  const logPath = useWorkflowLogPath();
  const logDir = useLogDirectory();

  // Consume deep-link search query from GenesisPanel
  useEffect(() => {
    if (logsSearchQuery) {
      setSearch(logsSearchQuery);
      setLogsSearchQuery(null);
    }
  }, [logsSearchQuery, setLogsSearchQuery]);

  const filteredLogs = useMemo(() => {
    if (source !== 'workflow' || !workflowLogs.data) return [];
    return workflowLogs.data.filter(entry => {
      if (level !== 'ALL' && entry.level !== level) return false;
      if (search) {
        const q = search.toLowerCase();
        return (
          entry.message.toLowerCase().includes(q) ||
          entry.phase.toLowerCase().includes(q) ||
          (entry.details && JSON.stringify(entry.details).toLowerCase().includes(q))
        );
      }
      return true;
    });
  }, [source, workflowLogs.data, level, search]);

  const toggleRow = (idx: number) => {
    setExpandedRows(prev => {
      const next = new Set(prev);
      if (next.has(idx)) next.delete(idx);
      else next.add(idx);
      return next;
    });
  };

  const handleCopy = () => {
    const text =
      source === 'workflow'
        ? filteredLogs
            .map(l => `[${formatLocalLogTime(l.ts)}] ${l.level} ${l.phase} — ${l.message}`)
            .join('\n')
        : systemLogs.data || '';
    navigator.clipboard.writeText(text);
    toast.success('已复制到剪贴板');
  };

  const currentPath = source === 'workflow' ? logPath.data : logDir.data;
  const isFetching = source === 'workflow' ? workflowLogs.isFetching : systemLogs.isFetching;

  // ── v0.89.0: 日志包上传（显式授权 → 打包 → 确认 → 上传）─────────
  const [uploadModalOpen, setUploadModalOpen] = useState(false);
  const [packaging, setPackaging] = useState(false);
  const [uploading, setUploading] = useState(false);
  const [packaged, setPackaged] = useState<{
    path: string;
    sha256: string;
    size_bytes: number;
    file_count: number;
    skipped_duplicates: boolean;
  } | null>(null);
  const uploadHistoryQuery = useQuery({
    queryKey: ['log-upload-history'],
    queryFn: () =>
      loggedInvoke<Array<{ sha256: string; filename: string; uploaded_at: string }>>(
        'get_log_upload_history'
      ),
  });

  const handlePackage = async () => {
    setPackaging(true);
    setPackaged(null);
    try {
      const info = await loggedInvoke<{
        path: string;
        sha256: string;
        size_bytes: number;
        file_count: number;
        skipped_duplicates: boolean;
      }>('package_log_report');
      if (info.skipped_duplicates) {
        toast.success('该日志内容与上次上传完全一致，无需重复上传');
      }
      setPackaged(info);
    } catch (e) {
      toast.error('打包失败：' + (e as Error).message);
    } finally {
      setPackaging(false);
    }
  };

  const handleUpload = async () => {
    if (!packaged) return;
    setUploading(true);
    try {
      const res = await loggedInvoke<{ accepted: boolean; duplicate: boolean; sha256: string }>(
        'upload_log_report',
        { file_path: packaged.path, sha256: packaged.sha256 }
      );
      if (res.duplicate) {
        toast.success('服务端已有同内容日志包，无需重复上传');
      } else {
        toast.success('日志包已上传，感谢反馈');
      }
      setUploadModalOpen(false);
      setPackaged(null);
      void uploadHistoryQuery.refetch();
    } catch (e) {
      toast.error((e as Error).message);
    } finally {
      setUploading(false);
    }
  };

  return (
    <div className="flex h-full flex-col gap-4 p-6 max-w-[1600px] mx-auto min-w-0">
      {/* Header */}
      <PageHeader
        title="日志"
        actions={
          <>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setAutoRefresh(v => !v)}
              className="gap-1.5"
            >
              {autoRefresh ? <Pause className="w-4 h-4" /> : <Play className="w-4 h-4" />}
              {autoRefresh ? '暂停' : '自动刷新'}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={() =>
                source === 'workflow' ? workflowLogs.refetch() : systemLogs.refetch()
              }
              isLoading={isFetching}
              className="gap-1.5"
            >
              <RefreshCw className="w-4 h-4" />
              刷新
            </Button>
            <Button variant="ghost" size="sm" onClick={handleCopy} className="gap-1.5">
              <Copy className="w-4 h-4" />
              复制
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setUploadModalOpen(true)}
              className="gap-1.5"
            >
              <UploadCloud className="w-4 h-4" />
              上传日志包
            </Button>
          </>
        }
      />

      {/* Source Tabs + Filters */}
      <Card glass className="p-4 space-y-3">
        <div className="flex flex-wrap items-center gap-3">
          {/* Source Tabs */}
          <div className="flex rounded-lg border border-cinema-700 overflow-hidden">
            <button
              onClick={() => setSource('workflow')}
              className={cn(
                'px-3 py-1.5 text-sm font-medium flex items-center gap-1.5 transition-colors',
                source === 'workflow'
                  ? 'bg-cinema-gold/20 text-cinema-gold'
                  : 'text-ai-ink-2 hover:bg-cinema-800'
              )}
            >
              <FileText className="w-4 h-4" />
              创作工作流
            </button>
            <button
              onClick={() => setSource('system')}
              className={cn(
                'px-3 py-1.5 text-sm font-medium flex items-center gap-1.5 transition-colors',
                source === 'system'
                  ? 'bg-cinema-gold/20 text-cinema-gold'
                  : 'text-ai-ink-2 hover:bg-cinema-800'
              )}
            >
              <Terminal className="w-4 h-4" />
              系统日志
            </button>
          </div>

          {/* Level Filter (workflow only) */}
          {source === 'workflow' && (
            <AiFilterChipsBar
              ariaLabel="日志级别筛选"
              activeKey={level}
              onSelect={key => setLevel(key as LogLevel)}
              items={(['ALL', 'INFO', 'WARN', 'ERROR'] as LogLevel[]).map(l => ({
                key: l,
                label: l,
                mono: true,
                count: workflowLogs.data
                  ? l === 'ALL'
                    ? workflowLogs.data.length
                    : workflowLogs.data.filter(e => e.level === l).length
                  : undefined,
              }))}
            />
          )}

          {/* Search */}
          <div className="relative flex-1 min-w-[200px]">
            <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-4 h-4 text-cinema-500" />
            <input
              value={search}
              onChange={e => setSearch(e.target.value)}
              placeholder="搜索消息、phase 或 details..."
              className="w-full pl-8 pr-3 py-1.5 text-sm bg-cinema-900 border border-cinema-700 rounded-lg text-ai-ink placeholder:text-cinema-500 focus:outline-none focus:border-cinema-gold/50"
            />
          </div>

          {/* Count Selector */}
          <select
            value={count}
            onChange={e => setCount(Number(e.target.value))}
            className="px-3 py-1.5 text-sm bg-cinema-900 border border-cinema-700 rounded-lg text-ai-ink focus:outline-none focus:border-cinema-gold/50"
          >
            {COUNT_OPTIONS.map(n => (
              <option key={n} value={n}>
                {n} 行
              </option>
            ))}
          </select>
        </div>

        {/* Log path */}
        {currentPath && (
          <div className="text-xs text-cinema-500 font-mono truncate">📄 {currentPath}</div>
        )}
      </Card>

      {/* Log Content */}
      <Card glass className="p-0 overflow-hidden flex-1 min-h-0 flex flex-col">
        <div className="flex-1 min-h-0 overflow-y-auto">
          {source === 'workflow' ? (
            /* Workflow Logs */
            workflowLogs.error ? (
              <div className="p-4 text-red-400 text-sm">加载失败：{String(workflowLogs.error)}</div>
            ) : filteredLogs.length === 0 ? (
              <div className="p-8 text-center text-ai-ink-3">
                {workflowLogs.isLoading ? (
                  <div className="space-y-3">
                    {Array.from({ length: 4 }, (_, i) => (
                      <Skeleton key={i} className="h-4 w-full" />
                    ))}
                  </div>
                ) : (
                  '暂无日志'
                )}
              </div>
            ) : (
              <div className="divide-y divide-cinema-800/50">
                {filteredLogs.map((entry, idx) => (
                  <LogRow
                    key={idx}
                    entry={entry}
                    expanded={expandedRows.has(idx)}
                    onToggle={() => toggleRow(idx)}
                  />
                ))}
              </div>
            )
          ) : (
            /* System Logs (plain text) */
            <div className="p-3">
              {systemLogs.error ? (
                <div className="text-red-400 text-sm">加载失败：{String(systemLogs.error)}</div>
              ) : systemLogs.isLoading ? (
                <div className="space-y-3">
                  {Array.from({ length: 4 }, (_, i) => (
                    <Skeleton key={i} className="h-4 w-full" />
                  ))}
                </div>
              ) : (
                <AiCodeBlock code={systemLogs.data || '暂无系统日志'} copyable={false} />
              )}
            </div>
          )}
        </div>
      </Card>

      {/* v0.89.0: 显式授权上传弹窗——列明内容/用途/保留期，确认后才打包上传 */}
      <Modal
        open={uploadModalOpen}
        onClose={() => {
          setUploadModalOpen(false);
          setPackaged(null);
        }}
        title="上传日志包到官网"
        size="md"
        testId="log-upload-modal"
      >
        <div className="space-y-3 text-sm text-ai-ink-2">
          <p className="text-xs leading-5 text-ai-ink-3">
            将压缩打包以下<b>近 3 天</b>的诊断日志（总量上限约 20MB）：
          </p>
          <ul className="list-disc space-y-1 pl-5 text-xs leading-5 text-ai-ink-3">
            <li>系统运行日志（storymoss.log.*，含错误与告警）</li>
            <li>创作工作流日志（creative_workflow.log，各阶段耗时/失败）</li>
            <li>生成链路 traces（模型/耗时/Token 统计）与崩溃快照</li>
            <li>
              manifest.json（应用版本/操作系统/文件清单）——<b>不含小说正文、角色与世界观内容</b>
            </li>
          </ul>
          <p className="text-xs leading-5 text-ai-ink-3">
            用途：定位崩溃与生成失败、改进稳定性。上传到 storymoss.top 后<b>仅保留 3 天</b>， 按内容
            SHA-256 去重，不会重复占用空间。上传需要登录账号。
          </p>

          {!packaged ? (
            <Button
              variant="primary"
              onClick={handlePackage}
              isLoading={packaging}
              className="w-full"
            >
              <UploadCloud className="mr-1.5 h-4 w-4" />
              第一步：打包日志（不上传）
            </Button>
          ) : packaged.skipped_duplicates ? (
            <div className="rounded border border-ai-line bg-ai-inset p-3 text-xs text-ai-ink-3">
              打包完成：内容与上次上传一致（SHA-256 去重命中），无需再传。
            </div>
          ) : (
            <div className="rounded border border-ai-line bg-ai-inset p-3 text-xs">
              <div className="text-ai-ink-2">
                已打包 {packaged.file_count} 个文件，
                {(packaged.size_bytes / 1024 / 1024).toFixed(1)} MB
              </div>
              <div className="mt-1 font-mono text-[10px] text-ai-ink-3 break-all">
                SHA-256: {packaged.sha256.slice(0, 24)}…
              </div>
              <Button
                variant="primary"
                onClick={handleUpload}
                isLoading={uploading}
                className="mt-2 w-full"
              >
                确认上传（需已登录）
              </Button>
            </div>
          )}

          {(uploadHistoryQuery.data?.length ?? 0) > 0 && (
            <div className="pt-2 border-t border-ai-line">
              <div className="mb-1 text-[11px] text-ai-ink-3">
                本机上传记录（{uploadHistoryQuery.data!.length}）
              </div>
              {uploadHistoryQuery.data!.slice(0, 5).map(r => (
                <div key={r.sha256} className="truncate text-[10px] text-ai-ink-3">
                  {r.uploaded_at} · {r.filename}
                </div>
              ))}
            </div>
          )}
        </div>
      </Modal>
    </div>
  );
}

function formatLocalLogTime(ts: string): string {
  // v0.73.7: 日志 JSON 的 ts 是 UTC ISO 串；旧实现直接 slice(11,23) 显示
  // UTC，用户看到的时间比本地时间早 8 小时。这里统一转本地时间。
  const d = new Date(ts);
  if (Number.isNaN(d.getTime())) return ts.slice(11, 23);
  const pad = (n: number, width = 2) => String(n).padStart(width, '0');
  return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}.${pad(d.getMilliseconds(), 3)}`;
}

function LogRow({
  entry,
  expanded,
  onToggle,
}: {
  entry: WorkflowLogEntry;
  expanded: boolean;
  onToggle: () => void;
}) {
  const time = formatLocalLogTime(entry.ts);
  const hasDetails = entry.details && Object.keys(entry.details).length > 0;

  return (
    <div className="px-3 py-2 hover:bg-cinema-800/30 transition-colors">
      <div className="flex items-start gap-2">
        {/* Expand toggle */}
        <button onClick={onToggle} className="mt-0.5 text-cinema-500 hover:text-ai-ink-2 shrink-0">
          {hasDetails ? (
            expanded ? (
              <ChevronDown className="w-3.5 h-3.5" />
            ) : (
              <ChevronRight className="w-3.5 h-3.5" />
            )
          ) : (
            <span className="w-3.5 inline-block" />
          )}
        </button>

        {/* Time */}
        <span className="text-xs font-mono text-cinema-500 shrink-0 mt-0.5">{time}</span>

        {/* Level badge */}
        <span
          className={cn(
            'px-1.5 py-0.5 text-[10px] font-mono font-bold rounded border shrink-0 mt-0.5',
            LEVEL_COLORS[entry.level] || 'bg-cinema-700/30 text-ai-ink-3 border-cinema-600/30'
          )}
        >
          {entry.level}
        </span>

        {/* Phase */}
        <span className="text-xs font-mono text-cinema-gold/70 shrink-0 mt-0.5 max-w-[280px] truncate">
          {entry.phase}
        </span>

        {/* Message */}
        <span className="text-xs text-ai-ink flex-1 break-all mt-0.5">{entry.message}</span>
      </div>

      {/* Details (expandable) */}
      {expanded && hasDetails && (
        <div className="mt-1.5 ml-8">
          <AiCodeBlock code={JSON.stringify(entry.details, null, 2)} language="JSON" />
        </div>
      )}
    </div>
  );
}
