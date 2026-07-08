import { useState, useEffect } from 'react';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { TrendingUp, Zap, Loader2 } from 'lucide-react';
import { evaluateReadingPower, getReadingPowerTrend, getChaseDebts } from '@/services/tauri';
import type { ReadingPowerEvaluation, ChaseDebt } from '@/services/tauri';
import toast from 'react-hot-toast';

interface ReadingPowerTabProps {
  storyId: string;
  selectedChapter: number;
  onChapterChange: (n: number) => void;
}

export function ReadingPowerTab({ storyId, selectedChapter, onChapterChange }: ReadingPowerTabProps) {
  const [readingTrend, setReadingTrend] = useState<ReadingPowerEvaluation[]>([]);
  const [chaseDebts, setChaseDebts] = useState<ChaseDebt[]>([]);
  const [isEvaluating, setIsEvaluating] = useState(false);

  const loadReadingPower = async () => {
    try {
      const [trend, debts] = await Promise.all([
        getReadingPowerTrend(storyId, 10),
        getChaseDebts(storyId),
      ]);
      setReadingTrend(trend);
      setChaseDebts(debts);
    } catch {
      // silent fail
    }
  };

  useEffect(() => {
    loadReadingPower();
  }, [storyId]);

  const handleEvaluateReadingPower = async () => {
    setIsEvaluating(true);
    try {
      const result = await evaluateReadingPower(storyId, selectedChapter);
      toast.success(`第${selectedChapter}章追读力评估完成: ${(result.score * 100).toFixed(0)}分`);
      loadReadingPower();
    } catch {
      toast.error('评估失败');
    } finally {
      setIsEvaluating(false);
    }
  };

  return (
    <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
      <Card>
        <CardContent className="p-4">
          <div className="flex items-center justify-between mb-4">
            <h3 className="text-lg font-semibold text-white flex items-center gap-2">
              <TrendingUp className="w-5 h-5 text-green-400" />
              追读力趋势
            </h3>
            <div className="flex items-center gap-2">
              <select
                value={selectedChapter}
                onChange={e => onChapterChange(Number(e.target.value))}
                className="bg-cinema-800 text-white text-sm rounded px-2 py-1 border border-cinema-700"
              >
                {Array.from({ length: 20 }, (_, i) => i + 1).map(n => (
                  <option key={n} value={n}>
                    第{n}章
                  </option>
                ))}
              </select>
              <Button size="sm" onClick={handleEvaluateReadingPower} disabled={isEvaluating}>
                {isEvaluating ? (
                  <Loader2 className="w-4 h-4 animate-spin mr-1" />
                ) : (
                  <Zap className="w-4 h-4 mr-1" />
                )}
                评估
              </Button>
            </div>
          </div>
          {readingTrend.length === 0 ? (
            <p className="text-gray-500 text-sm">暂无数据</p>
          ) : (
            <div className="space-y-3">
              {readingTrend.map(rp => (
                <div key={rp.chapter_number} className="flex items-center gap-3">
                  <span className="text-gray-400 text-sm w-12">Ch{rp.chapter_number}</span>
                  <div className="flex-1 h-6 bg-cinema-800 rounded-full overflow-hidden">
                    <div
                      className="h-full rounded-full transition-all"
                      style={{
                        width: `${rp.score * 100}%`,
                        backgroundColor:
                          rp.score > 0.7 ? '#4ade80' : rp.score > 0.4 ? '#fbbf24' : '#f87171',
                      }}
                    />
                  </div>
                  <span className="text-white text-sm w-10 text-right">
                    {(rp.score * 100).toFixed(0)}
                  </span>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardContent className="p-4">
          <h3 className="text-lg font-semibold text-white mb-4">追读债务</h3>
          {chaseDebts.length === 0 ? (
            <p className="text-gray-500 text-sm">无活跃债务</p>
          ) : (
            <div className="space-y-2">
              {chaseDebts.map(debt => (
                <div key={debt.id} className="p-3 bg-cinema-800 rounded-lg">
                  <p className="text-white text-sm">
                    {debt.debt_type} — 第{debt.source_chapter}章
                  </p>
                  <p className="text-gray-500 text-xs">
                    金额: {debt.current_amount} / 截止: 第{debt.due_chapter}章
                  </p>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
