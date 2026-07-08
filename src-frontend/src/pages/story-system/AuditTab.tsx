import { useState } from 'react';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { ShieldAlert, Activity, Loader2 } from 'lucide-react';
import { auditStory } from '@/services/tauri';
import type { StoryAnalysisReport } from '@/services/tauri';
import toast from 'react-hot-toast';

interface AuditTabProps {
  storyId: string;
}

export function AuditTab({ storyId }: AuditTabProps) {
  const [auditReport, setAuditReport] = useState<StoryAnalysisReport | null>(null);
  const [isAuditing, setIsAuditing] = useState(false);

  const handleAudit = async () => {
    setIsAuditing(true);
    try {
      const result = await auditStory(storyId);
      setAuditReport(result);
      toast.success('审计完成');
    } catch {
      toast.error('审计失败');
    } finally {
      setIsAuditing(false);
    }
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h3 className="text-lg font-semibold text-white flex items-center gap-2">
          <ShieldAlert className="w-5 h-5 text-orange-400" />
          叙事审计
        </h3>
        <Button size="sm" onClick={handleAudit} disabled={isAuditing}>
          {isAuditing ? (
            <Loader2 className="w-4 h-4 animate-spin mr-1" />
          ) : (
            <Activity className="w-4 h-4 mr-1" />
          )}
          运行全面审计
        </Button>
      </div>

      {auditReport ? (
        <div className="space-y-4">
          <Card>
            <CardContent className="p-4">
              <div className="flex items-center gap-4">
                <div className="text-4xl font-bold text-white">{auditReport.overall_score}</div>
                <div>
                  <p className="text-white font-medium">
                    {auditReport.overall_score >= 80
                      ? '结构健康'
                      : auditReport.overall_score >= 50
                        ? '需要关注'
                        : '问题较多'}
                  </p>
                  <p className="text-gray-500 text-sm">综合评分 (0-100)</p>
                </div>
              </div>
            </CardContent>
          </Card>

          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            {auditReport.dimensions.map(dim => (
              <Card key={dim.name}>
                <CardContent className="p-4">
                  <h4 className="text-white font-medium mb-2">{dim.name}</h4>
                  <div className="flex items-center gap-2 mb-2">
                    <div className="flex-1 h-3 bg-cinema-800 rounded-full overflow-hidden">
                      <div
                        className="h-full rounded-full transition-all"
                        style={{
                          width: `${dim.score}%`,
                          backgroundColor:
                            dim.score > 70 ? '#4ade80' : dim.score > 40 ? '#fbbf24' : '#f87171',
                        }}
                      />
                    </div>
                    <span className="text-white text-sm w-8 text-right">{dim.score}</span>
                  </div>
                  <p className="text-gray-500 text-xs">{dim.description}</p>
                  {dim.details.length > 0 && (
                    <ul className="mt-2 space-y-1">
                      {dim.details.map((d, i) => (
                        <li key={i} className="text-gray-400 text-xs">
                          {d}
                        </li>
                      ))}
                    </ul>
                  )}
                </CardContent>
              </Card>
            ))}
          </div>

          {auditReport.recommendations.length > 0 && (
            <Card>
              <CardContent className="p-4">
                <h4 className="text-white font-medium mb-3">建议</h4>
                <div className="space-y-2">
                  {auditReport.recommendations.map((rec, i) => (
                    <div key={i} className="p-2 bg-cinema-800 rounded text-gray-300 text-sm">
                      {rec}
                    </div>
                  ))}
                </div>
              </CardContent>
            </Card>
          )}
        </div>
      ) : (
        <Card>
          <CardContent className="p-4">
            <p className="text-gray-500 text-sm">点击“运行全面审计”开始分析故事结构健康度</p>
          </CardContent>
        </Card>
      )}
    </div>
  );
}
