import { useState, useMemo } from 'react';
import { Card } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
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
} from 'lucide-react';
import { toast } from 'react-hot-toast';

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

  const workflowLogs = useWorkflowLogs(count, autoRefresh && source === 'workflow');
  const systemLogs = useRecentLogs(count, autoRefresh && source === 'system');
  const logPath = useWorkflowLogPath();
  const logDir = useLogDirectory();

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
        ? filteredLogs.map(l => `[${l.ts}] ${l.level} ${l.phase} — ${l.message}`).join('\n')
        : systemLogs.data || '';
    navigator.clipboard.writeText(text);
    toast.success('已复制到剪贴板');
  };

  const currentPath = source === 'workflow' ? logPath.data : logDir.data;
  const isFetching = source === 'workflow' ? workflowLogs.isFetching : systemLogs.isFetching;

  return (
    <div className="space-y-4 p-6 max-w-[1600px] mx-auto">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-display text-cinema-gold">日志查看器</h1>
          <p className="text-sm text-cinema-300 mt-1">实时查看创作工作流日志与系统 tracing 日志</p>
        </div>
        <div className="flex items-center gap-2">
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
            onClick={() => (source === 'workflow' ? workflowLogs.refetch() : systemLogs.refetch())}
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
        </div>
      </div>

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
                  : 'text-cinema-300 hover:bg-cinema-800'
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
                  : 'text-cinema-300 hover:bg-cinema-800'
              )}
            >
              <Terminal className="w-4 h-4" />
              系统日志
            </button>
          </div>

          {/* Level Filter (workflow only) */}
          {source === 'workflow' && (
            <div className="flex rounded-lg border border-cinema-700 overflow-hidden">
              {(['ALL', 'INFO', 'WARN', 'ERROR'] as LogLevel[]).map(l => (
                <button
                  key={l}
                  onClick={() => setLevel(l)}
                  className={cn(
                    'px-3 py-1.5 text-xs font-mono transition-colors',
                    level === l
                      ? 'bg-cinema-gold/20 text-cinema-gold'
                      : 'text-cinema-400 hover:bg-cinema-800'
                  )}
                >
                  {l}
                </button>
              ))}
            </div>
          )}

          {/* Search */}
          <div className="relative flex-1 min-w-[200px]">
            <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-4 h-4 text-cinema-500" />
            <input
              value={search}
              onChange={e => setSearch(e.target.value)}
              placeholder="搜索消息、phase 或 details..."
              className="w-full pl-8 pr-3 py-1.5 text-sm bg-cinema-900 border border-cinema-700 rounded-lg text-cinema-100 placeholder:text-cinema-500 focus:outline-none focus:border-cinema-gold/50"
            />
          </div>

          {/* Count Selector */}
          <select
            value={count}
            onChange={e => setCount(Number(e.target.value))}
            className="px-3 py-1.5 text-sm bg-cinema-900 border border-cinema-700 rounded-lg text-cinema-100 focus:outline-none focus:border-cinema-gold/50"
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
      <Card glass className="p-0 overflow-hidden">
        <div className="max-h-[calc(100vh-320px)] overflow-y-auto">
          {source === 'workflow' ? (
            /* Workflow Logs */
            workflowLogs.error ? (
              <div className="p-4 text-red-400 text-sm">加载失败：{String(workflowLogs.error)}</div>
            ) : filteredLogs.length === 0 ? (
              <div className="p-8 text-center text-cinema-400">
                {workflowLogs.isLoading ? '加载中...' : '暂无日志'}
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
                <div className="text-cinema-400 text-sm">加载中...</div>
              ) : (
                <pre className="text-xs font-mono text-cinema-300 whitespace-pre-wrap break-all leading-relaxed">
                  {systemLogs.data || '暂无系统日志'}
                </pre>
              )}
            </div>
          )}
        </div>
      </Card>
    </div>
  );
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
  const time = entry.ts.slice(11, 23); // HH:MM:SS.mmm
  const hasDetails = entry.details && Object.keys(entry.details).length > 0;

  return (
    <div className="px-3 py-2 hover:bg-cinema-800/30 transition-colors">
      <div className="flex items-start gap-2">
        {/* Expand toggle */}
        <button
          onClick={onToggle}
          className="mt-0.5 text-cinema-500 hover:text-cinema-300 shrink-0"
        >
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
            LEVEL_COLORS[entry.level] || 'bg-cinema-700/30 text-cinema-400 border-cinema-600/30'
          )}
        >
          {entry.level}
        </span>

        {/* Phase */}
        <span className="text-xs font-mono text-cinema-gold/70 shrink-0 mt-0.5 max-w-[280px] truncate">
          {entry.phase}
        </span>

        {/* Message */}
        <span className="text-xs text-cinema-200 flex-1 break-all mt-0.5">{entry.message}</span>
      </div>

      {/* Details (expandable) */}
      {expanded && hasDetails && (
        <div className="mt-1.5 ml-8">
          <pre className="text-xs font-mono text-cinema-400 bg-cinema-900/50 rounded p-2 overflow-x-auto">
            {JSON.stringify(entry.details, null, 2)}
          </pre>
        </div>
      )}
    </div>
  );
}
