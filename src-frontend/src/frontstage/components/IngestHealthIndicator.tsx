import React, { useEffect, useRef, useState } from 'react';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { X, Clock } from 'lucide-react';
import { getIngestJobs } from '@/services/tauri';
import type { IngestJob } from '@/types/v3';
import { cn } from '@/utils/cn';

interface Props {
  storyId: string | null;
}

const POLL_INTERVAL = 30000; // 30s fallback poll

/** 统一 VI 风格采摘图标：柔和漏斗 + 下箭头，表示知识/素材汇入 */
const IngestIcon: React.FC<{ className?: string }> = ({ className }) => (
  <svg
    className={className}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.75"
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden="true"
  >
    <path d="M5 5h14l-5.5 7v5.5l-3 1.5V12L5 5z" />
    <path d="M12 13.5V17" />
    <path d="M10.5 15.5 12 17l1.5-1.5" />
  </svg>
);

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
  const isRunning = !isHealthy && !isFailed;

  const statusClass = isFailed
    ? 'ingest-status--failed'
    : isHealthy
      ? 'ingest-status--healthy'
      : 'ingest-status--pending';

  const formatTime = (iso: string) => {
    const d = new Date(iso);
    return d.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' });
  };

  return (
    <div className="ingest-health-wrapper" ref={panelRef}>
      <button
        className={cn('ingest-health-btn', showPanel && 'active', statusClass)}
        onClick={() => setShowPanel(!showPanel)}
        title={isFailed ? `采摘失败: ${latest.error_message || '未知错误'}` : '采摘状态'}
        aria-label="采摘状态"
      >
        <IngestIcon className="ingest-health-icon" />
        <span className="ingest-health-dot" aria-hidden="true" />
      </button>

      {showPanel && (
        <div className="ingest-health-panel">
          <div className="ingest-health-panel__header">
            <span className="ingest-health-panel__title">采摘作业记录</span>
            <button
              onClick={() => setShowPanel(false)}
              className="ingest-health-panel__close"
              aria-label="关闭"
            >
              <X className="w-3 h-3" />
            </button>
          </div>
          <div className="ingest-health-panel__body">
            {jobs.slice(0, 3).map(job => (
              <div key={job.id} className="ingest-health-job">
                <div className="ingest-health-job__row">
                  <span className="ingest-health-job__type">{job.resource_type}</span>
                  <span
                    className={cn(
                      'ingest-health-job__badge',
                      job.status === 'completed' && 'ingest-health-job__badge--success',
                      job.status === 'failed' && 'ingest-health-job__badge--failed',
                      job.status !== 'completed' &&
                        job.status !== 'failed' &&
                        'ingest-health-job__badge--pending'
                    )}
                  >
                    {job.status === 'completed'
                      ? '成功'
                      : job.status === 'failed'
                        ? '失败'
                        : '运行中'}
                  </span>
                </div>
                <div className="ingest-health-job__meta">
                  <Clock className="w-3 h-3" />
                  <span>{formatTime(job.created_at)}</span>
                </div>
                {job.error_message && (
                  <div className="ingest-health-job__error">{job.error_message}</div>
                )}
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
};
