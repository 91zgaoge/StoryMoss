import { useState, useEffect, useMemo } from 'react';
import { cn } from '@/utils/cn';
import { useAppStore } from '@/stores/appStore';
import { getLlmCallStats, getRecentLlmCalls, getStoryLlmCalls } from '@/services/tauri';
import { Card, CardContent } from '@/components/ui/Card';
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
  bootstrap: 'bootstrap',
  smart_execute: 'smart_execute',
  other: '其他',
};

export function UsageStats() {
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
  const [isLoading, setIsLoading] = useState(true);
  const [operationTab, setOperationTab] = useState<OperationTab>('all');

  useEffect(() => {
    const fetchStats = async () => {
      setIsLoading(true);
      try {
        const [global, recent] = await Promise.all([
          getLlmCallStats('global').catch(() => null),
          getRecentLlmCalls(50).catch(() => [] as LlmCall[]),
        ]);
        setGlobalStats(global);
        setRecentCalls(recent);

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

  if (isLoading) {
    return (
      <div className="p-8 flex items-center justify-center h-full">
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
    <div className="p-8 space-y-6 animate-fade-in">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="font-display text-3xl font-bold text-white">用量统计</h1>
          <p className="text-gray-400">
            {currentStory ? `${currentStory.title} - ` : ''}LLM 调用与 Token 消耗概览
          </p>
        </div>
      </div>

      {/* Operation grouping tabs */}
      <div className="flex flex-wrap items-center gap-2">
        {(['all', 'bootstrap', 'smart_execute', 'other'] as OperationTab[]).map(tab => (
          <button
            key={tab}
            onClick={() => setOperationTab(tab)}
            className={cn(
              'px-3 py-1.5 text-sm font-medium rounded-lg border transition-colors',
              operationTab === tab
                ? 'bg-cinema-gold/20 text-cinema-gold border-cinema-gold/30'
                : 'bg-cinema-900 border-cinema-700 text-cinema-300 hover:bg-cinema-800'
            )}
          >
            {TAB_LABELS[tab]}
          </button>
        ))}
        <span className="inline-flex items-center gap-1 text-xs text-cinema-500 ml-2">
          <Info className="w-3 h-3" />
          分组基于 purpose / task_type / metadata（含 JSON 中 operation、label
          等字段）关键词启发式推断
        </span>
      </div>

      {/* Stats Cards */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        <Card>
          <CardContent className="p-5">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-xs text-gray-500 uppercase tracking-wider">总调用次数</p>
                <p className="text-2xl font-bold text-white mt-1">{globalStats?.count ?? 0}</p>
              </div>
              <Hash className="w-8 h-8 text-cinema-gold/40" />
            </div>
            {storyStats != null && (
              <p className="text-xs text-cinema-gold/60 mt-2">本故事: {storyStats.count}</p>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardContent className="p-5">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-xs text-gray-500 uppercase tracking-wider">总 Token 数</p>
                <p className="text-2xl font-bold text-white mt-1">
                  {formatTokens(globalStats?.total_tokens ?? 0)}
                </p>
              </div>
              <Activity className="w-8 h-8 text-blue-400/40" />
            </div>
            {storyStats != null && (
              <p className="text-xs text-blue-400/60 mt-2">
                本故事: {formatTokens(storyStats.total_tokens)}
              </p>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardContent className="p-5">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-xs text-gray-500 uppercase tracking-wider">预估费用</p>
                <p className="text-2xl font-bold text-white mt-1">
                  {formatCost(globalStats?.total_cost ?? 0)}
                </p>
              </div>
              <Coins className="w-8 h-8 text-green-400/40" />
            </div>
            {storyStats != null && (
              <p className="text-xs text-green-400/60 mt-2">
                本故事: {formatCost(storyStats.total_cost)}
              </p>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardContent className="p-5">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-xs text-gray-500 uppercase tracking-wider">成功率</p>
                <p className="text-2xl font-bold text-white mt-1">
                  {recentCalls.length > 0
                    ? `${Math.round((recentCalls.filter(c => c.success).length / recentCalls.length) * 100)}%`
                    : 'N/A'}
                </p>
              </div>
              <BarChart3 className="w-8 h-8 text-purple-400/40" />
            </div>
            <p className="text-xs text-gray-600 mt-2">基于最近 {recentCalls.length} 次调用</p>
          </CardContent>
        </Card>
      </div>

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

          {filteredCalls.length === 0 ? (
            <div className="text-center py-8 text-gray-500">暂无 LLM 调用记录</div>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-cinema-700">
                    <th className="text-left py-2 px-3 text-gray-500 font-medium">用途</th>
                    <th className="text-left py-2 px-3 text-gray-500 font-medium">操作</th>
                    <th className="text-left py-2 px-3 text-gray-500 font-medium">模型</th>
                    <th className="text-right py-2 px-3 text-gray-500 font-medium">Token</th>
                    <th className="text-right py-2 px-3 text-gray-500 font-medium">耗时</th>
                    <th className="text-center py-2 px-3 text-gray-500 font-medium">状态</th>
                    <th className="text-left py-2 px-3 text-gray-500 font-medium">时间</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-cinema-800">
                  {filteredCalls.map(call => (
                    <tr key={call.id} className="hover:bg-cinema-800/30 transition-colors">
                      <td className="py-2 px-3 text-white/80">{call.purpose}</td>
                      <td className="py-2 px-3 text-gray-400">
                        {TAB_LABELS[deriveOperation(call)]}
                      </td>
                      <td className="py-2 px-3 text-gray-400">
                        {call.model_name || call.model_id}
                      </td>
                      <td className="py-2 px-3 text-right text-gray-400">
                        {call.total_tokens.toLocaleString()}
                      </td>
                      <td className="py-2 px-3 text-right text-gray-400">
                        {call.duration_ms >= 1000
                          ? `${(call.duration_ms / 1000).toFixed(1)}s`
                          : `${call.duration_ms}ms`}
                      </td>
                      <td className="py-2 px-3 text-center">
                        {call.success ? (
                          <CheckCircle className="w-4 h-4 text-green-400 mx-auto" />
                        ) : (
                          <XCircle className="w-4 h-4 text-red-400 mx-auto" />
                        )}
                      </td>
                      <td className="py-2 px-3 text-gray-500 text-xs">
                        {new Date(call.created_at).toLocaleString()}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
