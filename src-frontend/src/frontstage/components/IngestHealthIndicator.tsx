import React, { useEffect, useRef, useState } from 'react';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { Brain, CheckCircle, AlertTriangle, X, Clock } from 'lucide-react';
import { getIngestJobs } from '@/services/tauri';
import type { IngestJob } from '@/types/v3';
import { cn } from '@/utils/cn';

interface Props {
  storyId: string | null;
}

const POLL_INTERVAL = 30000; // 30s fallback poll

export const IngestHealthIndicator: React.FC<Props> = ({ storyId }) => {
  const [jobs, setJobs] = useState<IngestJob[]>([]);
  const [showPanel, setShowPanel] = useState(false);
  const panelRef = useRef<HTMLDivElement>(null);

  const fetchJobs = React.useCallback(async () => {
    if (!storyId) return;
    try {
      const data = await getIngestJobs(storyId, 5);
      setJobs(data);
    } catch (e) {
      // silent fail
    }
  }, [storyId]);

  useEffect(() => {
    if (!storyId) {
      setJobs([]);
      return;
    }
    fetchJobs();
    const interval = setInterval(fetchJobs, POLL_INTERVAL);
    return () => clearInterval(interval);
  }, [storyId, fetchJobs]);

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    const setup = async () => {
      unlisten = await listen<{ story_id: string }>('ingest-job-updated', event => {
        if (event.payload.story_id === storyId) {
          fetchJobs();
        }
      });
    };
    setup();
    return () => {
      if (unlisten) unlisten();
    };
  }, [storyId, fetchJobs]);

  // 点击外部关闭面板
  useEffect(() => {
    if (!showPanel) return;
    const handleClick = (e: MouseEvent) => {
      if (panelRef.current && !panelRef.current.contains(e.target as Node)) {
        setShowPanel(false);
      }
    };
    document.addEventListener('mousedown', handleClick);
    return () => document.removeEventListener('mousedown', handleClick);
  }, [showPanel]);

  if (!storyId || jobs.length === 0) return null;

  const latest = jobs[0];
  const isHealthy = latest.status === 'completed';
  const isFailed = latest.status === 'failed';

  const statusColor = isFailed ? 'text-amber-500' : isHealthy ? 'text-green-500' : 'text-slate-400';

  const StatusIcon = isFailed ? AlertTriangle : CheckCircle;

  const formatTime = (iso: string) => {
    const d = new Date(iso);
    return d.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' });
  };

  return (
    <div className="relative" ref={panelRef}>
      <button
        className={cn(
          'flex items-center gap-1 px-1.5 py-0.5 rounded-md text-xs transition-colors hover:bg-white/10',
          statusColor
        )}
        onClick={() => setShowPanel(!showPanel)}
        title={isFailed ? `Ingest 失败: ${latest.error_message || '未知错误'}` : 'Ingest 状态'}
      >
        <Brain className="w-3.5 h-3.5" />
        <StatusIcon className="w-3 h-3" />
      </button>

      {showPanel && (
        <div className="absolute right-0 top-full mt-1 w-64 bg-slate-800 border border-slate-700 rounded-lg shadow-xl z-50 overflow-hidden">
          <div className="flex items-center justify-between px-3 py-2 border-b border-slate-700">
            <span className="text-xs font-medium text-slate-200">Ingest 作业记录</span>
            <button
              onClick={() => setShowPanel(false)}
              className="text-slate-400 hover:text-slate-200"
            >
              <X className="w-3 h-3" />
            </button>
          </div>
          <div className="max-h-48 overflow-y-auto">
            {jobs.slice(0, 3).map(job => (
              <div key={job.id} className="px-3 py-2 border-b border-slate-700/50 last:border-0">
                <div className="flex items-center justify-between">
                  <span className="text-xs text-slate-300 capitalize">{job.resource_type}</span>
                  <span
                    className={cn(
                      'text-[10px] px-1.5 py-0.5 rounded-full',
                      job.status === 'completed'
                        ? 'bg-green-500/20 text-green-400'
                        : job.status === 'failed'
                          ? 'bg-red-500/20 text-red-400'
                          : 'bg-slate-500/20 text-slate-400'
                    )}
                  >
                    {job.status === 'completed'
                      ? '成功'
                      : job.status === 'failed'
                        ? '失败'
                        : '运行中'}
                  </span>
                </div>
                <div className="flex items-center gap-1 mt-1 text-[10px] text-slate-500">
                  <Clock className="w-3 h-3" />
                  <span>{formatTime(job.created_at)}</span>
                </div>
                {job.error_message && (
                  <div className="mt-1 text-[10px] text-red-400 line-clamp-2">
                    {job.error_message}
                  </div>
                )}
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
};
