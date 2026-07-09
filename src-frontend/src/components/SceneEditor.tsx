import { useState, useEffect } from 'react';
import {
  Target,
  Zap,
  Users,
  MapPin,
  Sparkles,
  Save,
  X,
  GitCompare,
  EyeOff,
  MessageSquare,
  Check,
  Minimize2,
  Loader2,
  ClipboardList,
  PenTool,
  Search,
  Lock,
  ArrowRight,
  ChevronDown,
  ChevronUp,
  GitMerge,
} from 'lucide-react';
import { Button } from '@/components/ui/Button';
import { Card, CardContent } from '@/components/ui/Card';
import type { Scene, ConflictType } from '@/types';
import { getConflictTypeLabel, getConflictTypeColor } from '@/hooks/useScenes';
import { useCompressScene } from '@/hooks/useMemoryCompression';
import { loggedInvoke } from '@/services/tauri';
import { SceneAuditPanel } from './scene-editor/SceneAuditPanel';
import { SceneAnnotationPanel } from './scene-editor/SceneAnnotationPanel';
import { PipelinePanel } from './pipeline/PipelinePanel';
import { createLogger } from '@/utils/logger';
import { useQueryClient } from '@tanstack/react-query';
import toast from 'react-hot-toast';

const sceneEditorLogger = createLogger('ui:SceneEditor');

interface SceneEditorProps {
  scene: Scene | null;
  characters: { id: string; name: string; personality?: string }[];
  onSave: (updates: Partial<Scene>) => void;
  onCancel: () => void;
}

const CONFLICT_TYPES: ConflictType[] = [
  'ManVsMan',
  'ManVsSelf',
  'ManVsSociety',
  'ManVsNature',
  'ManVsTechnology',
  'ManVsFate',
  'ManVsSupernatural',
  'ManVsTime',
  'ManVsMorality',
  'ManVsIdentity',
  'FactionVsFaction',
];

type ExecutionStage = 'planning' | 'outline' | 'drafting' | 'review' | 'final';

const STAGE_TABS: { id: ExecutionStage | 'annotations'; label: string; icon: React.ElementType }[] =
  [
    { id: 'planning', label: '规划', icon: Target },
    { id: 'outline', label: '大纲', icon: ClipboardList },
    { id: 'drafting', label: '起草', icon: PenTool },
    { id: 'review', label: '审校', icon: Search },
    { id: 'final', label: '定稿', icon: Lock },
    { id: 'annotations', label: '批注', icon: MessageSquare },
  ];

const STAGE_LABELS: Record<ExecutionStage, string> = {
  planning: '规划',
  outline: '大纲',
  drafting: '起草',
  review: '审校',
  final: '定稿',
};

export function SceneEditor({ scene, characters, onSave, onCancel }: SceneEditorProps) {
  const queryClient = useQueryClient();
  const [formData, setFormData] = useState<Partial<Scene>>({});
  const [activeTab, setActiveTab] = useState<ExecutionStage | 'annotations'>('planning');
  const [revisionMode, setRevisionMode] = useState(false);
  const [compressionResult, setCompressionResult] = useState<import('@/types').AgentResult | null>(
    null
  );
  const [showCompression, setShowCompression] = useState(false);
  const [generatingOutline, setGeneratingOutline] = useState(false);
  const [generatingDraft, setGeneratingDraft] = useState(false);
  const [pipelineOpen, setPipelineOpen] = useState(true);
  const compressScene = useCompressScene();

  useEffect(() => {
    if (scene) {
      setFormData({
        title: scene.title,
        dramatic_goal: scene.dramatic_goal,
        external_pressure: scene.external_pressure,
        conflict_type: scene.conflict_type,
        characters_present: scene.characters_present,
        character_conflicts: scene.character_conflicts,
        setting_location: scene.setting_location,
        setting_time: scene.setting_time,
        setting_atmosphere: scene.setting_atmosphere,
        content: scene.content,
        confidence_score: scene.confidence_score,
        execution_stage: scene.execution_stage,
        outline_content: scene.outline_content,
        draft_content: scene.draft_content,
      });
      const stage = scene.execution_stage;
      if (
        stage === 'planning' ||
        stage === 'outline' ||
        stage === 'drafting' ||
        stage === 'review' ||
        stage === 'final'
      ) {
        setActiveTab(stage);
      }
    }
  }, [scene]);

  const showPipelineRail =
    activeTab === 'drafting' || activeTab === 'review' || activeTab === 'final';

  const handlePipelineContentChange = (content: string) => {
    setFormData(prev => ({
      ...prev,
      content,
      draft_content: content,
      execution_stage: prev.execution_stage === 'final' ? 'final' : 'drafting',
    }));
    if (scene?.story_id) {
      queryClient.invalidateQueries({ queryKey: ['scenes', scene.story_id] });
      queryClient.invalidateQueries({ queryKey: ['scenes', 'detail', scene.id] });
    }
  };

  if (!scene) {
    return (
      <div className="flex items-center justify-center h-full text-gray-500">
        选择一个场景进行编辑
      </div>
    );
  }

  const handleSave = () => {
    onSave(formData);
    setRevisionMode(false);
  };

  const currentStage = (formData.execution_stage as ExecutionStage) || 'planning';

  const handleGenerateOutline = async () => {
    if (!scene) return;
    setGeneratingOutline(true);
    try {
      const result = await loggedInvoke<{ content: string }>('generate_scene_outline', {
        scene_id: scene.id,
      });
      setFormData(prev => ({
        ...prev,
        outline_content: result.content,
        execution_stage: 'outline',
      }));
      toast.success('大纲生成成功');
      setActiveTab('outline');
    } catch (e: unknown) {
      toast.error(`生成大纲失败: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setGeneratingOutline(false);
    }
  };

  const handleGenerateDraft = async () => {
    if (!scene) return;
    if (!formData.outline_content) {
      toast.error('请先生成大纲');
      return;
    }
    setGeneratingDraft(true);
    try {
      const result = await loggedInvoke<{ content: string }>('generate_scene_draft', {
        scene_id: scene.id,
      });
      setFormData(prev => ({
        ...prev,
        draft_content: result.content,
        execution_stage: 'drafting',
      }));
      toast.success('草稿生成成功');
      setActiveTab('drafting');
    } catch (e: unknown) {
      toast.error(`生成草稿失败: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setGeneratingDraft(false);
    }
  };

  const handlePromoteToFinal = () => {
    const source = formData.draft_content || formData.outline_content || '';
    if (!source) {
      toast.error('没有可提升为定稿的内容');
      return;
    }
    setFormData(prev => ({
      ...prev,
      content: source,
      execution_stage: 'final',
    }));
    toast.success('已提升为定稿');
    setActiveTab('final');
  };

  // Simple diff computation for revision mode
  const computeDiff = (oldText: string, newText: string) => {
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
    const result: Array<{ type: 'added' | 'removed' | 'unchanged'; content: string }> = [];
    let i = oldLines.length,
      j = newLines.length;
    while (i > 0 || j > 0) {
      if (i > 0 && j > 0 && oldLines[i - 1] === newLines[j - 1]) {
        result.unshift({ type: 'unchanged', content: oldLines[i - 1] });
        i--;
        j--;
      } else if (j > 0 && (i === 0 || lcs[i][j - 1] >= lcs[i - 1][j])) {
        result.unshift({ type: 'added', content: newLines[j - 1] });
        j--;
      } else {
        result.unshift({ type: 'removed', content: oldLines[i - 1] });
        i--;
      }
    }
    return result;
  };

  const contentDiff =
    activeTab === 'final' && revisionMode && scene
      ? computeDiff(scene.content || '', formData.content || '')
      : null;

  const toggleCharacter = (charId: string) => {
    const current = formData.characters_present || [];
    if (current.includes(charId)) {
      setFormData({
        ...formData,
        characters_present: current.filter(id => id !== charId),
      });
    } else {
      setFormData({
        ...formData,
        characters_present: [...current, charId],
      });
    }
  };

  return (
    <div className="h-full flex gap-4" data-testid="scene-editor">
      <div className="flex-1 min-w-0 flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-3">
            <h2 className="text-lg font-semibold text-white">
              编辑场景 #{scene.sequence_number}
              <span className="ml-2 text-xs font-normal text-gray-500">（场景序号，非章节号）</span>
            </h2>
            <span
              className={`
            px-2 py-0.5 rounded-full text-xs font-medium
            ${currentStage === 'planning' ? 'bg-blue-500/20 text-blue-400' : ''}
            ${currentStage === 'outline' ? 'bg-amber-500/20 text-amber-400' : ''}
            ${currentStage === 'drafting' ? 'bg-purple-500/20 text-purple-400' : ''}
            ${currentStage === 'review' ? 'bg-orange-500/20 text-orange-400' : ''}
            ${currentStage === 'final' ? 'bg-green-500/20 text-green-400' : ''}
          `}
            >
              {STAGE_LABELS[currentStage] || '规划'}
            </span>
          </div>
          <div className="flex items-center gap-2">
            {activeTab === 'final' && (
              <>
                <Button
                  variant="ghost"
                  size="sm"
                  disabled={compressScene.isPending || !formData.content?.trim()}
                  onClick={async () => {
                    if (!scene) return;
                    try {
                      const result = await compressScene.mutateAsync({
                        scene_id: scene.id,
                        target_ratio: 0.25,
                      });
                      setCompressionResult(result);
                      setShowCompression(true);
                    } catch (e) {
                      sceneEditorLogger.error('Compress failed', { error: e });
                    }
                  }}
                >
                  {compressScene.isPending ? (
                    <Loader2 className="w-4 h-4 mr-1 animate-spin" />
                  ) : (
                    <Minimize2 className="w-4 h-4 mr-1" />
                  )}
                  记忆压缩
                </Button>
                <Button variant="ghost" size="sm" onClick={() => setRevisionMode(!revisionMode)}>
                  {revisionMode ? (
                    <EyeOff className="w-4 h-4 mr-1" />
                  ) : (
                    <GitCompare className="w-4 h-4 mr-1" />
                  )}
                  {revisionMode ? '退出修订' : '修订模式'}
                </Button>
              </>
            )}
            <Button variant="ghost" size="sm" onClick={onCancel}>
              <X className="w-4 h-4 mr-1" />
              取消
            </Button>
            <Button variant="primary" size="sm" onClick={handleSave}>
              <Save className="w-4 h-4 mr-1" />
              保存
            </Button>
          </div>
        </div>

        {/* Stage Tabs */}
        <div className="flex gap-1 mb-4 p-1 bg-cinema-800 rounded-lg">
          {STAGE_TABS.map(tab => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={`
              flex items-center gap-2 px-3 py-2 rounded-md text-sm font-medium transition-colors
              ${
                activeTab === tab.id
                  ? 'bg-cinema-gold text-cinema-900'
                  : 'text-gray-400 hover:text-white hover:bg-cinema-700'
              }
            `}
            >
              <tab.icon className="w-4 h-4" />
              {tab.label}
            </button>
          ))}
        </div>

        {/* Content */}
        <div className="flex-1 overflow-auto space-y-4">
          {/* Planning Tab */}
          {activeTab === 'planning' && (
            <>
              {/* Title */}
              <div>
                <label className="block text-sm text-gray-400 mb-1">场景标题</label>
                <input
                  type="text"
                  value={formData.title || ''}
                  onChange={e => setFormData({ ...formData, title: e.target.value })}
                  placeholder={`场景 ${scene.sequence_number}`}
                  className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white focus:border-cinema-gold focus:outline-none"
                />
              </div>

              {/* Setting */}
              <Card>
                <CardContent className="p-4 space-y-3">
                  <h3 className="font-medium text-white flex items-center gap-2">
                    <MapPin className="w-4 h-4 text-cinema-gold" />
                    场景设置
                  </h3>

                  <div>
                    <label className="block text-xs text-gray-400 mb-1">地点</label>
                    <input
                      type="text"
                      value={formData.setting_location || ''}
                      onChange={e => setFormData({ ...formData, setting_location: e.target.value })}
                      placeholder="例如：长安城、太空站..."
                      className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white text-sm focus:border-cinema-gold focus:outline-none"
                    />
                  </div>

                  <div>
                    <label className="block text-xs text-gray-400 mb-1">时间</label>
                    <input
                      type="text"
                      value={formData.setting_time || ''}
                      onChange={e => setFormData({ ...formData, setting_time: e.target.value })}
                      placeholder="例如：黄昏、2145年..."
                      className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white text-sm focus:border-cinema-gold focus:outline-none"
                    />
                  </div>

                  <div>
                    <label className="block text-xs text-gray-400 mb-1">氛围</label>
                    <input
                      type="text"
                      value={formData.setting_atmosphere || ''}
                      onChange={e =>
                        setFormData({ ...formData, setting_atmosphere: e.target.value })
                      }
                      placeholder="例如：紧张、神秘、温馨..."
                      className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white text-sm focus:border-cinema-gold focus:outline-none"
                    />
                  </div>
                </CardContent>
              </Card>

              {/* Characters */}
              <Card>
                <CardContent className="p-4">
                  <h3 className="font-medium text-white flex items-center gap-2 mb-3">
                    <Users className="w-4 h-4 text-cinema-gold" />
                    出场角色
                  </h3>

                  <div className="grid grid-cols-2 gap-2">
                    {characters.map(char => (
                      <button
                        key={char.id}
                        onClick={() => toggleCharacter(char.id)}
                        className={`
                        flex items-center gap-2 p-2 rounded-lg text-left text-sm transition-colors
                        ${
                          (formData.characters_present || []).includes(char.id)
                            ? 'bg-cinema-gold/20 border border-cinema-gold/50 text-white'
                            : 'bg-cinema-800 border border-transparent text-gray-300 hover:bg-cinema-700'
                        }
                      `}
                      >
                        <div
                          className={`
                        w-2 h-2 rounded-full
                        ${(formData.characters_present || []).includes(char.id) ? 'bg-cinema-gold' : 'bg-gray-600'}
                      `}
                        />
                        <div>
                          <div className="font-medium">{char.name}</div>
                          {char.personality && (
                            <div className="text-xs text-gray-500 truncate">{char.personality}</div>
                          )}
                        </div>
                      </button>
                    ))}
                  </div>

                  {characters.length === 0 && (
                    <p className="text-sm text-gray-500 text-center py-4">还没有创建角色</p>
                  )}
                </CardContent>
              </Card>

              {/* Dramatic Goal */}
              <Card>
                <CardContent className="p-4">
                  <h3 className="font-medium text-white flex items-center gap-2 mb-3">
                    <Target className="w-4 h-4 text-cinema-gold" />
                    戏剧目标
                  </h3>
                  <p className="text-xs text-gray-500 mb-2">这个场景要完成什么？推动什么情节？</p>
                  <textarea
                    value={formData.dramatic_goal || ''}
                    onChange={e => setFormData({ ...formData, dramatic_goal: e.target.value })}
                    placeholder="例如：主角发现真相，反派暴露野心..."
                    rows={3}
                    className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white text-sm focus:border-cinema-gold focus:outline-none resize-none"
                  />
                </CardContent>
              </Card>

              {/* External Pressure */}
              <Card>
                <CardContent className="p-4">
                  <h3 className="font-medium text-white flex items-center gap-2 mb-3">
                    <Zap className="w-4 h-4 text-cinema-gold" />
                    外部压迫
                  </h3>
                  <p className="text-xs text-gray-500 mb-2">
                    什么力量在给角色施压？（环境、反派、事件等）
                  </p>
                  <textarea
                    value={formData.external_pressure || ''}
                    onChange={e => setFormData({ ...formData, external_pressure: e.target.value })}
                    placeholder="例如：暴雨将至，追兵逼近，时间紧迫..."
                    rows={3}
                    className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white text-sm focus:border-cinema-gold focus:outline-none resize-none"
                  />
                </CardContent>
              </Card>

              {/* Confidence Score */}
              <Card>
                <CardContent className="p-4">
                  <h3 className="font-medium text-white mb-3">AI 生成置信度</h3>
                  <p className="text-xs text-gray-500 mb-3">
                    评估此场景内容的质量置信度（0-1），用于版本管理和记忆保留策略。
                  </p>
                  <div className="flex items-center gap-3">
                    <input
                      type="range"
                      min="0"
                      max="1"
                      step="0.1"
                      value={formData.confidence_score ?? 0.5}
                      onChange={e =>
                        setFormData({ ...formData, confidence_score: Number(e.target.value) })
                      }
                      className="flex-1 accent-cinema-gold"
                    />
                    <span className="text-sm text-cinema-gold font-medium w-12 text-right">
                      {((formData.confidence_score ?? 0.5) * 100).toFixed(0)}%
                    </span>
                  </div>
                </CardContent>
              </Card>

              {/* Conflict Type */}
              <Card>
                <CardContent className="p-4">
                  <h3 className="font-medium text-white mb-3">冲突类型</h3>
                  <div className="grid grid-cols-3 gap-2">
                    {CONFLICT_TYPES.map(type => (
                      <button
                        key={type}
                        onClick={() => setFormData({ ...formData, conflict_type: type })}
                        className={`
                        flex items-center gap-2 p-3 rounded-lg text-left transition-colors
                        ${
                          formData.conflict_type === type
                            ? 'bg-cinema-gold/20 border border-cinema-gold/50'
                            : 'bg-cinema-800 border border-transparent hover:bg-cinema-700'
                        }
                      `}
                      >
                        <div
                          className="w-3 h-3 rounded-full"
                          style={{ backgroundColor: getConflictTypeColor(type) }}
                        />
                        <span
                          className={`
                        text-sm
                        ${formData.conflict_type === type ? 'text-white' : 'text-gray-300'}
                      `}
                        >
                          {getConflictTypeLabel(type)}
                        </span>
                      </button>
                    ))}
                  </div>
                </CardContent>
              </Card>

              {/* Next Stage Action */}
              <div className="flex justify-end">
                <Button
                  variant="primary"
                  onClick={handleGenerateOutline}
                  disabled={generatingOutline}
                >
                  {generatingOutline ? (
                    <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                  ) : (
                    <Sparkles className="w-4 h-4 mr-2" />
                  )}
                  生成大纲
                  <ArrowRight className="w-4 h-4 ml-2" />
                </Button>
              </div>
            </>
          )}

          {/* Outline Tab */}
          {activeTab === 'outline' && (
            <div className="space-y-4">
              <div className="flex items-center justify-between">
                <h3 className="text-sm font-medium text-white flex items-center gap-2">
                  <ClipboardList className="w-4 h-4 text-cinema-gold" />
                  场景大纲
                </h3>
                <Button
                  variant="primary"
                  size="sm"
                  onClick={handleGenerateOutline}
                  disabled={generatingOutline}
                >
                  {generatingOutline ? (
                    <Loader2 className="w-4 h-4 mr-1 animate-spin" />
                  ) : (
                    <Sparkles className="w-4 h-4 mr-1" />
                  )}
                  重新生成
                </Button>
              </div>
              <textarea
                value={formData.outline_content || ''}
                onChange={e => setFormData({ ...formData, outline_content: e.target.value })}
                placeholder="在此输入或生成场景大纲..."
                rows={20}
                className="w-full px-4 py-3 bg-cinema-800 border border-cinema-700 rounded-lg text-white focus:border-cinema-gold focus:outline-none resize-none font-serif leading-relaxed"
              />
              <div className="flex justify-between">
                <Button variant="secondary" size="sm" onClick={() => setActiveTab('planning')}>
                  返回规划
                </Button>
                <Button
                  variant="primary"
                  size="sm"
                  onClick={handleGenerateDraft}
                  disabled={generatingDraft || !formData.outline_content}
                >
                  {generatingDraft ? (
                    <Loader2 className="w-4 h-4 mr-1 animate-spin" />
                  ) : (
                    <PenTool className="w-4 h-4 mr-1" />
                  )}
                  根据大纲起草
                  <ArrowRight className="w-4 h-4 ml-2" />
                </Button>
              </div>
            </div>
          )}

          {/* Drafting Tab */}
          {activeTab === 'drafting' && (
            <div className="space-y-4">
              <div className="flex items-center justify-between">
                <h3 className="text-sm font-medium text-white flex items-center gap-2">
                  <PenTool className="w-4 h-4 text-cinema-gold" />
                  场景草稿
                </h3>
                <Button
                  variant="primary"
                  size="sm"
                  onClick={handleGenerateDraft}
                  disabled={generatingDraft || !formData.outline_content}
                >
                  {generatingDraft ? (
                    <Loader2 className="w-4 h-4 mr-1 animate-spin" />
                  ) : (
                    <Sparkles className="w-4 h-4 mr-1" />
                  )}
                  重新起草
                </Button>
              </div>
              <textarea
                value={formData.draft_content || ''}
                onChange={e => setFormData({ ...formData, draft_content: e.target.value })}
                placeholder="AI 根据大纲生成的草稿将显示在这里..."
                rows={20}
                className="w-full px-4 py-3 bg-cinema-800 border border-cinema-700 rounded-lg text-white focus:border-cinema-gold focus:outline-none resize-none font-serif leading-relaxed"
              />
              <div className="flex justify-between">
                <Button variant="secondary" size="sm" onClick={() => setActiveTab('outline')}>
                  返回大纲
                </Button>
                <Button variant="primary" size="sm" onClick={handlePromoteToFinal}>
                  <Check className="w-4 h-4 mr-1" />
                  提升为定稿
                  <ArrowRight className="w-4 h-4 ml-2" />
                </Button>
              </div>
            </div>
          )}

          {/* Review Tab */}
          {activeTab === 'review' && (
            <SceneAuditPanel
              sceneId={scene.id}
              onPromoteToFinal={handlePromoteToFinal}
              onBackToDrafting={() => setActiveTab('drafting')}
            />
          )}

          {/* Final Tab */}
          {activeTab === 'final' && (
            <div>
              {revisionMode && contentDiff ? (
                <div className="space-y-2">
                  <div className="flex items-center justify-between mb-2">
                    <label className="text-sm text-gray-400">修订对比</label>
                    <div className="flex items-center gap-3 text-xs">
                      <span className="flex items-center gap-1">
                        <span className="w-2 h-2 rounded-full bg-green-500" />
                        新增
                      </span>
                      <span className="flex items-center gap-1">
                        <span className="w-2 h-2 rounded-full bg-red-500" />
                        删除
                      </span>
                      <span className="flex items-center gap-1">
                        <span className="w-2 h-2 rounded-full bg-gray-500" />
                        未变
                      </span>
                    </div>
                  </div>
                  <div className="bg-cinema-800 border border-cinema-700 rounded-lg p-4 space-y-1 max-h-[32rem] overflow-y-auto font-mono text-sm">
                    {contentDiff.map((line, idx) => (
                      <div
                        key={idx}
                        className={`
                        px-2 py-1 rounded
                        ${line.type === 'added' ? 'bg-green-500/10 text-green-300' : ''}
                        ${line.type === 'removed' ? 'bg-red-500/10 text-red-300 line-through' : ''}
                        ${line.type === 'unchanged' ? 'text-gray-400' : ''}
                      `}
                      >
                        {line.type === 'added' && '+ '}
                        {line.type === 'removed' && '- '}
                        {line.type === 'unchanged' && '  '}
                        {line.content || ' '}
                      </div>
                    ))}
                  </div>
                </div>
              ) : (
                <div className="space-y-3">
                  <label className="block text-sm text-gray-400 mb-2">定稿内容</label>
                  <textarea
                    value={formData.content || ''}
                    onChange={e => setFormData({ ...formData, content: e.target.value })}
                    placeholder="最终场景内容..."
                    rows={showCompression ? 12 : 20}
                    className="w-full px-4 py-3 bg-cinema-800 border border-cinema-700 rounded-lg text-white focus:border-cinema-gold focus:outline-none resize-none font-serif leading-relaxed"
                  />
                  {showCompression && compressionResult && (
                    <div className="bg-cinema-900/50 border border-cinema-gold/30 rounded-lg p-4">
                      <div className="flex items-center justify-between mb-2">
                        <div className="flex items-center gap-2 text-sm text-cinema-gold">
                          <Sparkles className="w-4 h-4" />
                          <span>压缩摘要</span>
                          {compressionResult.score !== undefined && (
                            <span className="text-xs text-gray-400">
                              (压缩率: {(compressionResult.score * 100).toFixed(1)}%)
                            </span>
                          )}
                        </div>
                        <div className="flex items-center gap-2">
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() => {
                              setFormData({ ...formData, content: compressionResult.content });
                              setShowCompression(false);
                            }}
                          >
                            <Check className="w-4 h-4 mr-1" />
                            应用
                          </Button>
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() => setShowCompression(false)}
                          >
                            <X className="w-4 h-4 mr-1" />
                            关闭
                          </Button>
                        </div>
                      </div>
                      <div className="text-sm text-gray-300 leading-relaxed whitespace-pre-wrap">
                        {compressionResult.content}
                      </div>
                      {compressionResult.suggestions.length > 0 && (
                        <div className="mt-2 text-xs text-gray-500">
                          {compressionResult.suggestions.join(' · ')}
                        </div>
                      )}
                    </div>
                  )}
                </div>
              )}
              <div className="flex justify-between mt-4">
                <Button variant="secondary" size="sm" onClick={() => setActiveTab('review')}>
                  返回审校
                </Button>
                <Button variant="primary" size="sm" onClick={handleSave}>
                  <Save className="w-4 h-4 mr-1" />
                  保存定稿
                </Button>
              </div>
            </div>
          )}

          {/* Annotations Tab */}
          {activeTab === 'annotations' && (
            <SceneAnnotationPanel sceneId={scene.id} storyId={scene.story_id} />
          )}
        </div>
      </div>

      {showPipelineRail && (
        <div
          className={`flex-shrink-0 flex flex-col border border-cinema-700 rounded-lg overflow-hidden bg-cinema-900/40 transition-all ${
            pipelineOpen ? 'w-72' : 'w-10'
          }`}
          data-testid="scene-editor-pipeline-rail"
        >
          <button
            type="button"
            onClick={() => setPipelineOpen(open => !open)}
            className="flex items-center gap-2 px-3 py-2 border-b border-cinema-700 text-sm text-gray-300 hover:bg-cinema-800/60"
            data-testid="scene-editor-pipeline-toggle"
            aria-expanded={pipelineOpen}
          >
            <GitMerge className="w-4 h-4 text-purple-400 flex-shrink-0" />
            {pipelineOpen ? (
              <>
                <span className="flex-1 text-left font-medium">管线</span>
                <ChevronUp className="w-4 h-4 text-gray-500" />
              </>
            ) : (
              <ChevronDown className="w-4 h-4 text-gray-500" />
            )}
          </button>
          {pipelineOpen && (
            <div className="flex-1 min-h-0 overflow-hidden">
              <PipelinePanel
                storyId={scene.story_id}
                chapterNumber={scene.sequence_number}
                chapterTitle={formData.title || scene.title}
                currentContent={formData.content || formData.draft_content || scene.content}
                onContentChange={handlePipelineContentChange}
              />
            </div>
          )}
        </div>
      )}
    </div>
  );
}
