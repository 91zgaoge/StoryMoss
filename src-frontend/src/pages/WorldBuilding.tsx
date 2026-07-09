import { useState, useEffect, useCallback, useRef } from 'react';
import { useAppStore } from '@/stores/appStore';
import {
  useWorldBuilding,
  useCreateWorldBuilding,
  useUpdateWorldBuilding,
  useGenerateWorldBuildingOptions,
  useWritingStyle,
  useCreateWritingStyle,
  useUpdateWritingStyle,
} from '@/hooks/useWorldBuilding';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import {
  Globe,
  Plus,
  Trash2,
  Edit3,
  Save,
  X,
  Star,
  BookOpen,
  Shield,
  Clock,
  Palette,
  Sparkles,
  Wand2,
  RefreshCw,
  PenLine,
} from 'lucide-react';
import type {
  WorldBuilding,
  WorldRule,
  Culture,
  RuleType,
  WorldBuildingOption,
  WritingStyleUpdate,
} from '@/types';

const RULE_TYPE_LABELS: Record<RuleType, string> = {
  Magic: '魔法',
  Technology: '科技',
  Social: '社会',
  Physical: '物理',
  Biological: '生物',
  Historical: '历史',
  Cultural: '文化',
  Custom: '自定义',
};

const RULE_TYPE_COLORS: Record<RuleType, string> = {
  Magic: 'bg-purple-500/20 text-purple-300 border-purple-500/30',
  Technology: 'bg-cyan-500/20 text-cyan-300 border-cyan-500/30',
  Social: 'bg-blue-500/20 text-blue-300 border-blue-500/30',
  Physical: 'bg-green-500/20 text-green-300 border-green-500/30',
  Biological: 'bg-emerald-500/20 text-emerald-300 border-emerald-500/30',
  Historical: 'bg-amber-500/20 text-amber-300 border-amber-500/30',
  Cultural: 'bg-pink-500/20 text-pink-300 border-pink-500/30',
  Custom: 'bg-gray-500/20 text-gray-300 border-gray-500/30',
};

function RuleTypeBadge({ type }: { type: RuleType }) {
  return (
    <span className={`text-xs px-2 py-0.5 rounded-full border ${RULE_TYPE_COLORS[type]}`}>
      {RULE_TYPE_LABELS[type]}
    </span>
  );
}

function ImportanceStars({ level }: { level: number }) {
  return (
    <div className="flex items-center gap-0.5">
      {Array.from({ length: 10 }).map((_, i) => (
        <Star
          key={i}
          className={`w-3 h-3 ${i < level ? 'text-cinema-gold fill-cinema-gold' : 'text-gray-600'}`}
        />
      ))}
    </div>
  );
}

interface RuleModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSave: (rule: WorldRule) => void;
  initialRule?: WorldRule | null;
}

function RuleModal({ isOpen, onClose, onSave, initialRule }: RuleModalProps) {
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [ruleType, setRuleType] = useState<RuleType>('Physical');
  const [importance, setImportance] = useState(5);

  useEffect(() => {
    if (initialRule) {
      setName(initialRule.name);
      setDescription(initialRule.description || '');
      setRuleType(initialRule.rule_type);
      setImportance(initialRule.importance);
    } else {
      setName('');
      setDescription('');
      setRuleType('Physical');
      setImportance(5);
    }
  }, [initialRule, isOpen]);

  if (!isOpen) return null;

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;
    onSave({
      id: initialRule?.id || crypto.randomUUID(),
      name: name.trim(),
      description: description.trim() || undefined,
      rule_type: ruleType,
      importance,
    });
    onClose();
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <Card className="w-full max-w-lg mx-4">
        <CardContent className="p-6">
          <h2 className="font-display text-xl font-bold text-white mb-4">
            {initialRule ? '编辑规则' : '添加世界规则'}
          </h2>
          <form onSubmit={handleSubmit} className="space-y-4">
            <div>
              <label className="block text-sm text-gray-400 mb-1">规则名称 *</label>
              <input
                value={name}
                onChange={e => setName(e.target.value)}
                required
                className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none"
                placeholder="例如：重力异常"
              />
            </div>
            <div>
              <label className="block text-sm text-gray-400 mb-1">规则类型</label>
              <select
                value={ruleType}
                onChange={e => setRuleType(e.target.value as RuleType)}
                className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none"
              >
                {Object.entries(RULE_TYPE_LABELS).map(([key, label]) => (
                  <option key={key} value={key}>
                    {label}
                  </option>
                ))}
              </select>
            </div>
            <div>
              <label className="block text-sm text-gray-400 mb-1">重要性: {importance}</label>
              <input
                type="range"
                min={1}
                max={10}
                value={importance}
                onChange={e => setImportance(Number(e.target.value))}
                className="w-full accent-cinema-gold"
              />
            </div>
            <div>
              <label className="block text-sm text-gray-400 mb-1">描述</label>
              <textarea
                value={description}
                onChange={e => setDescription(e.target.value)}
                rows={3}
                className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none resize-none"
                placeholder="该规则的具体描述..."
              />
            </div>
            <div className="flex gap-3 pt-4">
              <Button type="button" variant="ghost" onClick={onClose}>
                取消
              </Button>
              <Button type="submit" variant="primary">
                <Save className="w-4 h-4" />
                保存
              </Button>
            </div>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}

interface CultureModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSave: (culture: Culture) => void;
  initialCulture?: Culture | null;
}

function CultureModal({ isOpen, onClose, onSave, initialCulture }: CultureModalProps) {
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [customs, setCustoms] = useState('');
  const [values, setValues] = useState('');

  useEffect(() => {
    if (initialCulture) {
      setName(initialCulture.name);
      setDescription(initialCulture.description);
      setCustoms(initialCulture.customs.join('\n'));
      setValues(initialCulture.values.join('\n'));
    } else {
      setName('');
      setDescription('');
      setCustoms('');
      setValues('');
    }
  }, [initialCulture, isOpen]);

  if (!isOpen) return null;

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;
    onSave({
      name: name.trim(),
      description: description.trim(),
      customs: customs
        .split('\n')
        .map(s => s.trim())
        .filter(Boolean),
      values: values
        .split('\n')
        .map(s => s.trim())
        .filter(Boolean),
    });
    onClose();
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <Card className="w-full max-w-lg mx-4">
        <CardContent className="p-6">
          <h2 className="font-display text-xl font-bold text-white mb-4">
            {initialCulture ? '编辑文化' : '添加文化体系'}
          </h2>
          <form onSubmit={handleSubmit} className="space-y-4">
            <div>
              <label className="block text-sm text-gray-400 mb-1">文化名称 *</label>
              <input
                value={name}
                onChange={e => setName(e.target.value)}
                required
                className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none"
                placeholder="例如：东方修真文化"
              />
            </div>
            <div>
              <label className="block text-sm text-gray-400 mb-1">描述</label>
              <textarea
                value={description}
                onChange={e => setDescription(e.target.value)}
                rows={2}
                className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none resize-none"
                placeholder="该文化的总体描述..."
              />
            </div>
            <div>
              <label className="block text-sm text-gray-400 mb-1">习俗（每行一个）</label>
              <textarea
                value={customs}
                onChange={e => setCustoms(e.target.value)}
                rows={3}
                className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none resize-none"
                placeholder="例如：&#10;晨间冥想&#10;拜师礼"
              />
            </div>
            <div>
              <label className="block text-sm text-gray-400 mb-1">价值观（每行一个）</label>
              <textarea
                value={values}
                onChange={e => setValues(e.target.value)}
                rows={3}
                className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none resize-none"
                placeholder="例如：&#10;尊师重道&#10;天人合一"
              />
            </div>
            <div className="flex gap-3 pt-4">
              <Button type="button" variant="ghost" onClick={onClose}>
                取消
              </Button>
              <Button type="submit" variant="primary">
                <Save className="w-4 h-4" />
                保存
              </Button>
            </div>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}

interface AiWorldBuildingModalProps {
  isOpen: boolean;
  onClose: () => void;
  storyTitle: string;
  onApply: (option: WorldBuildingOption) => void;
}

function AiWorldBuildingModal({ isOpen, onClose, storyTitle, onApply }: AiWorldBuildingModalProps) {
  const [prompt, setPrompt] = useState('');
  const [options, setOptions] = useState<WorldBuildingOption[]>([]);
  const [selectedIndex, setSelectedIndex] = useState<number | null>(null);
  const generateOptions = useGenerateWorldBuildingOptions();

  useEffect(() => {
    if (isOpen) {
      setPrompt(storyTitle);
      setOptions([]);
      setSelectedIndex(null);
    }
  }, [isOpen, storyTitle]);

  if (!isOpen) return null;

  const handleGenerate = async () => {
    if (!prompt.trim()) return;
    const result = await generateOptions.mutateAsync(prompt.trim());
    setOptions(result);
    setSelectedIndex(result.length > 0 ? 0 : null);
  };

  const handleApply = () => {
    if (selectedIndex === null || !options[selectedIndex]) return;
    onApply(options[selectedIndex]);
    onClose();
  };

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-4">
      <Card className="w-full max-w-3xl max-h-[90vh] overflow-y-auto">
        <CardContent className="p-6 space-y-5">
          <div className="flex items-center justify-between">
            <h2 className="font-display text-xl font-bold text-white flex items-center gap-2">
              <Wand2 className="w-5 h-5 text-cinema-gold" />
              AI 生成世界观
            </h2>
            <button
              onClick={onClose}
              className="p-1.5 rounded-lg hover:bg-cinema-700 text-gray-400 hover:text-white transition-colors"
            >
              <X className="w-5 h-5" />
            </button>
          </div>

          <div className="space-y-2">
            <label className="block text-sm text-gray-400">描述你的故事题材与核心设定</label>
            <textarea
              value={prompt}
              onChange={e => setPrompt(e.target.value)}
              rows={3}
              className="w-full px-4 py-3 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none resize-none"
              placeholder="例如：末世废土、东方修仙、赛博朋克侦探..."
            />
            <Button
              variant="primary"
              onClick={handleGenerate}
              isLoading={generateOptions.isPending}
              disabled={!prompt.trim()}
            >
              {generateOptions.isPending ? (
                <RefreshCw className="w-4 h-4 animate-spin" />
              ) : (
                <Sparkles className="w-4 h-4" />
              )}
              生成选项
            </Button>
          </div>

          {options.length > 0 && (
            <div className="space-y-3">
              <p className="text-sm text-gray-400">选择一套世界观应用到当前故事：</p>
              <div className="grid gap-3">
                {options.map((option, index) => (
                  <div
                    key={option.id}
                    onClick={() => setSelectedIndex(index)}
                    className={`p-4 rounded-xl border cursor-pointer transition-all ${
                      selectedIndex === index
                        ? 'border-cinema-gold bg-cinema-gold/10'
                        : 'border-cinema-700 bg-cinema-800/50 hover:border-cinema-gold/40'
                    }`}
                  >
                    <h3 className="font-medium text-white mb-1">{option.concept}</h3>
                    <p className="text-xs text-gray-400 mb-2">
                      {option.rules.length} 条规则 · {option.cultures.length} 种文化
                    </p>
                    {option.history && (
                      <p className="text-sm text-gray-500 line-clamp-2">{option.history}</p>
                    )}
                  </div>
                ))}
              </div>
              <Button
                variant="primary"
                onClick={handleApply}
                disabled={selectedIndex === null}
                className="w-full"
              >
                应用选中的世界观
              </Button>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}

type WorldBuildingTab = 'world' | 'style';

export function WorldBuilding() {
  const currentStory = useAppStore(s => s.currentStory);
  const { data: worldBuilding, isLoading } = useWorldBuilding(currentStory?.id || null);
  const { data: writingStyle, isLoading: isWritingStyleLoading } = useWritingStyle(
    currentStory?.id || null
  );
  const createWorldBuilding = useCreateWorldBuilding();
  const updateWorldBuilding = useUpdateWorldBuilding();
  const createWritingStyle = useCreateWritingStyle();
  const updateWritingStyle = useUpdateWritingStyle();

  const [activeTab, setActiveTab] = useState<WorldBuildingTab>('world');

  // Local edit state with debounced auto-save
  const [localConcept, setLocalConcept] = useState('');
  const [localHistory, setLocalHistory] = useState('');
  const [localRules, setLocalRules] = useState<WorldRule[]>([]);
  const [localCultures, setLocalCultures] = useState<Culture[]>([]);
  const [hasLocalChanges, setHasLocalChanges] = useState(false);

  // Writing style local state
  const [localStyleName, setLocalStyleName] = useState('');
  const [localStyleDescription, setLocalStyleDescription] = useState('');
  const [localStyleTone, setLocalStyleTone] = useState('');
  const [localStylePacing, setLocalStylePacing] = useState('');
  const [localStyleVocabulary, setLocalStyleVocabulary] = useState('');
  const [localStyleSentenceStructure, setLocalStyleSentenceStructure] = useState('');
  const [hasStyleLocalChanges, setHasStyleLocalChanges] = useState(false);

  // Modal states
  const [ruleModalOpen, setRuleModalOpen] = useState(false);
  const [editingRule, setEditingRule] = useState<WorldRule | null>(null);
  const [cultureModalOpen, setCultureModalOpen] = useState(false);
  const [editingCulture, setEditingCulture] = useState<Culture | null>(null);
  const [aiModalOpen, setAiModalOpen] = useState(false);

  // Refs for debounce
  const saveTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const styleSaveTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Sync local state when data loads
  useEffect(() => {
    if (worldBuilding) {
      setLocalConcept(worldBuilding.concept);
      setLocalHistory(worldBuilding.history || '');
      setLocalRules(worldBuilding.rules);
      setLocalCultures(worldBuilding.cultures);
      setHasLocalChanges(false);
    }
  }, [worldBuilding?.id]);

  useEffect(() => {
    if (writingStyle) {
      setLocalStyleName(writingStyle.name || '');
      setLocalStyleDescription(writingStyle.description || '');
      setLocalStyleTone(writingStyle.tone || '');
      setLocalStylePacing(writingStyle.pacing || '');
      setLocalStyleVocabulary(writingStyle.vocabulary_level || '');
      setLocalStyleSentenceStructure(writingStyle.sentence_structure || '');
      setHasStyleLocalChanges(false);
    }
  }, [writingStyle?.id]);

  // Debounced auto-save
  const triggerSave = useCallback(() => {
    if (!worldBuilding || !currentStory) return;
    setHasLocalChanges(true);

    if (saveTimeoutRef.current) clearTimeout(saveTimeoutRef.current);
    saveTimeoutRef.current = setTimeout(() => {
      updateWorldBuilding.mutate(
        {
          id: worldBuilding.id,
          storyId: currentStory.id,
          concept: localConcept,
          history: localHistory,
          rules: localRules,
          cultures: localCultures,
        },
        {
          onSuccess: () => setHasLocalChanges(false),
        }
      );
    }, 800);
  }, [
    worldBuilding,
    currentStory,
    localConcept,
    localHistory,
    localRules,
    localCultures,
    updateWorldBuilding,
  ]);

  const triggerStyleSave = useCallback(
    (updates: WritingStyleUpdate) => {
      if (!writingStyle || !currentStory) return;
      setHasStyleLocalChanges(true);

      if (styleSaveTimeoutRef.current) clearTimeout(styleSaveTimeoutRef.current);
      styleSaveTimeoutRef.current = setTimeout(() => {
        updateWritingStyle.mutate(
          {
            id: writingStyle.id,
            storyId: currentStory.id,
            updates,
          },
          {
            onSuccess: () => setHasStyleLocalChanges(false),
          }
        );
      }, 800);
    },
    [writingStyle, currentStory, updateWritingStyle]
  );

  useEffect(() => {
    return () => {
      if (saveTimeoutRef.current) clearTimeout(saveTimeoutRef.current);
      if (styleSaveTimeoutRef.current) clearTimeout(styleSaveTimeoutRef.current);
    };
  }, []);

  const handleConceptChange = (value: string) => {
    setLocalConcept(value);
    triggerSave();
  };

  const handleHistoryChange = (value: string) => {
    setLocalHistory(value);
    triggerSave();
  };

  const handleAddRule = (rule: WorldRule) => {
    const next = editingRule
      ? localRules.map(r => (r.id === rule.id ? rule : r))
      : [...localRules, rule];
    setLocalRules(next);
    setEditingRule(null);
    triggerSave();
  };

  const handleDeleteRule = (id: string) => {
    if (!confirm('确定删除这条世界规则吗？')) return;
    setLocalRules(prev => prev.filter(r => r.id !== id));
    triggerSave();
  };

  const handleAddCulture = (culture: Culture) => {
    const next = editingCulture
      ? localCultures.map(c => (c.name === culture.name ? culture : c))
      : [...localCultures, culture];
    setLocalCultures(next);
    setEditingCulture(null);
    triggerSave();
  };

  const handleDeleteCulture = (name: string) => {
    if (!confirm('确定删除这个文化体系吗？')) return;
    setLocalCultures(prev => prev.filter(c => c.name !== name));
    triggerSave();
  };

  const handleInitWorldBuilding = () => {
    if (!currentStory) return;
    createWorldBuilding.mutate(
      { storyId: currentStory.id, concept: `${currentStory.title} 的世界观` },
      {
        onSuccess: () => {
          // The query will auto-refetch
        },
      }
    );
  };

  const handleInitWritingStyle = () => {
    if (!currentStory) return;
    createWritingStyle.mutate(currentStory.id);
  };

  const handleStyleFieldChange = (field: keyof WritingStyleUpdate, value: string) => {
    switch (field) {
      case 'name':
        setLocalStyleName(value);
        break;
      case 'description':
        setLocalStyleDescription(value);
        break;
      case 'tone':
        setLocalStyleTone(value);
        break;
      case 'pacing':
        setLocalStylePacing(value);
        break;
      case 'vocabulary_level':
        setLocalStyleVocabulary(value);
        break;
      case 'sentence_structure':
        setLocalStyleSentenceStructure(value);
        break;
    }
    triggerStyleSave({ [field]: value });
  };

  const handleApplyAiOption = (option: WorldBuildingOption) => {
    if (!worldBuilding || !currentStory) return;
    setLocalConcept(option.concept);
    setLocalHistory(option.history || '');
    setLocalRules(option.rules);
    setLocalCultures(option.cultures);
    updateWorldBuilding.mutate(
      {
        id: worldBuilding.id,
        storyId: currentStory.id,
        concept: option.concept,
        history: option.history,
        rules: option.rules,
        cultures: option.cultures,
      },
      {
        onSuccess: () => setHasLocalChanges(false),
      }
    );
  };

  if (!currentStory) {
    return (
      <div className="p-8 flex items-center justify-center h-full">
        <Card>
          <CardContent className="p-8 text-center">
            <Globe className="w-12 h-12 text-gray-600 mx-auto mb-4" />
            <h2 className="font-display text-xl font-semibold text-white mb-2">先选择一个故事</h2>
            <p className="text-gray-400">在故事库中选择一个故事来构建世界</p>
          </CardContent>
        </Card>
      </div>
    );
  }

  if (isLoading) {
    return (
      <div className="p-8 flex items-center justify-center h-full">
        <div className="animate-spin w-8 h-8 border-2 border-cinema-gold border-t-transparent rounded-full" />
      </div>
    );
  }

  if (!worldBuilding) {
    return (
      <div className="p-8 flex items-center justify-center h-full">
        <Card>
          <CardContent className="p-8 text-center max-w-md">
            <Globe className="w-16 h-16 text-cinema-gold/60 mx-auto mb-4" />
            <h2 className="font-display text-2xl font-bold text-white mb-2">世界尚未构建</h2>
            <p className="text-gray-400 mb-6">
              为「{currentStory.title}」初始化世界构建数据，开始设定小说的世界观、规则和文化。
            </p>
            <Button
              variant="primary"
              onClick={handleInitWorldBuilding}
              isLoading={createWorldBuilding.isPending}
            >
              <Sparkles className="w-4 h-4" />
              初始化世界构建
            </Button>
          </CardContent>
        </Card>
      </div>
    );
  }

  return (
    <div className="p-8 space-y-6 animate-fade-in max-w-5xl mx-auto">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="font-display text-3xl font-bold text-white flex items-center gap-3">
            <Globe className="w-8 h-8 text-cinema-gold" />
            世界构建
            {worldBuilding?.is_auto_generated && (
              <span className="text-sm px-2 py-0.5 rounded bg-cinema-gold/20 text-cinema-gold flex items-center gap-1">
                <Star className="w-3.5 h-3.5" />
                创世
              </span>
            )}
          </h1>
          <p className="text-gray-400 mt-1">
            {currentStory.title} · {localRules.length} 条规则 · {localCultures.length} 种文化
            {hasLocalChanges && <span className="text-cinema-gold ml-2 text-sm">保存中...</span>}
          </p>
        </div>
        {activeTab === 'world' && (
          <Button variant="secondary" onClick={() => setAiModalOpen(true)}>
            <Wand2 className="w-4 h-4" />
            AI 生成
          </Button>
        )}
      </div>

      {/* Tabs */}
      <div className="flex gap-1 p-1 bg-cinema-800 rounded-lg w-fit">
        <button
          onClick={() => setActiveTab('world')}
          className={`px-4 py-1.5 rounded-md text-sm transition-colors flex items-center gap-2 ${
            activeTab === 'world' ? 'bg-cinema-700 text-white' : 'text-gray-400 hover:text-gray-200'
          }`}
        >
          <Globe className="w-4 h-4" />
          世界观
        </button>
        <button
          onClick={() => setActiveTab('style')}
          className={`px-4 py-1.5 rounded-md text-sm transition-colors flex items-center gap-2 ${
            activeTab === 'style' ? 'bg-cinema-700 text-white' : 'text-gray-400 hover:text-gray-200'
          }`}
        >
          <PenLine className="w-4 h-4" />
          文风
        </button>
      </div>

      {activeTab === 'world' ? (
        <>
          {/* Core Concept */}
          <Card>
            <CardContent className="p-6">
              <div className="flex items-center gap-2 mb-4">
                <BookOpen className="w-5 h-5 text-cinema-gold" />
                <h2 className="font-display text-lg font-semibold text-white">核心概念</h2>
              </div>
              <textarea
                value={localConcept}
                onChange={e => handleConceptChange(e.target.value)}
                rows={4}
                className="w-full px-4 py-3 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none resize-none"
                placeholder="描述这个世界的核心概念、基本法则和独特之处..."
              />
            </CardContent>
          </Card>

          {/* World Rules */}
          <Card>
            <CardContent className="p-6">
              <div className="flex items-center justify-between mb-4">
                <div className="flex items-center gap-2">
                  <Shield className="w-5 h-5 text-cinema-gold" />
                  <h2 className="font-display text-lg font-semibold text-white">世界规则</h2>
                  <span className="text-xs text-gray-500">({localRules.length})</span>
                </div>
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={() => {
                    setEditingRule(null);
                    setRuleModalOpen(true);
                  }}
                >
                  <Plus className="w-4 h-4" />
                  添加规则
                </Button>
              </div>

              {localRules.length === 0 ? (
                <div className="text-center py-8">
                  <Shield className="w-12 h-12 text-gray-700 mx-auto mb-2" />
                  <p className="text-gray-500 text-sm">还没有世界规则，添加一条吧</p>
                </div>
              ) : (
                <div className="space-y-3">
                  {localRules.map(rule => (
                    <div
                      key={rule.id}
                      className="p-4 bg-cinema-800/50 rounded-xl border border-cinema-700/50 hover:border-cinema-gold/20 transition-colors group"
                    >
                      <div className="flex items-start justify-between">
                        <div className="flex-1 min-w-0">
                          <div className="flex items-center gap-2 flex-wrap">
                            <h3 className="font-medium text-white">{rule.name}</h3>
                            <RuleTypeBadge type={rule.rule_type} />
                          </div>
                          {rule.description && (
                            <p className="text-sm text-gray-400 mt-1 line-clamp-2">
                              {rule.description}
                            </p>
                          )}
                          <div className="mt-2">
                            <ImportanceStars level={rule.importance} />
                          </div>
                        </div>
                        <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity ml-2">
                          <button
                            onClick={() => {
                              setEditingRule(rule);
                              setRuleModalOpen(true);
                            }}
                            className="p-1.5 rounded-lg hover:bg-cinema-700 text-gray-400 hover:text-white transition-colors"
                          >
                            <Edit3 className="w-4 h-4" />
                          </button>
                          <button
                            onClick={() => handleDeleteRule(rule.id)}
                            className="p-1.5 rounded-lg hover:bg-red-500/20 text-gray-400 hover:text-red-400 transition-colors"
                          >
                            <Trash2 className="w-4 h-4" />
                          </button>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </CardContent>
          </Card>

          {/* History */}
          <Card>
            <CardContent className="p-6">
              <div className="flex items-center gap-2 mb-4">
                <Clock className="w-5 h-5 text-cinema-gold" />
                <h2 className="font-display text-lg font-semibold text-white">历史背景</h2>
              </div>
              <textarea
                value={localHistory}
                onChange={e => handleHistoryChange(e.target.value)}
                rows={6}
                className="w-full px-4 py-3 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none resize-none"
                placeholder="记录这个世界的历史脉络、重大事件和时间线..."
              />
            </CardContent>
          </Card>

          {/* Cultures */}
          <Card>
            <CardContent className="p-6">
              <div className="flex items-center justify-between mb-4">
                <div className="flex items-center gap-2">
                  <Palette className="w-5 h-5 text-cinema-gold" />
                  <h2 className="font-display text-lg font-semibold text-white">文化体系</h2>
                  <span className="text-xs text-gray-500">({localCultures.length})</span>
                </div>
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={() => {
                    setEditingCulture(null);
                    setCultureModalOpen(true);
                  }}
                >
                  <Plus className="w-4 h-4" />
                  添加文化
                </Button>
              </div>

              {localCultures.length === 0 ? (
                <div className="text-center py-8">
                  <Palette className="w-12 h-12 text-gray-700 mx-auto mb-2" />
                  <p className="text-gray-500 text-sm">还没有文化体系，添加一个吧</p>
                </div>
              ) : (
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  {localCultures.map(culture => (
                    <div
                      key={culture.name}
                      className="p-4 bg-cinema-800/50 rounded-xl border border-cinema-700/50 hover:border-cinema-gold/20 transition-colors group"
                    >
                      <div className="flex items-start justify-between">
                        <h3 className="font-medium text-white">{culture.name}</h3>
                        <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                          <button
                            onClick={() => {
                              setEditingCulture(culture);
                              setCultureModalOpen(true);
                            }}
                            className="p-1.5 rounded-lg hover:bg-cinema-700 text-gray-400 hover:text-white transition-colors"
                          >
                            <Edit3 className="w-4 h-4" />
                          </button>
                          <button
                            onClick={() => handleDeleteCulture(culture.name)}
                            className="p-1.5 rounded-lg hover:bg-red-500/20 text-gray-400 hover:text-red-400 transition-colors"
                          >
                            <Trash2 className="w-4 h-4" />
                          </button>
                        </div>
                      </div>
                      {culture.description && (
                        <p className="text-sm text-gray-400 mt-1 line-clamp-2">
                          {culture.description}
                        </p>
                      )}
                      {culture.customs.length > 0 && (
                        <div className="mt-3">
                          <p className="text-xs text-gray-500 mb-1">习俗</p>
                          <div className="flex flex-wrap gap-1">
                            {culture.customs.map(c => (
                              <span
                                key={c}
                                className="text-xs px-2 py-0.5 rounded-full bg-cinema-700 text-gray-300"
                              >
                                {c}
                              </span>
                            ))}
                          </div>
                        </div>
                      )}
                      {culture.values.length > 0 && (
                        <div className="mt-2">
                          <p className="text-xs text-gray-500 mb-1">价值观</p>
                          <div className="flex flex-wrap gap-1">
                            {culture.values.map(v => (
                              <span
                                key={v}
                                className="text-xs px-2 py-0.5 rounded-full bg-cinema-900/80 text-gray-400 border border-cinema-700"
                              >
                                {v}
                              </span>
                            ))}
                          </div>
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              )}
            </CardContent>
          </Card>
        </>
      ) : isWritingStyleLoading ? (
        <div className="flex items-center justify-center py-20">
          <div className="animate-spin w-8 h-8 border-2 border-cinema-gold border-t-transparent rounded-full" />
        </div>
      ) : !writingStyle ? (
        <Card>
          <CardContent className="p-8 text-center max-w-md mx-auto">
            <PenLine className="w-16 h-16 text-cinema-gold/60 mx-auto mb-4" />
            <h2 className="font-display text-2xl font-bold text-white mb-2">文风尚未设定</h2>
            <p className="text-gray-400 mb-6">
              为「{currentStory.title}」初始化文风设定，定义小说的语气、节奏与句式风格。
            </p>
            <Button
              variant="primary"
              onClick={handleInitWritingStyle}
              isLoading={createWritingStyle.isPending}
            >
              <Sparkles className="w-4 h-4" />
              初始化文风
            </Button>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <CardContent className="p-6 space-y-5">
            <div className="flex items-center gap-2 mb-2">
              <PenLine className="w-5 h-5 text-cinema-gold" />
              <h2 className="font-display text-lg font-semibold text-white">文风设定</h2>
              {hasStyleLocalChanges && <span className="text-cinema-gold text-sm">保存中...</span>}
            </div>

            <div>
              <label className="block text-sm text-gray-400 mb-1">名称</label>
              <input
                value={localStyleName}
                onChange={e => handleStyleFieldChange('name', e.target.value)}
                className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none"
                placeholder="例如：冷峻写实"
              />
            </div>

            <div>
              <label className="block text-sm text-gray-400 mb-1">描述</label>
              <textarea
                value={localStyleDescription}
                onChange={e => handleStyleFieldChange('description', e.target.value)}
                rows={3}
                className="w-full px-4 py-3 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none resize-none"
                placeholder="整体文风概述..."
              />
            </div>

            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              <div>
                <label className="block text-sm text-gray-400 mb-1">语气 (tone)</label>
                <input
                  value={localStyleTone}
                  onChange={e => handleStyleFieldChange('tone', e.target.value)}
                  className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none"
                  placeholder="例如：沉稳、幽默、悬疑"
                />
              </div>
              <div>
                <label className="block text-sm text-gray-400 mb-1">节奏 (pacing)</label>
                <input
                  value={localStylePacing}
                  onChange={e => handleStyleFieldChange('pacing', e.target.value)}
                  className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none"
                  placeholder="例如：快节奏、舒缓"
                />
              </div>
              <div>
                <label className="block text-sm text-gray-400 mb-1">
                  词汇层级 (vocabulary_level)
                </label>
                <input
                  value={localStyleVocabulary}
                  onChange={e => handleStyleFieldChange('vocabulary_level', e.target.value)}
                  className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none"
                  placeholder="例如：通俗、文学、专业"
                />
              </div>
              <div>
                <label className="block text-sm text-gray-400 mb-1">
                  句式结构 (sentence_structure)
                </label>
                <input
                  value={localStyleSentenceStructure}
                  onChange={e => handleStyleFieldChange('sentence_structure', e.target.value)}
                  className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none"
                  placeholder="例如：短句为主、长句铺陈"
                />
              </div>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Modals */}
      <RuleModal
        isOpen={ruleModalOpen}
        onClose={() => {
          setRuleModalOpen(false);
          setEditingRule(null);
        }}
        onSave={handleAddRule}
        initialRule={editingRule}
      />
      <CultureModal
        isOpen={cultureModalOpen}
        onClose={() => {
          setCultureModalOpen(false);
          setEditingCulture(null);
        }}
        onSave={handleAddCulture}
        initialCulture={editingCulture}
      />
      <AiWorldBuildingModal
        isOpen={aiModalOpen}
        onClose={() => setAiModalOpen(false)}
        storyTitle={currentStory.title}
        onApply={handleApplyAiOption}
      />
    </div>
  );
}
