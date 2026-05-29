import React, { useState, useMemo } from 'react';
import {
  History,
  User,
  Bot,
  Settings,
  GitCompare,
  RotateCcw,
  Trash2,
  Check,
  FileText,
  BarChart3,
  X,
  Plus,
  Minus,
  Edit3,
  Network,
} from 'lucide-react';
import { Button } from '@/components/ui/Button';
import { Card, CardContent } from '@/components/ui/Card';
import { cn } from '@/utils/cn';
import type { SceneVersion, CreatorType, VersionStats, ChangeTrack } from '@/types';
import {
  useSceneVersions,
  useVersionStats,
  useVersionChain,
  useRestoreSceneVersion,
  useDeleteSceneVersion,
  getCreatorTypeLabel,
  getCreatorTypeColor,
  getConfidenceColor,
  getConfidenceLabel,
  formatVersionNumber,
  calculateWordCountDelta,
} from '@/hooks/useSceneVersions';
import { useVersionChangeTracks } from '@/hooks/useChangeTracking';
import { ConfidenceIndicator } from './ConfidenceIndicator';

interface VersionTimelineProps {
  sceneId: string;
  storyId: string;
  onVersionSelect?: (version: SceneVersion) => void;
  onCompare?: (v1: SceneVersion, v2: SceneVersion) => void;
}

interface VersionItemProps {
  version: SceneVersion;
  previousVersion?: SceneVersion;
  isSelected: boolean;
  isCompareSelected: boolean;
  selectionMode: 'none' | 'single' | 'compare';
  onSelect: () => void;
  onToggleCompare: () => void;
  onRestore: () => void;
  onDelete: () => void;
}

function CreatorBadge({ type }: { type: CreatorType }) {
  const icons: Record<CreatorType, React.ReactNode> = {
    user: <User className="w-3 h-3" />,
    ai: <Bot className="w-3 h-3" />,
    system: <Settings className="w-3 h-3" />,
  };

  return (
    <span
      className="inline-flex items-center gap-1 px-2 py-0.5 text-xs rounded-full font-medium"
      style={{
        backgroundColor: `${getCreatorTypeColor(type)}20`,
        color: getCreatorTypeColor(type),
      }}
    >
      {icons[type]}
      {getCreatorTypeLabel(type)}
    </span>
  );
}

function VersionItem({
  version,
  previousVersion,
  isSelected,
  isCompareSelected,
  selectionMode,
  onSelect,
  onToggleCompare,
  onRestore,
  onDelete,
}: VersionItemProps) {
  const wordDelta = useMemo(() => {
    if (!previousVersion) return null;
    return calculateWordCountDelta(version.word_count, previousVersion.word_count);
  }, [version.word_count, previousVersion]);

  return (
    <div
      className={cn(
        'relative flex items-start gap-4 p-4 rounded-xl border transition-all duration-200 cursor-pointer group',
        isSelected
          ? 'bg-cinema-gold/10 border-cinema-gold/50 ring-1 ring-cinema-gold/30'
          : isCompareSelected
            ? 'bg-blue-500/10 border-blue-500/50 ring-1 ring-blue-500/30'
            : 'bg-cinema-800/50 border-cinema-700/50 hover:bg-cinema-800 hover:border-cinema-600'
      )}
      onClick={onSelect}
    >
      {/* Timeline connector */}
      <div className="absolute left-6 top-0 -translate-y-1/2 w-px h-4 bg-cinema-700" />

      {/* Version number badge */}
      <div
        className={cn(
          'flex-shrink-0 w-10 h-10 rounded-full flex items-center justify-center text-sm font-bold transition-colors',
          isSelected
            ? 'bg-cinema-gold text-cinema-900'
            : isCompareSelected
              ? 'bg-blue-500 text-white'
              : 'bg-cinema-700 text-gray-300'
        )}
      >
        {formatVersionNumber(version.version_number)}
      </div>

      {/* Content */}
      <div className="flex-1 min-w-0 space-y-2">
        {/* Header row */}
        <div className="flex items-center justify-between gap-2">
          <div className="flex items-center gap-2 flex-wrap">
            <CreatorBadge type={version.created_by} />
            <span className="text-xs text-gray-500">
              {new Date(version.created_at).toLocaleString('zh-CN', {
                month: 'short',
                day: 'numeric',
                hour: '2-digit',
                minute: '2-digit',
              })}
            </span>
          </div>

          {/* Actions */}
          <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
            {selectionMode === 'compare' && (
              <Button
                variant={isCompareSelected ? 'primary' : 'ghost'}
                size="sm"
                className="h-7 px-2 text-xs"
                onClick={e => {
                  e.stopPropagation();
                  onToggleCompare();
                }}
              >
                {isCompareSelected ? (
                  <>
                    <Check className="w-3 h-3 mr-1" />
                    已选
                  </>
                ) : (
                  <>
                    <GitCompare className="w-3 h-3 mr-1" />
                    对比
                  </>
                )}
              </Button>
            )}
            <Button
              variant="ghost"
              size="sm"
              className="h-7 w-7 p-0"
              onClick={e => {
                e.stopPropagation();
                onRestore();
              }}
              title="恢复此版本"
            >
              <RotateCcw className="w-3.5 h-3.5" />
            </Button>
            <Button
              variant="ghost"
              size="sm"
              className="h-7 w-7 p-0 text-red-400 hover:text-red-300"
              onClick={e => {
                e.stopPropagation();
                onDelete();
              }}
              title="删除版本"
            >
              <Trash2 className="w-3.5 h-3.5" />
            </Button>
          </div>
        </div>

        {/* Change summary */}
        <p className="text-sm text-gray-300 line-clamp-2">
          {version.change_summary || '无变更说明'}
        </p>

        {/* Stats row */}
        <div className="flex items-center gap-4 flex-wrap">
          {/* Word count */}
          <div className="flex items-center gap-1.5 text-xs">
            <FileText className="w-3.5 h-3.5 text-gray-500" />
            <span className="text-gray-400">{version.word_count.toLocaleString()} 字</span>
            {wordDelta && (
              <span
                className={cn(
                  'font-medium',
                  wordDelta.isIncrease ? 'text-green-400' : 'text-red-400'
                )}
              >
                {wordDelta.isIncrease ? '+' : ''}
                {wordDelta.delta}
              </span>
            )}
          </div>

          {/* Confidence score */}
          {version.confidence_score !== undefined && (
            <ConfidenceIndicator score={version.confidence_score} size="sm" showLabel />
          )}

          {/* Model info */}
          {version.model_used && (
            <span className="text-xs text-gray-500">{version.model_used}</span>
          )}
        </div>
      </div>
    </div>
  );
}

function VersionStatsPanel({ stats }: { stats: VersionStats | null | undefined }) {
  if (!stats) return null;

  return (
    <Card className="bg-cinema-800/80">
      <CardContent className="p-4">
        <div className="flex items-center gap-2 mb-3">
          <BarChart3 className="w-4 h-4 text-cinema-gold" />
          <h4 className="text-sm font-medium text-white">版本统计</h4>
        </div>
        <div className="grid grid-cols-2 gap-3">
          <div className="bg-cinema-700/50 rounded-lg p-3">
            <div className="text-2xl font-bold text-white">{stats.total_versions}</div>
            <div className="text-xs text-gray-400">总版本数</div>
          </div>
          <div className="bg-cinema-700/50 rounded-lg p-3">
            <div className="text-2xl font-bold text-cinema-gold">
              {(stats.avg_confidence * 100).toFixed(0)}%
            </div>
            <div className="text-xs text-gray-400">平均置信度</div>
          </div>
          <div className="bg-cinema-700/50 rounded-lg p-3">
            <div className="flex items-center gap-1 text-sm">
              <span className="text-blue-400">{stats.user_edits}</span>
              <span className="text-green-400">{stats.ai_edits}</span>
              <span className="text-gray-400">{stats.system_edits}</span>
            </div>
            <div className="text-xs text-gray-400">用户/AI/系统</div>
          </div>
          <div className="bg-cinema-700/50 rounded-lg p-3">
            <div
              className={cn(
                'text-lg font-bold',
                stats.total_word_delta >= 0 ? 'text-green-400' : 'text-red-400'
              )}
            >
              {stats.total_word_delta >= 0 ? '+' : ''}
              {stats.total_word_delta}
            </div>
            <div className="text-xs text-gray-400">字数变化</div>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}

function ChangeTrackItem({ track }: { track: ChangeTrack }) {
  const isInsert = track.change_type === 'Insert';
  const isDelete = track.change_type === 'Delete';

  return (
    <div
      className={cn(
        'flex items-start gap-2 p-2 rounded-lg text-sm',
        isInsert && 'bg-green-500/10',
        isDelete && 'bg-red-500/10',
        track.change_type === 'Format' && 'bg-blue-500/10'
      )}
    >
      <span className="mt-0.5 shrink-0">
        {isInsert && <Plus className="w-3.5 h-3.5 text-green-400" />}
        {isDelete && <Minus className="w-3.5 h-3.5 text-red-400" />}
        {track.change_type === 'Format' && <Edit3 className="w-3.5 h-3.5 text-blue-400" />}
      </span>
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2 text-xs text-gray-500 mb-0.5">
          <span>{isInsert ? '插入' : isDelete ? '删除' : '格式'}</span>
          <span>·</span>
          <span>
            位置 {track.from_pos}-{track.to_pos}
          </span>
        </div>
        <p
          className={cn(
            'break-words',
            isInsert && 'text-green-300',
            isDelete && 'text-red-300 line-through',
            track.change_type === 'Format' && 'text-blue-300'
          )}
        >
          {track.content || ' '}
        </p>
      </div>
    </div>
  );
}

function VersionChangeTracksPanel({ versionId }: { versionId: string }) {
  const { data: tracks = [], isLoading } = useVersionChangeTracks(versionId);

  if (isLoading) {
    return <div className="p-4 text-center text-sm text-gray-500">加载变更追踪...</div>;
  }

  if (tracks.length === 0) {
    return <div className="p-4 text-center text-sm text-gray-500">此版本没有记录的变更追踪</div>;
  }

  return (
    <div className="space-y-2">
      {tracks.map(track => (
        <ChangeTrackItem key={track.id} track={track} />
      ))}
    </div>
  );
}

export function VersionTimeline({
  sceneId,
  storyId,
  onVersionSelect,
  onCompare,
}: VersionTimelineProps) {
  const [selectionMode, setSelectionMode] = useState<'none' | 'single' | 'compare'>('none');
  const [selectedVersion, setSelectedVersion] = useState<SceneVersion | null>(null);
  const [compareVersions, setCompareVersions] = useState<SceneVersion[]>([]);
  const [viewMode, setViewMode] = useState<'timeline' | 'chain'>('timeline');

  const { data: versions = [], isLoading, error } = useSceneVersions(sceneId);
  const { data: stats } = useVersionStats(sceneId);
  const { data: chainNodes = [] } = useVersionChain(sceneId);
  const restoreMutation = useRestoreSceneVersion();
  const deleteMutation = useDeleteSceneVersion();

  const sortedVersions = useMemo(() => {
    return [...versions].sort((a, b) => b.version_number - a.version_number);
  }, [versions]);

  const handleSelect = (version: SceneVersion) => {
    if (selectionMode === 'compare') {
      return; // In compare mode, use the compare toggle
    }
    setSelectedVersion(version);
    onVersionSelect?.(version);
  };

  const handleToggleCompare = (version: SceneVersion) => {
    setCompareVersions(prev => {
      const exists = prev.find(v => v.id === version.id);
      if (exists) {
        return prev.filter(v => v.id !== version.id);
      }
      if (prev.length >= 2) {
        return [prev[1], version];
      }
      return [...prev, version];
    });
  };

  const handleCompare = () => {
    if (compareVersions.length === 2) {
      onCompare?.(compareVersions[0], compareVersions[1]);
    }
  };

  const handleRestore = async (version: SceneVersion) => {
    if (confirm(`确定要恢复到 ${formatVersionNumber(version.version_number)} 吗？`)) {
      await restoreMutation.mutateAsync({
        sceneId,
        versionId: version.id,
        restoredBy: 'user',
      });
    }
  };

  const handleDelete = async (version: SceneVersion) => {
    if (confirm(`确定要删除 ${formatVersionNumber(version.version_number)} 吗？此操作不可撤销。`)) {
      await deleteMutation.mutateAsync({
        versionId: version.id,
        sceneId,
      });
    }
  };

  const cancelCompare = () => {
    setSelectionMode('none');
    setCompareVersions([]);
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64 text-gray-500">
        <div className="animate-spin w-6 h-6 border-2 border-cinema-gold border-t-transparent rounded-full mr-2" />
        加载版本历史...
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex items-center justify-center h-64 text-red-400">
        加载失败: {error.message}
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-2">
          <History className="w-5 h-5 text-cinema-gold" />
          <h3 className="text-lg font-semibold text-white">版本历史</h3>
          <span className="text-sm text-gray-500">({versions.length})</span>
        </div>

        <div className="flex items-center gap-2">
          <div className="flex items-center bg-cinema-800 rounded-lg p-1">
            <button
              onClick={() => setViewMode('timeline')}
              className={cn(
                'px-2 py-1 rounded text-xs font-medium transition-colors',
                viewMode === 'timeline'
                  ? 'bg-cinema-700 text-white'
                  : 'text-gray-400 hover:text-white'
              )}
            >
              时间线
            </button>
            <button
              onClick={() => setViewMode('chain')}
              className={cn(
                'px-2 py-1 rounded text-xs font-medium transition-colors',
                viewMode === 'chain' ? 'bg-cinema-700 text-white' : 'text-gray-400 hover:text-white'
              )}
            >
              版本链
            </button>
          </div>
          {selectionMode === 'compare' ? (
            <>
              <span className="text-sm text-gray-400">已选 {compareVersions.length}/2</span>
              <Button
                variant="primary"
                size="sm"
                disabled={compareVersions.length !== 2}
                onClick={handleCompare}
              >
                <GitCompare className="w-4 h-4 mr-1" />
                对比
              </Button>
              <Button variant="ghost" size="sm" onClick={cancelCompare}>
                <X className="w-4 h-4" />
              </Button>
            </>
          ) : (
            <Button
              variant="secondary"
              size="sm"
              onClick={() => setSelectionMode('compare')}
              disabled={versions.length < 2}
            >
              <GitCompare className="w-4 h-4 mr-1" />
              版本对比
            </Button>
          )}
        </div>
      </div>

      {/* Stats Panel */}
      <div className="mb-4">
        <VersionStatsPanel stats={stats} />
      </div>

      {/* Timeline / Chain View */}
      <div className="flex-1 overflow-auto space-y-1 relative">
        {viewMode === 'timeline' ? (
          <>
            {/* Timeline vertical line */}
            <div className="absolute left-9 top-4 bottom-4 w-px bg-cinema-700/50" />

            {sortedVersions.map((version, index) => (
              <VersionItem
                key={version.id}
                version={version}
                previousVersion={sortedVersions[index + 1]}
                isSelected={selectedVersion?.id === version.id && selectionMode !== 'compare'}
                isCompareSelected={compareVersions.some(v => v.id === version.id)}
                selectionMode={selectionMode}
                onSelect={() => handleSelect(version)}
                onToggleCompare={() => handleToggleCompare(version)}
                onRestore={() => handleRestore(version)}
                onDelete={() => handleDelete(version)}
              />
            ))}
          </>
        ) : (
          <div className="space-y-1">
            {chainNodes.map(node => (
              <div
                key={node.version.id}
                className={cn(
                  'flex items-center gap-3 p-3 rounded-xl border transition-colors cursor-pointer',
                  selectedVersion?.id === node.version.id && selectionMode !== 'compare'
                    ? 'bg-cinema-gold/10 border-cinema-gold/50'
                    : 'bg-cinema-800/50 border-cinema-700/50 hover:bg-cinema-800'
                )}
                style={{ marginLeft: `${node.depth * 24}px` }}
                onClick={() => handleSelect(node.version)}
              >
                <div className="flex-shrink-0 w-8 h-8 rounded-full bg-cinema-700 text-gray-300 flex items-center justify-center text-sm font-bold">
                  {formatVersionNumber(node.version.version_number)}
                </div>
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2 flex-wrap">
                    <CreatorBadge type={node.version.created_by} />
                    <span className="text-xs text-gray-500">
                      {new Date(node.version.created_at).toLocaleString('zh-CN', {
                        month: 'short',
                        day: 'numeric',
                      })}
                    </span>
                  </div>
                  <p className="text-sm text-gray-300 line-clamp-1 mt-0.5">
                    {node.version.change_summary || '无变更说明'}
                  </p>
                </div>
                {node.children.length > 0 && (
                  <span className="text-xs px-2 py-0.5 rounded-full bg-cinema-700 text-gray-400">
                    {node.children.length} 分支
                  </span>
                )}
              </div>
            ))}
            {chainNodes.length === 0 && (
              <div className="text-center py-8 text-gray-500">
                <Network className="w-12 h-12 mx-auto mb-3 opacity-30" />
                <p>暂无版本链数据</p>
              </div>
            )}
          </div>
        )}

        {versions.length === 0 && viewMode === 'timeline' && (
          <div className="text-center py-8 text-gray-500">
            <History className="w-12 h-12 mx-auto mb-3 opacity-30" />
            <p>暂无版本历史</p>
            <p className="text-sm mt-1">保存场景后将自动创建第一个版本</p>
          </div>
        )}
      </div>

      {/* Selected Version Change Tracks */}
      {selectedVersion && selectionMode !== 'compare' && (
        <div className="mt-4 pt-4 border-t border-cinema-700">
          <h4 className="text-sm font-medium text-white mb-3 flex items-center gap-2">
            <GitCompare className="w-4 h-4 text-cinema-gold" />
            版本变更详情 (v{selectedVersion.version_number})
          </h4>
          <VersionChangeTracksPanel versionId={selectedVersion.id} />
        </div>
      )}
    </div>
  );
}
