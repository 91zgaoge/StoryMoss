import { useState, useEffect } from 'react';
import { loggedInvoke } from '@/services/tauri';
import { createLogger } from '@/utils/logger';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { BrainCircuit, Activity, Network, ArrowRight, Clock, AlertCircle } from 'lucide-react';
import toast from 'react-hot-toast';

const logger = createLogger('ui:IntentionGraphDiagnostics');

interface ExecutionSummary {
  id: string;
  request_id: string;
  user_input: string;
  status: string;
  created_at: string;
}

interface IntentionGraphDiagnosticsData {
  intention_count: number;
  asset_count: number;
  edge_count: number;
  recent_executions: ExecutionSummary[];
}

interface ExecutionGraphDetail {
  id: string;
  request_id: string;
  user_input: string;
  status: string;
  created_at: string;
  completed_at?: string;
  execution_time_ms?: number;
  plan_json?: string;
  result_json?: string;
}

export function IntentionGraphDiagnostics() {
  const [data, setData] = useState<IntentionGraphDiagnosticsData | null>(null);
  const [loading, setLoading] = useState(true);
  const [selectedGraph, setSelectedGraph] = useState<ExecutionGraphDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);

  const fetchDiagnostics = async () => {
    try {
      setLoading(true);
      const result = await loggedInvoke<IntentionGraphDiagnosticsData>(
        'get_intention_graph_diagnostics'
      );
      setData(result);
    } catch (error) {
      logger.error('Failed to fetch intention graph diagnostics', { error });
      toast.error('获取意图图诊断信息失败');
    } finally {
      setLoading(false);
    }
  };

  const fetchGraphDetail = async (graphId: string) => {
    try {
      setDetailLoading(true);
      const result = await loggedInvoke<ExecutionGraphDetail | null>('get_execution_graph_detail', {
        graph_id: graphId,
      });
      setSelectedGraph(result);
    } catch (error) {
      logger.error('Failed to fetch execution graph detail', { error, graphId });
      toast.error('获取执行图详情失败');
    } finally {
      setDetailLoading(false);
    }
  };

  useEffect(() => {
    fetchDiagnostics();
  }, []);

  const getStatusColor = (status: string) => {
    switch (status.toLowerCase()) {
      case 'completed':
        return 'bg-green-500/10 text-green-400 border-green-500/20';
      case 'failed':
        return 'bg-red-500/10 text-red-400 border-red-500/20';
      case 'building':
        return 'bg-amber-500/10 text-amber-400 border-amber-500/20';
      case 'executing':
        return 'bg-blue-500/10 text-blue-400 border-blue-500/20';
      default:
        return 'bg-gray-500/10 text-gray-400 border-gray-500/20';
    }
  };

  const getStatusLabel = (status: string) => {
    switch (status.toLowerCase()) {
      case 'completed':
        return '已完成';
      case 'failed':
        return '失败';
      case 'building':
        return '构建中';
      case 'executing':
        return '执行中';
      default:
        return status;
    }
  };

  const formatDate = (dateStr: string) => {
    try {
      return new Date(dateStr).toLocaleString('zh-CN');
    } catch {
      return dateStr;
    }
  };

  if (loading) {
    return (
      <div className="p-6 space-y-6">
        <div className="flex items-center gap-3 mb-6">
          <BrainCircuit className="w-6 h-6 text-cinema-gold" />
          <h1 className="text-2xl font-bold text-white">意图诊断</h1>
        </div>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          {[1, 2, 3].map(i => (
            <Card key={i}>
              <CardContent className="p-6">
                <div className="h-8 w-24 bg-cinema-700/50 rounded animate-pulse mb-2" />
                <div className="h-12 w-16 bg-cinema-700/50 rounded animate-pulse" />
              </CardContent>
            </Card>
          ))}
        </div>
        <div className="h-96 w-full bg-cinema-700/30 rounded-2xl animate-pulse" />
      </div>
    );
  }

  if (!data) {
    return (
      <div className="p-6">
        <div className="flex items-center gap-3 mb-6">
          <BrainCircuit className="w-6 h-6 text-cinema-gold" />
          <h1 className="text-2xl font-bold text-white">意图诊断</h1>
        </div>
        <Card>
          <CardContent className="p-8 text-center">
            <AlertCircle className="w-12 h-12 text-gray-500 mx-auto mb-4" />
            <p className="text-gray-400 mb-4">无法加载意图图诊断信息</p>
            <Button onClick={fetchDiagnostics} variant="ghost">
              重试
            </Button>
          </CardContent>
        </Card>
      </div>
    );
  }

  return (
    <div className="p-6 space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <BrainCircuit className="w-6 h-6 text-cinema-gold" />
          <h1 className="text-2xl font-bold text-white">意图诊断</h1>
        </div>
        <Button onClick={fetchDiagnostics} variant="ghost" size="sm">
          <Activity className="w-4 h-4 mr-2" />
          刷新
        </Button>
      </div>

      {/* Stats Cards */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <Card>
          <CardContent className="p-6">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm text-gray-400 mb-1">意图节点</p>
                <p className="text-3xl font-bold text-white">{data.intention_count}</p>
              </div>
              <BrainCircuit className="w-8 h-8 text-cinema-gold/50" />
            </div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-6">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm text-gray-400 mb-1">资产节点</p>
                <p className="text-3xl font-bold text-white">{data.asset_count}</p>
              </div>
              <Network className="w-8 h-8 text-cinema-gold/50" />
            </div>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-6">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm text-gray-400 mb-1">边连接</p>
                <p className="text-3xl font-bold text-white">{data.edge_count}</p>
              </div>
              <Activity className="w-8 h-8 text-cinema-gold/50" />
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Recent Executions */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <Card>
          <div className="p-4 border-b border-cinema-800">
            <h2 className="flex items-center gap-2 text-lg font-semibold text-white">
              <Clock className="w-5 h-5 text-cinema-gold" />
              最近执行记录
            </h2>
          </div>
          <CardContent className="p-0">
            {data.recent_executions.length === 0 ? (
              <div className="p-6 text-center text-gray-400">暂无执行记录</div>
            ) : (
              <div className="divide-y divide-cinema-800">
                {data.recent_executions.map(exec => (
                  <div
                    key={exec.id}
                    className="p-4 hover:bg-cinema-800/50 transition-colors cursor-pointer group"
                    onClick={() => fetchGraphDetail(exec.id)}
                  >
                    <div className="flex items-start justify-between gap-3">
                      <div className="flex-1 min-w-0">
                        <p className="text-sm text-white truncate mb-1" title={exec.user_input}>
                          {exec.user_input || '(空输入)'}
                        </p>
                        <div className="flex items-center gap-2 text-xs text-gray-500">
                          <span>{formatDate(exec.created_at)}</span>
                          <span className="text-gray-700">|</span>
                          <span className="font-mono">{exec.request_id.slice(0, 8)}</span>
                        </div>
                      </div>
                      <div className="flex items-center gap-2">
                        <span
                          className={`inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium border ${getStatusColor(exec.status)}`}
                        >
                          {getStatusLabel(exec.status)}
                        </span>
                        <ArrowRight className="w-4 h-4 text-gray-600 group-hover:text-cinema-gold transition-colors" />
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </CardContent>
        </Card>

        {/* Detail Panel */}
        <Card>
          <div className="p-4 border-b border-cinema-800">
            <h2 className="flex items-center gap-2 text-lg font-semibold text-white">
              <Network className="w-5 h-5 text-cinema-gold" />
              执行图详情
            </h2>
          </div>
          <CardContent>
            {detailLoading ? (
              <div className="space-y-3">
                <div className="h-4 w-full bg-cinema-700/50 rounded animate-pulse" />
                <div className="h-4 w-3/4 bg-cinema-700/50 rounded animate-pulse" />
                <div className="h-32 w-full bg-cinema-700/50 rounded animate-pulse" />
              </div>
            ) : selectedGraph ? (
              <div className="space-y-4">
                <div className="grid grid-cols-2 gap-3 text-sm">
                  <div>
                    <span className="text-gray-500">状态</span>
                    <span
                      className={`inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium border ${getStatusColor(selectedGraph.status)}`}
                    >
                      {getStatusLabel(selectedGraph.status)}
                    </span>
                  </div>
                  <div>
                    <span className="text-gray-500">执行时间</span>
                    <span className="ml-2 text-white">
                      {selectedGraph.execution_time_ms
                        ? `${selectedGraph.execution_time_ms}ms`
                        : '—'}
                    </span>
                  </div>
                  <div>
                    <span className="text-gray-500">创建时间</span>
                    <span className="ml-2 text-white">{formatDate(selectedGraph.created_at)}</span>
                  </div>
                  <div>
                    <span className="text-gray-500">完成时间</span>
                    <span className="ml-2 text-white">
                      {selectedGraph.completed_at ? formatDate(selectedGraph.completed_at) : '—'}
                    </span>
                  </div>
                </div>

                <div className="border-t border-cinema-800 pt-4">
                  <p className="text-sm text-gray-500 mb-2">用户输入</p>
                  <p className="text-sm text-white bg-cinema-900/50 p-3 rounded-lg">
                    {selectedGraph.user_input || '(空)'}
                  </p>
                </div>

                {selectedGraph.plan_json && (
                  <div className="border-t border-cinema-800 pt-4">
                    <p className="text-sm text-gray-500 mb-2">执行计划</p>
                    <pre className="text-xs text-gray-300 bg-cinema-900/50 p-3 rounded-lg overflow-auto max-h-48">
                      {(() => {
                        try {
                          return JSON.stringify(JSON.parse(selectedGraph.plan_json!), null, 2);
                        } catch {
                          return selectedGraph.plan_json;
                        }
                      })()}
                    </pre>
                  </div>
                )}

                {selectedGraph.result_json && (
                  <div className="border-t border-cinema-800 pt-4">
                    <p className="text-sm text-gray-500 mb-2">执行结果</p>
                    <pre className="text-xs text-gray-300 bg-cinema-900/50 p-3 rounded-lg overflow-auto max-h-48">
                      {(() => {
                        try {
                          return JSON.stringify(JSON.parse(selectedGraph.result_json!), null, 2);
                        } catch {
                          return selectedGraph.result_json;
                        }
                      })()}
                    </pre>
                  </div>
                )}
              </div>
            ) : (
              <div className="text-center text-gray-400 py-12">
                <Network className="w-12 h-12 mx-auto mb-4 opacity-30" />
                <p>点击左侧执行记录查看详情</p>
              </div>
            )}
          </CardContent>
        </Card>
      </div>

      {/* Info Section */}
      <Card>
        <div className="p-4 border-b border-cinema-800">
          <h2 className="flex items-center gap-2 text-lg font-semibold text-white">
            <BrainCircuit className="w-5 h-5 text-cinema-gold" />
            关于 SING 意图图
          </h2>
        </div>
        <CardContent>
          <div className="text-sm text-gray-400 space-y-2">
            <p>
              SING（Synthetic Intention Graph）是一种基于意图-工具异构图的智能创作调度系统。
              它将用户的自然语言输入合成为原子化意图，通过分层发现机制动态匹配最合适的创作工具（Agent、Skill、MCP）。
            </p>
            <p>
              意图图持续学习用户的创作习惯，通过执行反馈优化意图-资产关联，实现"越写越懂"的自适应进化。
            </p>
            <div className="flex gap-4 mt-4 text-xs">
              <span className="text-gray-500">论文: arXiv:2606.16591v2</span>
              <span className="text-gray-500">集成版本: v0.17.0</span>
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
