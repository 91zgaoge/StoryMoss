import { RefreshCw, Loader2, GitBranch, GitCommit, ArrowRight } from 'lucide-react';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { useWorkflows, useReloadWorkflows } from '@/hooks/useWorkflows';

export function WorkflowSettings() {
  const { data: workflows = [], isLoading } = useWorkflows();
  const reload = useReloadWorkflows();

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h3 className="font-display text-lg font-bold text-white">工作流模板</h3>
          <p className="text-sm text-gray-400">从文件系统自动加载的工作流定义（JSON/YAML）</p>
        </div>
        <Button
          variant="secondary"
          onClick={() => reload.mutate()}
          isLoading={reload.isPending}
          className="gap-2"
        >
          <RefreshCw className="w-4 h-4" />
          重新加载
        </Button>
      </div>

      {isLoading ? (
        <div className="flex items-center justify-center py-12">
          <Loader2 className="w-6 h-6 text-cinema-gold animate-spin" />
        </div>
      ) : workflows.length === 0 ? (
        <div className="text-center py-12 text-gray-500">
          <GitBranch className="w-8 h-8 mx-auto mb-3 opacity-50" />
          <p>暂无工作流模板</p>
          <p className="text-sm mt-1">
            在应用数据目录 workflows/ 文件夹中放入 .json 或 .yaml 文件即可自动加载
          </p>
        </div>
      ) : (
        <div className="space-y-3">
          {workflows.map(wf => (
            <Card key={wf.id} className="bg-cinema-900/50 border-cinema-800">
              <CardContent className="p-4">
                <div className="flex items-start justify-between gap-4">
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 mb-1">
                      <h4 className="font-medium text-white">{wf.name}</h4>
                      {wf.is_builtin && (
                        <span className="text-[10px] px-1.5 py-0.5 rounded bg-cinema-700 text-gray-400">
                          内置
                        </span>
                      )}
                    </div>
                    <p className="text-sm text-gray-400">{wf.description}</p>
                    <div className="flex items-center gap-4 mt-2 text-xs text-gray-500">
                      <span className="flex items-center gap-1">
                        <GitCommit className="w-3 h-3" />
                        {wf.nodes.length} 个节点
                      </span>
                      <span className="flex items-center gap-1">
                        <ArrowRight className="w-3 h-3" />
                        {wf.edges.length} 条边
                      </span>
                      <span>ID: {wf.id}</span>
                    </div>
                  </div>
                </div>

                {/* Node list */}
                <div className="mt-3 pt-3 border-t border-cinema-800">
                  <div className="flex flex-wrap gap-2">
                    {wf.nodes.map(node => (
                      <span
                        key={node.id}
                        className={`text-xs px-2 py-1 rounded border ${
                          node.node_type === 'Start' || node.node_type === 'End'
                            ? 'bg-cinema-800 border-cinema-700 text-gray-400'
                            : 'bg-cinema-gold/5 border-cinema-gold/20 text-cinema-gold'
                        }`}
                      >
                        {node.name}
                      </span>
                    ))}
                  </div>
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}
