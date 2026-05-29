import { useState } from 'react';
import { Zap, Loader2, Search, Check, ArrowRight, AlertTriangle } from 'lucide-react';
import { Button } from '@/components/ui/Button';
import { useAuditScene } from '@/hooks/useAudit';

interface SceneAuditPanelProps {
  sceneId: string;
  onPromoteToFinal: () => void;
  onBackToDrafting: () => void;
}

export function SceneAuditPanel({
  sceneId,
  onPromoteToFinal,
  onBackToDrafting,
}: SceneAuditPanelProps) {
  const [auditEnabled, setAuditEnabled] = useState(false);
  const { data: auditReport, isLoading: auditLoading } = useAuditScene(
    sceneId,
    'light',
    auditEnabled
  );

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-medium text-white flex items-center gap-2">
          <Search className="w-4 h-4 text-cinema-gold" />
          审校
        </h3>
        <Button
          variant="primary"
          size="sm"
          onClick={() => setAuditEnabled(true)}
          disabled={auditLoading}
        >
          {auditLoading ? (
            <Loader2 className="w-4 h-4 mr-1 animate-spin" />
          ) : (
            <Zap className="w-4 h-4 mr-1" />
          )}
          运行审校
        </Button>
      </div>

      {auditLoading && (
        <div className="p-8 bg-cinema-800/50 border border-cinema-700 rounded-lg text-center">
          <Loader2 className="w-8 h-8 text-cinema-gold mx-auto mb-3 animate-spin" />
          <p className="text-gray-400">正在运行 AI 审校...</p>
          <p className="text-xs text-gray-600 mt-1">检查逻辑一致性、人物连贯性、文风质量等</p>
        </div>
      )}

      {!auditLoading && !auditReport && (
        <div className="p-8 bg-cinema-800/50 border border-cinema-700 rounded-lg text-center">
          <Search className="w-12 h-12 text-cinema-700 mx-auto mb-4" />
          <p className="text-gray-400 mb-2">点击上方按钮运行审校</p>
          <ul className="text-sm text-gray-500 space-y-1">
            <li>• 逻辑一致性</li>
            <li>• 人物行为连贯性</li>
            <li>• 世界观规则遵守情况</li>
            <li>• 文风质量评估</li>
            <li>• 伏笔回收检查</li>
          </ul>
        </div>
      )}

      {!auditLoading && auditReport && (
        <div className="space-y-4">
          {/* Overall Score */}
          <div className="p-4 bg-cinema-800/50 border border-cinema-700 rounded-lg">
            <div className="flex items-center justify-between mb-3">
              <span className="text-sm font-medium text-white">综合评分</span>
              <span
                className={`text-lg font-bold ${
                  auditReport.overall_score >= 0.8
                    ? 'text-green-400'
                    : auditReport.overall_score >= 0.6
                      ? 'text-amber-400'
                      : 'text-red-400'
                }`}
              >
                {(auditReport.overall_score * 100).toFixed(0)}分
              </span>
            </div>
            <div className="h-2 bg-cinema-700 rounded-full overflow-hidden">
              <div
                className={`h-full rounded-full transition-all ${
                  auditReport.overall_score >= 0.8
                    ? 'bg-green-500'
                    : auditReport.overall_score >= 0.6
                      ? 'bg-amber-500'
                      : 'bg-red-500'
                }`}
                style={{ width: `${auditReport.overall_score * 100}%` }}
              />
            </div>
            {auditReport.has_blocking_issues && (
              <div className="mt-2 flex items-center gap-2 text-xs text-red-400">
                <AlertTriangle className="w-4 h-4" />
                <span>发现阻塞性问题，建议先修复再定稿</span>
              </div>
            )}
          </div>

          {/* Dimensions */}
          {auditReport.dimensions.map(dim => (
            <div
              key={dim.name}
              className="p-3 bg-cinema-800/30 border border-cinema-700/50 rounded-lg"
            >
              <div className="flex items-center justify-between mb-2">
                <span className="text-sm text-gray-300">{dim.name}</span>
                <span
                  className={`text-sm font-medium ${
                    dim.score >= 0.8
                      ? 'text-green-400'
                      : dim.score >= 0.6
                        ? 'text-amber-400'
                        : 'text-red-400'
                  }`}
                >
                  {(dim.score * 100).toFixed(0)}分
                </span>
              </div>
              <div className="h-1.5 bg-cinema-700 rounded-full overflow-hidden mb-2">
                <div
                  className={`h-full rounded-full ${
                    dim.score >= 0.8
                      ? 'bg-green-500'
                      : dim.score >= 0.6
                        ? 'bg-amber-500'
                        : 'bg-red-500'
                  }`}
                  style={{ width: `${dim.score * 100}%` }}
                />
              </div>
              {dim.issues.length > 0 && (
                <div className="space-y-1.5">
                  {dim.issues.map((issue, idx) => (
                    <div
                      key={idx}
                      className={`text-xs p-2 rounded ${
                        issue.severity === 'blocking'
                          ? 'bg-red-500/10 text-red-300 border border-red-500/20'
                          : issue.severity === 'warning'
                            ? 'bg-amber-500/10 text-amber-300 border border-amber-500/20'
                            : 'bg-blue-500/10 text-blue-300 border border-blue-500/20'
                      }`}
                    >
                      <div className="font-medium mb-0.5">
                        {issue.severity === 'blocking'
                          ? '🔴 阻塞'
                          : issue.severity === 'warning'
                            ? '🟡 警告'
                            : '🔵 提示'}{' '}
                        {issue.message}
                      </div>
                      {issue.suggestion && (
                        <div className="text-gray-400 pl-5">建议：{issue.suggestion}</div>
                      )}
                    </div>
                  ))}
                </div>
              )}
            </div>
          ))}
        </div>
      )}

      <div className="flex justify-between">
        <Button variant="secondary" size="sm" onClick={onBackToDrafting}>
          返回起草
        </Button>
        <Button variant="primary" size="sm" onClick={onPromoteToFinal}>
          <Check className="w-4 h-4 mr-1" />
          确认定稿
          <ArrowRight className="w-4 h-4 ml-2" />
        </Button>
      </div>
    </div>
  );
}
