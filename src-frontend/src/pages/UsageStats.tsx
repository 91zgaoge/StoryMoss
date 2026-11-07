import { useState, useEffect, useMemo } from 'react';
import { cn } from '@/utils/cn';
import { useAppStore } from '@/stores/appStore';
import { getLlmCallStats, getRecentLlmCalls, getLlmUsageByRole } from '@/services/tauri';
import type { LlmUsageByRole } from '@/services/tauri';
import { Card, CardContent } from '@/components/ui/Card';
import { AiFilterChipsBar, AiFilterTable } from '@/components/ui/ai/AiFilterTable';
import { AiInsightCards } from '@/components/ui/ai/AiInsightCards';
import {
  BarChart3,
  Coins,
  Hash,
  Activity,
  Clock,
  CheckCircle,
  XCircle,
  Loader2,
  Info,
  Users,
} from 'lucide-react';
import type { LlmCall } from '@/types';

type OperationTab = 'all' | 'bootstrap' | 'smart_execute' | 'other';

const BOOTSTRAP_KEYWORDS = [
  'genesis',
  'bootstrap',
  '创世',
  'opening',
  'novel-bootstrap',
  'strategy_selection',
  'world_building',
  'foreshadow',
];

const SMART_EXECUTE_KEYWORDS = [
  'smart_execute',
  '续写',
  'writer',
  'continuation',
  'tri_shot',
  'trishot',
  'append',
  'call3',
];

function buildOperationHaystack(call: LlmCall): string {
  const parts = [call.purpose ?? '', call.task_type ?? '', call.metadata ?? ''];

  if (call.metadata) {
    try {
      const meta = JSON.parse(call.metadata) as Record<string, unknown>;
      for (const key of ['operation', 'operation_type', 'label', 'purpose']) {
        const value = meta[key];
        if (value != null) parts.push(String(value));
      }
    } catch {
      // metadata may be plain text, not JSON
    }
  }

  return parts.join('|').toLowerCase();
}

function deriveOperation(call: LlmCall): OperationTab {
  const haystack = buildOperationHaystack(call);
  if (BOOTSTRAP_KEYWORDS.some(keyword => haystack.includes(keyword))) {
    return 'bootstrap';
  }
  if (SMART_EXECUTE_KEYWORDS.some(keyword => haystack.includes(keyword))) {
    return 'smart_execute';
  }
  return 'other';
}

const TAB_LABELS: Record<OperationTab, string> = {
  all: '全部',
  bootstrap: '创世',
  smart_execute: '智能续写',
  other: '其他',
};

/** v0.86.0 A3：llm_calls.purpose 里的角色标签 → 用户可读名。 */
const ROLE_LABELS: Record<string, string> = {
  agency_writer: '主创',
  agency_producer: '管理',
  agency_editor: '编审',
  director: '导演',
  frontstage: '幕前',
  planner: '规划',
};

function roleUsageLabel(raw: string): string {
  return ROLE_LABELS[raw] ?? raw.replace(/^agency_/, '');
}

export function UsageStats({ embedded = false }: { embedded?: boolean }) {
  const currentStory = useAppStore(s => s.currentStory);
  const [globalStats, setGlobalStats] = useState<{
    count: number;
    total_tokens: number;
    total_cost: number;
  } | null>(null);
  const [storyStats, setStoryStats] = useState<{
    count: number;
    total_tokens: number;
    total_cost: number;
  } | null>(null);
  const [recentCalls, setRecentCalls] = useState<LlmCall[]>([]);
  const [roleUsage, setRoleUsage] = useState<LlmUsageByRole[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [operationTab, setOperationTab] = useState<OperationTab>('all');

  useEffect(() => {
    const fetchStats = async () => {
      setIsLoading(true);
      try {
        const [global, recent, roles] = await Promise.all([
          getLlmCallStats('global').catch(() => null),
          getRecentLlmCalls(50).catch(() => [] as LlmCall[]),
          getLlmUsageByRole(currentStory?.id).catch(() => [] as LlmUsageByRole[]),
        ]);
        setGlobalStats(global);
        setRecentCalls(recent);
        setRoleUsage(roles);

        if (currentStory?.id) {
          const story = await getLlmCallStats(currentStory.id).catch(() => null);
          setStoryStats(story);
        } else {
          setStoryStats(null);
        }
      } catch (e) {
        console.warn('[UsageStats] fetch failed:', e);
      } finally {
        setIsLoading(false);
      }
    };

    fetchStats();
  }, [currentStory?.id]);

  const filteredCalls = useMemo(() => {
    if (operationTab === 'all') return recentCalls;
    return recentCalls.filter(c => deriveOperation(c) === operationTab);
  }, [recentCalls, operationTab]);

  const filteredStats = useMemo(() => {
    const calls = filteredCalls;
    return {
      count: calls.length,
      total_tokens: calls.reduce((s, c) => s + (c.total_tokens || 0), 0),
      success_rate:
        calls.length > 0
          ? Math.round((calls.filter(c => c.success).length / calls.length) * 100)
          : null,
    };
  }, [filteredCalls]);

  const operationCounts = useMemo(() => {
    const counts: Record<OperationTab, number> = {
      all: recentCalls.length,
      bootstrap: 0,
      smart_execute: 0,
      other: 0,
    };
    for (const c of recentCalls) counts[deriveOperation(c)] += 1;
    return counts;
  }, [recentCalls]);

  // getRecentLlmCalls 返回新→旧（repositories_pipeline.rs L1339 DESC），取 20 条反转为时间正序
  const tokenSeries = useMemo(
    () => [...recentCalls.slice(0, 20)].reverse().map(c => c.total_tokens || 0),
    [recentCalls]
  );

  if (isLoading) {
    return (
      <div className={cn('flex items-center justify-center', embedded ? 'py-16' : 'p-8 h-full')}>
        <Loader2 className="w-8 h-8 text-cinema-gold animate-spin" />
      </div>
    );
  }

  const formatTokens = (n: number) => {
    if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
    if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
    return String(n);
  };

  const formatCost = (c: number) => {
    if (c >= 1) return `$${c.toFixed(2)}`;
    if (c > 0) return `$${c.toFixed(4)}`;
    return '$0';
  };

  return (
    <div className={cn(embedded ? 'space-y-6' : 'p-8 space-y-6 animate-fade-in')}>
      {!embedded && (
        <div className="flex items-center justify-between">
          <div>
            <h1 className="font-display text-3xl font-bold text-white">用量统计</h1>
            <p className="text-gray-400">
              {currentStory ? `${currentStory.title} - ` : ''}LLM 调用与 Token 消耗概览
            </p>
          </div>
        </div>
      )}

      {/* Operation grouping tabs */}
      <div className="flex flex-wrap items-center gap-2">
        <AiFilterChipsBar
          ariaLabel="调用分组筛选"
          activeKey={operationTab}
          onSelect={key => setOperationTab(key as OperationTab)}
          items={(['all', 'bootstrap', 'smart_execute', 'other'] as OperationTab[]).map(tab => ({
            key: tab,
            label: TAB_LABELS[tab],
            count: operationCounts[tab],
          }))}
        />
        <span className="inline-flex items-center gap-1 text-xs text-cinema-500 ml-2">
          <Info className="w-3 h-3" />
          分组基于 purpose / task_type / metadata（含 JSON 中 operation、label
          等字段）关键词启发式推断
        </span>
      </div>

      {/* Stats Cards */}
      <AiInsightCards
        columns={4}
        items={[
          {
            key: 'calls',
            label: '总调用次数',
            value: String(globalStats?.count ?? 0),
            tone: 'accent',
            icon: <Hash size={20} />,
            sub: storyStats != null ? `本故事: ${storyStats.count}` : undefined,
          },
          {
            key: 'tokens',
            label: '总 Token 数',
            value: formatTokens(globalStats?.total_tokens ?? 0),
            tone: 'neutral',
            icon: <Activity size={20} />,
            sub:
              storyStats != null ? `本故事: ${formatTokens(storyStats.total_tokens)}` : undefined,
            series: tokenSeries,
            seriesLabel: '最近调用 token 趋势',
          },
          {
            key: 'cost',
            label: '预估费用',
            value: formatCost(globalStats?.total_cost ?? 0),
            tone: 'green',
            icon: <Coins size={20} />,
            sub: storyStats != null ? `本故事: ${formatCost(storyStats.total_cost)}` : undefined,
          },
          {
            key: 'success',
            label: '成功率',
            value:
              recentCalls.length > 0
                ? `${Math.round((recentCalls.filter(c => c.success).length / recentCalls.length) * 100)}%`
                : 'N/A',
            tone: 'orange',
            icon: <BarChart3 size={20} />,
            sub: `基于最近 ${recentCalls.length} 次调用`,
          },
        ]}
      />

      {/* v0.86.0 A3：角色用量（agency 四角色 + 幕前/规划） */}
      {roleUsage.length > 0 && (
        <Card>
          <CardContent className="p-5">
            <div className="mb-4 flex items-center gap-2">
              <Users className="h-4 w-4 text-gray-400" />
              <h2 className="font-display text-lg font-semibold text-white">角色用量</h2>
              <span className="text-xs text-cinema-400">导演 / 管理 / 编审 / 主创</span>
            </div>
            <div
              className="grid grid-cols-1 gap-2 md:grid-cols-2 xl:grid-cols-4"
              data-testid="role-usage-table"
            >
              {roleUsage.slice(0, 8).map(row => (
                <div
                  key={row.label}
                  className="rounded-lg border border-borderSubtle bg-cinema-800/40 p-3"
                >
                  <p className="text-sm font-medium text-ai-ink">{roleUsageLabel(row.label)}</p>
                  <p className="mt-1 text-xs text-ai-ink-3">
                    {row.count} 次 · {formatTokens(row.total_tokens)} tokens ·{' '}
                    {formatCost(row.total_cost)}
                  </p>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      )}

      {/* Recent Calls Table */}
      <Card>
        <CardContent className="p-5">
          <div className="flex items-center justify-between mb-4">
            <div className="flex items-center gap-2">
              <Clock className="w-4 h-4 text-gray-400" />
              <h2 className="font-display text-lg font-semibold text-white">最近调用</h2>
            </div>
            <div className="text-xs text-cinema-400">
              当前分组：{filteredStats.count} 次 / {formatTokens(filteredStats.total_tokens)} tokens
              {filteredStats.success_rate != null && ` / ${filteredStats.success_rate}% 成功`}
            </div>
          </div>

          <AiFilterTable
            columns={[
              {
                key: 'purpose',
                label: '用途',
                width: '1.6fr',
                render: call => <span className="text-ai-ink">{call.purpose}</span>,
              },
              {
                key: 'operation',
                label: '操作',
                width: '0.8fr',
                render: call => (
                  <span className="text-ai-ink-2">{TAB_LABELS[deriveOperation(call)]}</span>
                ),
              },
              {
                key: 'model',
                label: '模型',
                width: '1fr',
                render: call => (
                  <span className="text-ai-ink-2">{call.model_name || call.model_id}</span>
                ),
              },
              {
                key: 'tokens',
                label: 'Token',
                align: 'right',
                width: '0.7fr',
                render: call => (
                  <span className="text-ai-ink-2 tabular-nums">
                    {call.total_tokens.toLocaleString()}
                  </span>
                ),
              },
              {
                key: 'duration',
                label: '耗时',
                align: 'right',
                width: '0.6fr',
                render: call => (
                  <span className="text-ai-ink-2 tabular-nums">
                    {call.duration_ms >= 1000
                      ? `${(call.duration_ms / 1000).toFixed(1)}s`
                      : `${call.duration_ms}ms`}
                  </span>
                ),
              },
              {
                key: 'status',
                label: '状态',
                align: 'center',
                width: '0.5fr',
                render: call =>
                  call.success ? (
                    <CheckCircle className="mx-auto h-4 w-4 text-ai-green" />
                  ) : (
                    <XCircle className="mx-auto h-4 w-4 text-ai-red" />
                  ),
              },
              {
                key: 'time',
                label: '时间',
                width: '1.1fr',
                render: call => (
                  <span className="text-xs text-ai-ink-3">
                    {new Date(call.created_at).toLocaleString()}
                  </span>
                ),
              },
            ]}
            rows={filteredCalls}
            rowKey={call => call.id}
            emptyText="暂无 LLM 调用记录"
          />
        </CardContent>
      </Card>
    </div>
  );
}
