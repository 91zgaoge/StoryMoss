import React, { useState, useMemo } from 'react';
import { cn } from '@/utils/cn';
import {
  ArrowRight,
  Plus,
  Minus,
  FileText,
  GitCompare,
  Columns,
  AlignJustify,
  Copy,
  Check,
} from 'lucide-react';
import { Button } from '@/components/ui/Button';
import { Card, CardContent } from '@/components/ui/Card';
import type { VersionDiff as VersionDiffType } from '@/types';
import { useVersionDiff } from '@/hooks/useSceneVersions';

interface DiffLine {
  type: 'added' | 'removed' | 'unchanged';
  content: string;
  lineNumber: {
    old?: number;
    new?: number;
  };
}

interface DiffViewerProps {
  oldContent?: string;
  newContent?: string;
  oldLabel?: string;
  newLabel?: string;
  fromVersionId?: string;
  toVersionId?: string;
  className?: string;
}

type ViewMode = 'split' | 'unified';

// Simple diff algorithm using LCS
function computeDiff(oldText: string, newText: string): DiffLine[] {
  const oldLines = oldText.split('\n');
  const newLines = newText.split('\n');

  const lcs: number[][] = Array(oldLines.length + 1)
    .fill(null)
    .map(() => Array(newLines.length + 1).fill(0));

  for (let i = 1; i <= oldLines.length; i++) {
    for (let j = 1; j <= newLines.length; j++) {
      if (oldLines[i - 1] === newLines[j - 1]) {
        lcs[i][j] = lcs[i - 1][j - 1] + 1;
      } else {
        lcs[i][j] = Math.max(lcs[i - 1][j], lcs[i][j - 1]);
      }
    }
  }

  const result: Array<{
    type: 'added' | 'removed' | 'unchanged';
    content: string;
    oldNum?: number;
    newNum?: number;
  }> = [];
  let i = oldLines.length;
  let j = newLines.length;

  while (i > 0 || j > 0) {
    if (i > 0 && j > 0 && oldLines[i - 1] === newLines[j - 1]) {
      result.unshift({ type: 'unchanged', content: oldLines[i - 1], oldNum: i, newNum: j });
      i--;
      j--;
    } else if (j > 0 && (i === 0 || lcs[i][j - 1] >= lcs[i - 1][j])) {
      result.unshift({ type: 'added', content: newLines[j - 1], newNum: j });
      j--;
    } else {
      result.unshift({ type: 'removed', content: oldLines[i - 1], oldNum: i });
      i--;
    }
  }

  return result.map(r => ({
    type: r.type,
    content: r.content,
    lineNumber: { old: r.oldNum, new: r.newNum },
  }));
}

function DiffStats({ diff }: { diff: DiffLine[] }) {
  const stats = useMemo(() => {
    const added = diff.filter(d => d.type === 'added').length;
    const removed = diff.filter(d => d.type === 'removed').length;
    const unchanged = diff.filter(d => d.type === 'unchanged').length;
    return { added, removed, unchanged };
  }, [diff]);

  return (
    <div className="flex items-center gap-4 text-sm">
      <div className="flex items-center gap-1.5">
        <span className="w-2 h-2 rounded-full bg-green-500" />
        <span className="text-gray-400">新增</span>
        <span className="text-green-400 font-medium">{stats.added}</span>
      </div>
      <div className="flex items-center gap-1.5">
        <span className="w-2 h-2 rounded-full bg-red-500" />
        <span className="text-gray-400">删除</span>
        <span className="text-red-400 font-medium">{stats.removed}</span>
      </div>
      <div className="flex items-center gap-1.5">
        <span className="w-2 h-2 rounded-full bg-gray-500" />
        <span className="text-gray-400">未变</span>
        <span className="text-gray-300 font-medium">{stats.unchanged}</span>
      </div>
    </div>
  );
}

function SplitView({ diff }: { diff: DiffLine[] }) {
  return (
    <div className="grid grid-cols-2 divide-x divide-cinema-700">
      <div className="overflow-auto">
        <div className="text-xs font-medium text-gray-500 px-4 py-2 bg-cinema-800/50 border-b border-cinema-700">
          旧版本
        </div>
        <div className="font-mono text-sm">
          {diff.map((line, index) => (
            <div
              key={`old-${index}`}
              className={cn(
                'flex px-2 py-0.5',
                line.type === 'removed'
                  ? 'bg-red-500/10'
                  : line.type === 'unchanged'
                    ? 'hover:bg-cinema-800/50'
                    : 'opacity-30'
              )}
            >
              <span className="w-10 shrink-0 text-right text-gray-600 select-none pr-2">
                {line.lineNumber.old || ''}
              </span>
              <span
                className={cn(
                  'flex-1 whitespace-pre-wrap break-all',
                  line.type === 'removed' && 'text-red-300'
                )}
              >
                {line.type === 'removed' || line.type === 'unchanged' ? line.content : ''}
              </span>
            </div>
          ))}
        </div>
      </div>
      <div className="overflow-auto">
        <div className="text-xs font-medium text-gray-500 px-4 py-2 bg-cinema-800/50 border-b border-cinema-700">
          新版本
        </div>
        <div className="font-mono text-sm">
          {diff.map((line, index) => (
            <div
              key={`new-${index}`}
              className={cn(
                'flex px-2 py-0.5',
                line.type === 'added'
                  ? 'bg-green-500/10'
                  : line.type === 'unchanged'
                    ? 'hover:bg-cinema-800/50'
                    : 'opacity-30'
              )}
            >
              <span className="w-10 shrink-0 text-right text-gray-600 select-none pr-2">
                {line.lineNumber.new || ''}
              </span>
              <span
                className={cn(
                  'flex-1 whitespace-pre-wrap break-all',
                  line.type === 'added' && 'text-green-300'
                )}
              >
                {line.type === 'added' || line.type === 'unchanged' ? line.content : ''}
              </span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

function UnifiedView({ diff }: { diff: DiffLine[] }) {
  return (
    <div className="font-mono text-sm overflow-auto">
      {diff.map((line, index) => (
        <div
          key={index}
          className={cn(
            'flex px-2 py-0.5 hover:bg-cinema-800/30',
            line.type === 'added'
              ? 'bg-green-500/10'
              : line.type === 'removed'
                ? 'bg-red-500/10'
                : ''
          )}
        >
          <span className="w-10 shrink-0 text-right text-gray-600 select-none pr-2">
            {line.lineNumber.old || ''}
          </span>
          <span className="w-10 shrink-0 text-right text-gray-600 select-none pr-2">
            {line.lineNumber.new || ''}
          </span>
          <span className="w-6 shrink-0 text-center select-none">
            {line.type === 'added' && <Plus className="w-3 h-3 text-green-500 inline" />}
            {line.type === 'removed' && <Minus className="w-3 h-3 text-red-500 inline" />}
            {line.type === 'unchanged' && ' '}
          </span>
          <span
            className={cn(
              'flex-1 whitespace-pre-wrap break-all',
              line.type === 'added' && 'text-green-300',
              line.type === 'removed' && 'text-red-300'
            )}
          >
            {line.content}
          </span>
        </div>
      ))}
    </div>
  );
}

function VersionDiffMeta({ data }: { data: VersionDiffType | null | undefined }) {
  if (!data) return null;
  return (
    <div className="flex flex-wrap items-center gap-3 text-xs">
      {data.title_changed && (
        <span className="px-2 py-0.5 rounded bg-purple-500/20 text-purple-300">标题变更</span>
      )}
      {data.setting_changed && (
        <span className="px-2 py-0.5 rounded bg-blue-500/20 text-blue-300">场景变更</span>
      )}
      {data.characters_changed && (
        <span className="px-2 py-0.5 rounded bg-pink-500/20 text-pink-300">角色变更</span>
      )}
      {data.dramatic_goal_changed && (
        <span className="px-2 py-0.5 rounded bg-orange-500/20 text-orange-300">戏剧目标变更</span>
      )}
      <span
        className={cn(
          'px-2 py-0.5 rounded',
          data.word_count_delta >= 0
            ? 'bg-green-500/20 text-green-300'
            : 'bg-red-500/20 text-red-300'
        )}
      >
        字数 {data.word_count_delta >= 0 ? '+' : ''}
        {data.word_count_delta}
      </span>
      {data.confidence_delta !== 0 && (
        <span
          className={cn(
            'px-2 py-0.5 rounded',
            data.confidence_delta >= 0
              ? 'bg-green-500/20 text-green-300'
              : 'bg-red-500/20 text-red-300'
          )}
        >
          置信度 {data.confidence_delta >= 0 ? '+' : ''}
          {(data.confidence_delta * 100).toFixed(1)}%
        </span>
      )}
    </div>
  );
}

export function DiffViewer({
  oldContent = '',
  newContent = '',
  oldLabel = '旧版本',
  newLabel = '新版本',
  fromVersionId,
  toVersionId,
  className,
}: DiffViewerProps) {
  const [viewMode, setViewMode] = useState<ViewMode>('unified');
  const [copied, setCopied] = useState(false);

  const { data: versionDiffData, isLoading: isVersionDiffLoading } = useVersionDiff(
    fromVersionId || null,
    toVersionId || null
  );

  const diff = useMemo(() => computeDiff(oldContent, newContent), [oldContent, newContent]);

  const handleCopy = async () => {
    const text = diff
      .map(d => `${d.type === 'added' ? '+' : d.type === 'removed' ? '-' : ' '} ${d.content}`)
      .join('\n');
    await navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const isEmpty = diff.length === 0 || (diff.length === 1 && diff[0].content === '');

  return (
    <Card className={cn('overflow-hidden', className)}>
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 bg-cinema-800/50 border-b border-cinema-700">
        <div className="flex items-center gap-3">
          <GitCompare className="w-5 h-5 text-cinema-gold" />
          <div className="flex items-center gap-2">
            <span className="text-sm text-gray-400">{oldLabel}</span>
            <ArrowRight className="w-4 h-4 text-gray-600" />
            <span className="text-sm text-white">{newLabel}</span>
          </div>
        </div>

        <div className="flex items-center gap-2">
          {fromVersionId && toVersionId && versionDiffData && (
            <VersionDiffMeta data={versionDiffData} />
          )}
          {!isEmpty && <DiffStats diff={diff} />}

          <div className="flex items-center gap-1 ml-4 bg-cinema-700 rounded-lg p-1">
            <button
              onClick={() => setViewMode('unified')}
              className={cn(
                'p-1.5 rounded-md transition-colors',
                viewMode === 'unified'
                  ? 'bg-cinema-600 text-white'
                  : 'text-gray-400 hover:text-white'
              )}
              title="统一视图"
            >
              <AlignJustify className="w-4 h-4" />
            </button>
            <button
              onClick={() => setViewMode('split')}
              className={cn(
                'p-1.5 rounded-md transition-colors',
                viewMode === 'split' ? 'bg-cinema-600 text-white' : 'text-gray-400 hover:text-white'
              )}
              title="分栏视图"
            >
              <Columns className="w-4 h-4" />
            </button>
          </div>

          <Button
            variant="ghost"
            size="sm"
            className="h-8 w-8 p-0"
            onClick={handleCopy}
            title="复制差异"
          >
            {copied ? <Check className="w-4 h-4 text-green-400" /> : <Copy className="w-4 h-4" />}
          </Button>
        </div>
      </div>

      {/* Content */}
      <CardContent className="p-0 max-h-96 overflow-auto">
        {isEmpty ? (
          <div className="flex flex-col items-center justify-center py-12 text-gray-500">
            <FileText className="w-12 h-12 mb-3 opacity-30" />
            <p>没有差异</p>
            <p className="text-sm mt-1">两个版本内容相同</p>
          </div>
        ) : viewMode === 'split' ? (
          <SplitView diff={diff} />
        ) : (
          <UnifiedView diff={diff} />
        )}
      </CardContent>
    </Card>
  );
}

export default DiffViewer;
