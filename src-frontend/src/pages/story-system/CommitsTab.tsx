import { useState, useEffect } from 'react';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { BookOpen, Plus, Loader2, Activity, CheckCircle, AlertTriangle } from 'lucide-react';
import { getChapterCommits, initChapterCommit, checkProjectionHealth } from '@/services/tauri';
import type { ChapterCommit } from '@/services/tauri';
import type { ProjectionHealthReport } from '@/types/v3';
import toast from 'react-hot-toast';

interface CommitsTabProps {
  storyId: string;
  selectedChapter: number;
}

export function CommitsTab({ storyId, selectedChapter }: CommitsTabProps) {
  const [commits, setCommits] = useState<ChapterCommit[]>([]);
  const [healthReports, setHealthReports] = useState<Record<number, ProjectionHealthReport>>({});
  const [checkingHealth, setCheckingHealth] = useState<Set<number>>(new Set());

  const loadCommits = async () => {
    try {
      const data = await getChapterCommits(storyId);
      setCommits(data);
    } catch {
      // silent fail
    }
  };

  useEffect(() => {
    loadCommits();
  }, [storyId]);

  const handleCheckHealth = async (chapterNumber: number) => {
    setCheckingHealth(prev => new Set(prev).add(chapterNumber));
    try {
      const report = await checkProjectionHealth(storyId, chapterNumber);
      setHealthReports(prev => ({ ...prev, [chapterNumber]: report }));
    } catch (e) {
      toast.error(`健康检查失败: ${e}`);
    } finally {
      setCheckingHealth(prev => {
        const next = new Set(prev);
        next.delete(chapterNumber);
        return next;
      });
    }
  };

  const handleInitCommit = async () => {
    try {
      await initChapterCommit(storyId, selectedChapter);
      toast.success(`第${selectedChapter}章提交已初始化`);
      loadCommits();
    } catch {
      toast.error('初始化提交失败');
    }
  };

  return (
    <Card>
      <CardContent className="p-4">
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-lg font-semibold text-white">章节提交链</h3>
          {commits.length === 0 && (
            <Button size="sm" onClick={handleInitCommit}>
              <Plus className="w-4 h-4 mr-1" />
              初始化提交
            </Button>
          )}
        </div>
        {commits.length === 0 ? (
          <p className="text-gray-500 text-sm">暂无提交记录</p>
        ) : (
          <div className="space-y-2">
            {commits.map(commit => {
              const report = healthReports[commit.chapter_number];
              const isChecking = checkingHealth.has(commit.chapter_number);
              return (
                <div key={commit.id} className="p-3 bg-cinema-800 rounded-lg">
                  <div className="flex items-center justify-between">
                    <div>
                      <p className="text-white text-sm font-medium">第{commit.chapter_number}章</p>
                      <p className="text-gray-500 text-xs">状态: {commit.status}</p>
                    </div>
                    <div className="flex items-center gap-2">
                      {report && (
                        <span
                          className={`text-xs px-2 py-0.5 rounded-full ${
                            report.overall_healthy
                              ? 'bg-green-500/20 text-green-400'
                              : 'bg-red-500/20 text-red-400'
                          }`}
                        >
                          {report.overall_healthy ? '健康' : '异常'}
                        </span>
                      )}
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={() => handleCheckHealth(commit.chapter_number)}
                        disabled={isChecking}
                      >
                        {isChecking ? (
                          <Loader2 className="w-3 h-3 animate-spin" />
                        ) : (
                          <Activity className="w-3 h-3" />
                        )}
                        <span className="ml-1 text-xs">健康检查</span>
                      </Button>
                    </div>
                  </div>
                  {report && (
                    <div className="mt-2 grid grid-cols-5 gap-2">
                      {report.writers.map((w: { name: string; status: string }) => (
                        <div
                          key={w.name}
                          className={`text-[10px] px-1.5 py-0.5 rounded text-center ${
                            w.status === 'success' || w.status.startsWith('skipped')
                              ? 'bg-green-500/10 text-green-400'
                              : w.status === 'pending'
                                ? 'bg-yellow-500/10 text-yellow-400'
                                : 'bg-red-500/10 text-red-400'
                          }`}
                          title={w.status}
                        >
                          <span className="capitalize">{w.name}</span>
                          {w.status === 'success' || w.status.startsWith('skipped') ? (
                            <CheckCircle className="w-3 h-3 mx-auto mt-0.5" />
                          ) : w.status === 'pending' ? (
                            <Loader2 className="w-3 h-3 mx-auto mt-0.5 animate-spin" />
                          ) : (
                            <AlertTriangle className="w-3 h-3 mx-auto mt-0.5" />
                          )}
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
