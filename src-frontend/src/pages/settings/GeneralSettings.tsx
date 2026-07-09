import { useState, useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import {
  Settings2,
  BookOpen,
  Zap,
  Bot,
  Sparkles,
  RefreshCw,
  Download,
  PenTool,
  SlidersHorizontal,
  Clock,
  FileText,
} from 'lucide-react';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { useUpdater } from '@/hooks/useUpdater';
import { useSettingsContext } from '@/hooks/useSettingsContext';
import { useDebounceCallback } from '@/hooks/useDebounceCallback';
import { EditorSettings } from '@/components/EditorSettings';
import {
  colorThemeList,
  applyColorTheme,
  loadColorTheme,
  saveColorTheme,
  COLOR_THEME_STORAGE_KEY,
  type ColorThemeId,
} from '@/frontstage/config/colorThemes';
import { cn } from '@/utils/cn';
import { normalizeFloat, formatDisplayFloat } from '@/utils/numberFormat';
import type { WritingStrategy } from '@/types/llm';
import { loggedInvoke } from '@/services/api/core';

const DEFAULT_WRITING_STRATEGY: WritingStrategy = {
  run_mode: 'fast',
  conflict_level: 50,
  pace: 'balanced',
  ai_freedom: 'medium',
};

// 颜色主题选择器组件
function ColorThemeSelector() {
  const [currentTheme, setCurrentTheme] = useState<ColorThemeId>(() => loadColorTheme());

  const handleSelect = (themeId: ColorThemeId) => {
    setCurrentTheme(themeId);
    applyColorTheme(themeId);
    saveColorTheme(themeId);
  };

  // 跨窗口同步：幕前 ColorThemeDot 变更时更新幕后选择器
  useEffect(() => {
    const handleThemeChange = (themeId: ColorThemeId) => {
      setCurrentTheme(themeId);
      applyColorTheme(themeId);
    };

    const handleStorageChange = (e: StorageEvent) => {
      if (e.key === COLOR_THEME_STORAGE_KEY || e.key === null) {
        handleThemeChange(loadColorTheme());
      }
    };
    window.addEventListener('storage', handleStorageChange);

    let unlisten: (() => void) | undefined;
    void listen<ColorThemeId>('color-theme-changed', event => {
      handleThemeChange(event.payload);
    })
      .then(fn => {
        unlisten = fn;
      })
      .catch(() => {
        /* non-Tauri / test env */
      });

    return () => {
      window.removeEventListener('storage', handleStorageChange);
      unlisten?.();
    };
  }, []);

  return (
    <div className="space-y-3">
      <label className="block text-sm text-gray-400">颜色主题</label>
      <div className="flex flex-wrap gap-3">
        {colorThemeList.map(theme => (
          <button
            key={theme.id}
            onClick={() => handleSelect(theme.id)}
            className={cn(
              'flex items-center gap-2 px-4 py-2.5 rounded-xl border-2 transition-all',
              currentTheme === theme.id
                ? 'border-cinema-gold bg-cinema-gold/10'
                : 'border-cinema-700 bg-cinema-800/50 hover:border-cinema-600'
            )}
            title={theme.description}
          >
            <div
              className="w-5 h-5 rounded-full border border-white/10"
              style={{ backgroundColor: theme.terracotta }}
            />
            <span className="text-sm text-white">{theme.name}</span>
          </button>
        ))}
      </div>
      <p className="text-xs text-gray-500">选择后即时生效，同步影响幕前写作界面</p>
    </div>
  );
}

// v0.26.39: 可按 sections 选择性渲染，供设置页重组复用
export type GeneralSection = 'about' | 'book' | 'agent' | 'writing' | 'prompts' | 'appearance';

export function GeneralSettings({
  sections,
}: {
  /** 不传则渲染全部（兼容旧入口）；传入则只渲染指定卡片 */
  sections?: GeneralSection[];
} = {}) {
  const show = (s: GeneralSection) => !sections || sections.includes(s);
  const {
    currentVersion,
    hasUpdate,
    latestVersion,
    isChecking,
    isInstalling,
    downloadProgress,
    error,
    checkUpdate,
    installUpdate,
  } = useUpdater(false);

  const { settings, updateSettings, isPending } = useSettingsContext();

  const concurrency = settings?.book_deconstruction_concurrency ?? 3;
  const rewriteThreshold = settings?.rewrite_threshold ?? 0.75;
  const maxFeedbackLoops = settings?.max_feedback_loops ?? 2;
  const writingStrategy = settings?.writing_strategy ?? DEFAULT_WRITING_STRATEGY;

  // 滑块、下拉选择、文本域仍使用防抖保存
  const debouncedUpdateSettings = useDebounceCallback(updateSettings, 800);

  // 超时设置本地状态（避免输入过程中频繁保存）
  const [timeoutValues, setTimeoutValues] = useState({
    llm_connect_timeout_secs: settings?.llm_connect_timeout_secs ?? 60,
    llm_first_chunk_timeout_secs: settings?.llm_first_chunk_timeout_secs ?? 180,
    executor_step_timeout_secs: settings?.executor_step_timeout_secs ?? 300,
    smart_execute_total_timeout_secs: settings?.smart_execute_total_timeout_secs ?? 600,
    frontend_timeout_secs: settings?.frontend_timeout_secs ?? 600,
  });

  // 当 settings 从服务端更新时，同步本地状态
  useEffect(() => {
    if (settings) {
      setTimeoutValues({
        llm_connect_timeout_secs: settings.llm_connect_timeout_secs ?? 60,
        llm_first_chunk_timeout_secs: settings.llm_first_chunk_timeout_secs ?? 180,
        executor_step_timeout_secs: settings.executor_step_timeout_secs ?? 300,
        smart_execute_total_timeout_secs: settings.smart_execute_total_timeout_secs ?? 600,
        frontend_timeout_secs: settings.frontend_timeout_secs ?? 600,
      });
    }
  }, [
    settings?.llm_connect_timeout_secs,
    settings?.llm_first_chunk_timeout_secs,
    settings?.executor_step_timeout_secs,
    settings?.smart_execute_total_timeout_secs,
    settings?.frontend_timeout_secs,
  ]);

  const handleTimeoutChange = (field: keyof typeof timeoutValues, value: number) => {
    setTimeoutValues(prev => ({ ...prev, [field]: value }));
  };

  const handleTimeoutBlur = (field: keyof typeof timeoutValues) => {
    const value = timeoutValues[field];
    const originalValue = settings?.[field];
    // 只有值真正改变时才保存
    if (value !== originalValue) {
      updateSettings({ [field]: value });
    }
  };

  const handleConcurrencyChange = (value: number) => {
    debouncedUpdateSettings({ book_deconstruction_concurrency: value });
  };

  // v0.19.0: 提示词注册表数量
  const [promptCount, setPromptCount] = useState(0);
  useEffect(() => {
    loggedInvoke<any[]>('list_prompt_entries')
      .then(data => setPromptCount(data.length))
      .catch(() => setPromptCount(0));
  }, []);

  const handleRewriteThresholdChange = (value: number) => {
    const normalized = normalizeFloat(value, 2);
    debouncedUpdateSettings({ rewrite_threshold: normalized });
  };

  const handleMaxFeedbackLoopsChange = (value: number) => {
    debouncedUpdateSettings({ max_feedback_loops: value });
  };

  const handleWritingStrategyChange = (partial: Partial<WritingStrategy>) => {
    debouncedUpdateSettings({ writing_strategy: { ...writingStrategy, ...partial } });
  };

  return (
    <div className="space-y-6">
      {/* 版本信息 */}
      {show('about') && (
        <Card>
          <CardContent className="p-6 space-y-4">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-4">
                <div className="w-16 h-16 rounded-xl bg-gradient-to-br from-terracotta to-terracotta/60 flex items-center justify-center">
                  <span className="text-white font-serif text-2xl font-bold">草</span>
                </div>
                <div>
                  <h3 className="text-lg font-medium text-white">StoryForge (草苔)</h3>
                  <p className="text-gray-400">当前版本: v{currentVersion}</p>
                  {hasUpdate && !isInstalling && (
                    <p className="text-terracotta text-sm">新版本可用: v{latestVersion}</p>
                  )}
                  {isInstalling && downloadProgress && (
                    <p className="text-cinema-gold text-sm">
                      正在下载 v{latestVersion}… {downloadProgress.percentage.toFixed(0)}%
                    </p>
                  )}
                  {error && <p className="text-red-400 text-sm">{error}</p>}
                </div>
              </div>
              <div className="flex gap-2">
                {hasUpdate ? (
                  <Button variant="primary" onClick={installUpdate} disabled={isInstalling}>
                    {isInstalling ? (
                      <>
                        <RefreshCw className="w-4 h-4 mr-2 animate-spin" />
                        更新中…
                      </>
                    ) : (
                      <>
                        <Download className="w-4 h-4 mr-2" />
                        立即更新
                      </>
                    )}
                  </Button>
                ) : (
                  <Button variant="secondary" onClick={checkUpdate} disabled={isChecking}>
                    {isChecking ? (
                      <>
                        <RefreshCw className="w-4 h-4 mr-2 animate-spin" />
                        检查中…
                      </>
                    ) : (
                      <>
                        <RefreshCw className="w-4 h-4 mr-2" />
                        检查更新
                      </>
                    )}
                  </Button>
                )}
              </div>
            </div>

            {/* 下载进度条 */}
            {isInstalling && downloadProgress && (
              <div className="space-y-1">
                <div className="h-2 bg-cinema-800 rounded-full overflow-hidden">
                  <div
                    className="h-full bg-terracotta rounded-full transition-all duration-300"
                    style={{ width: `${downloadProgress.percentage}%` }}
                  />
                </div>
                <div className="flex justify-between text-xs text-gray-500">
                  <span>下载更新包</span>
                  {downloadProgress.total && downloadProgress.total > 0 && (
                    <span>
                      {formatBytes(downloadProgress.downloaded)} /{' '}
                      {formatBytes(downloadProgress.total)}
                    </span>
                  )}
                </div>
              </div>
            )}
          </CardContent>
        </Card>
      )}

      {/* 拆书分析并发设置 */}
      {show('book') && (
        <Card>
          <CardContent className="p-6">
            <div className="flex items-center gap-3 mb-6">
              <div className="w-10 h-10 rounded-xl bg-cinema-gold/20 flex items-center justify-center">
                <BookOpen className="w-5 h-5 text-cinema-gold" />
              </div>
              <div>
                <h3 className="text-lg font-medium text-white">拆书分析设置</h3>
                <p className="text-sm text-gray-500">
                  调整拆书时的 LLM 并发数，本地模型可调大以加速分析
                </p>
              </div>
            </div>

            <div className="space-y-4">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <Zap className="w-4 h-4 text-cinema-gold" />
                  <span className="text-sm text-white">LLM 并发数</span>
                </div>
                <span
                  className={cn(
                    'text-lg font-bold text-cinema-gold font-mono',
                    isPending && 'opacity-70'
                  )}
                >
                  {concurrency}
                </span>
              </div>

              <div className="flex items-center gap-4">
                <span className="text-xs text-gray-500 w-8">1</span>
                <input
                  type="range"
                  min={1}
                  max={50}
                  value={concurrency}
                  onChange={e => handleConcurrencyChange(Number(e.target.value))}
                  className="flex-1 h-2 bg-cinema-800 rounded-lg appearance-none cursor-pointer accent-cinema-gold"
                />
                <span className="text-xs text-gray-500 w-8">50</span>
              </div>

              <div className="flex items-center justify-between text-xs text-gray-500">
                <span>保守（慢但稳）</span>
                <span>激进（快但占用资源）</span>
              </div>

              <div className="p-3 bg-cinema-900/50 rounded-lg border border-cinema-800">
                <p className="text-xs text-gray-400">
                  <span className="text-cinema-gold font-medium">提示：</span>
                  远程 API 建议 1~5，本地模型（Ollama/vLLM）建议 10~50。
                  当前设置会在下次拆书时生效。
                </p>
              </div>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Agent 配置 */}
      {show('agent') && (
        <Card>
          <CardContent className="p-6">
            <div className="flex items-center gap-3 mb-6">
              <div className="w-10 h-10 rounded-xl bg-cinema-gold/20 flex items-center justify-center">
                <Bot className="w-5 h-5 text-cinema-gold" />
              </div>
              <div>
                <h3 className="text-lg font-medium text-white">Agent 质检配置</h3>
                <p className="text-sm text-gray-500">
                  调整 Writer → Inspector 闭环优化的质检严格度
                </p>
              </div>
            </div>

            <div className="space-y-6">
              {/* 质检阈值 */}
              <div className="space-y-3">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <Sparkles className="w-4 h-4 text-cinema-gold" />
                    <span className="text-sm text-white">质检阈值</span>
                  </div>
                  <span
                    className={cn(
                      'text-lg font-bold text-cinema-gold font-mono',
                      isPending && 'opacity-70'
                    )}
                  >
                    {formatDisplayFloat(rewriteThreshold, 2)}
                  </span>
                </div>

                <div className="flex items-center gap-4">
                  <span className="text-xs text-gray-500 w-8">0.6</span>
                  <input
                    type="range"
                    min={0.6}
                    max={0.9}
                    step={0.05}
                    value={normalizeFloat(rewriteThreshold, 2)}
                    onChange={e => handleRewriteThresholdChange(Number(e.target.value))}
                    className="flex-1 h-2 bg-cinema-800 rounded-lg appearance-none cursor-pointer accent-cinema-gold"
                  />
                  <span className="text-xs text-gray-500 w-8">0.9</span>
                </div>

                <div className="flex items-center justify-between text-xs text-gray-500">
                  <span>宽松（易通过，改写少）</span>
                  <span>严格（难通过，改写多）</span>
                </div>

                <div className="p-3 bg-cinema-900/50 rounded-lg border border-cinema-800">
                  <p className="text-xs text-gray-400">
                    <span className="text-cinema-gold font-medium">提示：</span>
                    低于此阈值的文本将触发 Writer 自动改写。默认 0.75 是平衡点。
                  </p>
                </div>
              </div>

              {/* 最大循环次数 */}
              <div className="space-y-3">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <RefreshCw className="w-4 h-4 text-cinema-gold" />
                    <span className="text-sm text-white">最大改写轮数</span>
                  </div>
                  <span
                    className={cn(
                      'text-lg font-bold text-cinema-gold font-mono',
                      isPending && 'opacity-70'
                    )}
                  >
                    {maxFeedbackLoops}
                  </span>
                </div>

                <div className="flex items-center gap-4">
                  <span className="text-xs text-gray-500 w-8">1</span>
                  <input
                    type="range"
                    min={1}
                    max={5}
                    step={1}
                    value={maxFeedbackLoops}
                    onChange={e => handleMaxFeedbackLoopsChange(Number(e.target.value))}
                    className="flex-1 h-2 bg-cinema-800 rounded-lg appearance-none cursor-pointer accent-cinema-gold"
                  />
                  <span className="text-xs text-gray-500 w-8">5</span>
                </div>

                <div className="flex items-center justify-between text-xs text-gray-500">
                  <span>快速（1轮）</span>
                  <span>深度（5轮）</span>
                </div>

                <div className="p-3 bg-cinema-900/50 rounded-lg border border-cinema-800">
                  <p className="text-xs text-gray-400">
                    <span className="text-cinema-gold font-medium">提示：</span>
                    每轮 Inspector 质检不通过都会触发 Writer 改写。轮数越多质量越高但耗时越长。
                  </p>
                </div>
              </div>
            </div>
          </CardContent>
        </Card>
      )}

      {/* 写作策略 */}
      {show('writing') && (
        <>
          <Card>
            <CardContent className="p-6">
              <div className="flex items-center gap-3 mb-6">
                <div className="w-10 h-10 rounded-xl bg-cinema-gold/20 flex items-center justify-center">
                  <PenTool className="w-5 h-5 text-cinema-gold" />
                </div>
                <div>
                  <h3 className="text-lg font-medium text-white">写作策略</h3>
                  <p className="text-sm text-gray-500">调整 AI 生成内容的行为倾向</p>
                </div>
              </div>

              <div className="space-y-6">
                {/* v0.14.3: AI 生成模式 */}
                <div>
                  <label className="block text-sm text-gray-400 mb-2">
                    AI 生成模式
                    <span className="ml-2 text-xs text-cinema-gold/70">v0.14.3 新增</span>
                  </label>
                  <select
                    value={settings?.generation_mode ?? 'auto'}
                    onChange={e => {
                      const v = e.target.value as
                        | 'auto'
                        | 'time_sliced'
                        | 'fast'
                        | 'full'
                        | 'tri_shot';
                      // 直接保存，不使用防抖：select 每次点击只有一次事件，无需防抖；
                      // 乐观更新即时刷新 UI，避免受控组件在 800ms 防抖期间被弹回旧值。
                      updateSettings({ generation_mode: v });
                    }}
                    disabled={isPending}
                    className="w-full px-3 py-2 bg-cinema-800 border border-cinema-600 rounded-lg text-white text-sm focus:outline-none focus:ring-2 focus:ring-cinema-gold/50"
                  >
                    <option value="auto">智能路由（推荐）— 续写快速、重写精修</option>
                    <option value="tri_shot">
                      三击模式 — 智能合成资产，2~3 次 LLM，质检改写下沉后台（v0.23）
                    </option>
                    <option value="time_sliced">分时模式 — 最快（30-60秒）</option>
                    <option value="fast">快速模式 — 单次 + 风格技能（约 60秒）</option>
                    <option value="full">精修模式 — 含质检改写（2-5 分钟）</option>
                  </select>
                  <p className="text-xs text-gray-500 mt-1.5">
                    智能路由：续写场景自动用分时模式快速生成（推荐）；选中文本重写自动用精修模式含质检。三击模式用最快模型选资产合成提示词再生成，质检/改写/入库/洞察全部后台静默执行。可手动覆盖此行为。
                  </p>
                </div>

                {/* 运行模式 */}
                <div>
                  <label className="block text-sm text-gray-400 mb-2">运行模式</label>
                  <div className="grid grid-cols-2 gap-3">
                    <button
                      onClick={() => handleWritingStrategyChange({ run_mode: 'fast' })}
                      className={`p-3 rounded-lg text-left transition-colors border ${
                        writingStrategy.run_mode === 'fast'
                          ? 'bg-cinema-gold/20 border-cinema-gold/50'
                          : 'bg-cinema-800 border-transparent hover:bg-cinema-700'
                      }`}
                    >
                      <div className="font-medium text-white">快速</div>
                      <div className="text-xs text-gray-400 mt-0.5">高 temperature，注重效率</div>
                    </button>
                    <button
                      onClick={() => handleWritingStrategyChange({ run_mode: 'polish' })}
                      className={`p-3 rounded-lg text-left transition-colors border ${
                        writingStrategy.run_mode === 'polish'
                          ? 'bg-cinema-gold/20 border-cinema-gold/50'
                          : 'bg-cinema-800 border-transparent hover:bg-cinema-700'
                      }`}
                    >
                      <div className="font-medium text-white">精修</div>
                      <div className="text-xs text-gray-400 mt-0.5">低 temperature，注重质量</div>
                    </button>
                  </div>
                </div>

                {/* 冲突强度 */}
                <div className="space-y-3">
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      <Zap className="w-4 h-4 text-cinema-gold" />
                      <span className="text-sm text-white">冲突强度</span>
                    </div>
                    <span
                      className={cn(
                        'text-lg font-bold text-cinema-gold font-mono',
                        isPending && 'opacity-70'
                      )}
                    >
                      {writingStrategy.conflict_level}
                    </span>
                  </div>
                  <div className="flex items-center gap-4">
                    <span className="text-xs text-gray-500 w-8">0</span>
                    <input
                      type="range"
                      min={0}
                      max={100}
                      step={1}
                      value={writingStrategy.conflict_level}
                      onChange={e =>
                        handleWritingStrategyChange({
                          conflict_level: Number(e.target.value),
                        })
                      }
                      className="flex-1 h-2 bg-cinema-800 rounded-lg appearance-none cursor-pointer accent-cinema-gold"
                    />
                    <span className="text-xs text-gray-500 w-8">100</span>
                  </div>
                  <div className="flex items-center justify-between text-xs text-gray-500">
                    <span>平和抒情</span>
                    <span>激烈冲突</span>
                  </div>
                  {writingStrategy.conflict_level >= 80 && (
                    <div className="p-3 bg-cinema-900/50 rounded-lg border border-cinema-800">
                      <p className="text-xs text-gray-400">
                        <span className="text-cinema-gold font-medium">提示：</span>
                        冲突强度 ≥ 80 时，AI 会确保每 500 字至少安排一次冲突或张力。
                      </p>
                    </div>
                  )}
                </div>

                {/* 叙事节奏 */}
                <div>
                  <label className="block text-sm text-gray-400 mb-2">叙事节奏</label>
                  <div className="grid grid-cols-3 gap-3">
                    {[
                      { id: 'slow', label: '慢', desc: '细腻描写' },
                      { id: 'balanced', label: '均衡', desc: '动作描写交替' },
                      { id: 'fast', label: '快', desc: '快速推进' },
                    ].map(opt => (
                      <button
                        key={opt.id}
                        onClick={() =>
                          handleWritingStrategyChange({
                            pace: opt.id as WritingStrategy['pace'],
                          })
                        }
                        className={`p-3 rounded-lg text-left transition-colors border ${
                          writingStrategy.pace === opt.id
                            ? 'bg-cinema-gold/20 border-cinema-gold/50'
                            : 'bg-cinema-800 border-transparent hover:bg-cinema-700'
                        }`}
                      >
                        <div className="font-medium text-white">{opt.label}</div>
                        <div className="text-xs text-gray-400 mt-0.5">{opt.desc}</div>
                      </button>
                    ))}
                  </div>
                </div>

                {/* AI 自由度 */}
                <div>
                  <label className="block text-sm text-gray-400 mb-2">AI 自由度</label>
                  <div className="grid grid-cols-3 gap-3">
                    {[
                      { id: 'low', label: '低', desc: '严格遵循设定' },
                      { id: 'medium', label: '中', desc: '核心约束+发挥' },
                      { id: 'high', label: '高', desc: '允许创新转折' },
                    ].map(opt => (
                      <button
                        key={opt.id}
                        onClick={() =>
                          handleWritingStrategyChange({
                            ai_freedom: opt.id as WritingStrategy['ai_freedom'],
                          })
                        }
                        className={`p-3 rounded-lg text-left transition-colors border ${
                          writingStrategy.ai_freedom === opt.id
                            ? 'bg-cinema-gold/20 border-cinema-gold/50'
                            : 'bg-cinema-800 border-transparent hover:bg-cinema-700'
                        }`}
                      >
                        <div className="font-medium text-white">{opt.label}</div>
                        <div className="text-xs text-gray-400 mt-0.5">{opt.desc}</div>
                      </button>
                    ))}
                  </div>
                </div>
              </div>
            </CardContent>
          </Card>

          {/* v0.16.0 创作参数微调 */}
          <Card>
            <CardContent className="p-6">
              <div className="flex items-center gap-3 mb-6">
                <div className="w-10 h-10 rounded-xl bg-cinema-gold/20 flex items-center justify-center">
                  <SlidersHorizontal className="w-5 h-5 text-cinema-gold" />
                </div>
                <div>
                  <h3 className="text-lg font-medium text-white">创作参数</h3>
                  <p className="text-sm text-gray-500">
                    调整 AI 生成的行为倾向，数值越高 AI 越严格遵循对应规则
                  </p>
                </div>
              </div>
              <div className="space-y-5">
                {/* 跳过改写阈值 */}
                <div>
                  <div className="flex items-center justify-between mb-2">
                    <span className="text-sm text-gray-300">
                      跳过改写{' '}
                      <span className="text-xs text-gray-500">
                        — AI 对生成结果满意时直接跳过质检改写
                      </span>
                    </span>
                    <span className="text-cinema-gold font-mono font-bold">
                      {settings?.skip_rewrite_threshold ?? 0.9}
                    </span>
                  </div>
                  <input
                    type="range"
                    min={0.5}
                    max={0.99}
                    step={0.01}
                    value={settings?.skip_rewrite_threshold ?? 0.9}
                    onChange={e =>
                      debouncedUpdateSettings({ skip_rewrite_threshold: Number(e.target.value) })
                    }
                    className="w-full accent-cinema-gold"
                  />
                  <p className="text-xs text-gray-500 mt-1">
                    调低 = 更严格（每次都质检），调高 = 更宽松（结果满意就直接用，快）
                  </p>
                </div>
                {/* 风格权重 */}
                <div>
                  <div className="flex items-center justify-between mb-2">
                    <span className="text-sm text-gray-300">
                      风格权重{' '}
                      <span className="text-xs text-gray-500">— 文风一致性在 AI 决策中的占比</span>
                    </span>
                    <span className="text-cinema-gold font-mono font-bold">
                      {settings?.style_weight ?? 0.5}
                    </span>
                  </div>
                  <input
                    type="range"
                    min={0}
                    max={1}
                    step={0.05}
                    value={settings?.style_weight ?? 0.5}
                    onChange={e =>
                      debouncedUpdateSettings({ style_weight: Number(e.target.value) })
                    }
                    className="w-full accent-cinema-gold"
                  />
                  <p className="text-xs text-gray-500 mt-1">
                    调高 = 严格遵循既定文风，调低 = 允许 AI 自由发挥
                  </p>
                </div>
                {/* 叙事权重 */}
                <div>
                  <div className="flex items-center justify-between mb-2">
                    <span className="text-sm text-gray-300">
                      叙事权重{' '}
                      <span className="text-xs text-gray-500">— 情节连贯性在 AI 决策中的占比</span>
                    </span>
                    <span className="text-cinema-gold font-mono font-bold">
                      {settings?.narrative_weight ?? 0.5}
                    </span>
                  </div>
                  <input
                    type="range"
                    min={0}
                    max={1}
                    step={0.05}
                    value={settings?.narrative_weight ?? 0.5}
                    onChange={e =>
                      debouncedUpdateSettings({ narrative_weight: Number(e.target.value) })
                    }
                    className="w-full accent-cinema-gold"
                  />
                  <p className="text-xs text-gray-500 mt-1">
                    调高 = 更注重前后文衔接，调低 = 允许跳脱
                  </p>
                </div>
                {/* 上下文预算 */}
                <div>
                  <div className="flex items-center justify-between mb-2">
                    <span className="text-sm text-gray-300">
                      上下文预算{' '}
                      <span className="text-xs text-gray-500">
                        — 每次生成时喂给 AI 的历史文本量
                      </span>
                    </span>
                    <span className="text-cinema-gold font-mono font-bold">
                      {settings?.context_budget_ratio ?? 0.8}
                    </span>
                  </div>
                  <input
                    type="range"
                    min={0.2}
                    max={1.0}
                    step={0.05}
                    value={settings?.context_budget_ratio ?? 0.8}
                    onChange={e =>
                      debouncedUpdateSettings({ context_budget_ratio: Number(e.target.value) })
                    }
                    className="w-full accent-cinema-gold"
                  />
                  <p className="text-xs text-gray-500 mt-1">
                    调低 = 速度快但可能上下文不足，调高 = 更准确但处理速度慢
                  </p>
                </div>
              </div>
            </CardContent>
          </Card>

          {/* v0.16.0 超时设置 */}
          <Card>
            <CardContent className="p-6">
              <div className="flex items-center gap-3 mb-6">
                <div className="w-10 h-10 rounded-xl bg-cinema-gold/20 flex items-center justify-center">
                  <Clock className="w-5 h-5 text-cinema-gold" />
                </div>
                <div>
                  <h3 className="text-lg font-medium text-white">超时设置</h3>
                  <p className="text-sm text-gray-500">
                    模型连接与生成各阶段的等待时间上限。自托管模型建议调高，云端 API 可调低
                  </p>
                </div>
              </div>
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                {/* 连接超时 */}
                <div>
                  <label className="block text-xs text-gray-400 mb-1">
                    连接超时（秒）
                    <span className="text-gray-600 ml-1">
                      — 与模型服务端建立网络连接的最大等待时间
                    </span>
                  </label>
                  <input
                    type="number"
                    min={5}
                    max={120}
                    value={timeoutValues.llm_connect_timeout_secs}
                    onChange={e =>
                      handleTimeoutChange('llm_connect_timeout_secs', Number(e.target.value))
                    }
                    onBlur={() => handleTimeoutBlur('llm_connect_timeout_secs')}
                    className="w-full px-3 py-2 bg-cinema-800 border border-cinema-600 rounded-lg text-white text-sm"
                  />
                  <p className="text-xs text-gray-500 mt-1">
                    自托管模型（vllm/Ollama）建议 30，云端 API 建议 10
                  </p>
                </div>
                {/* 首字节超时 */}
                <div>
                  <label className="block text-xs text-gray-400 mb-1">
                    首字节超时（秒）
                    <span className="text-gray-600 ml-1">
                      — 发送请求后等待 AI 开始输出第一个字的时间
                    </span>
                  </label>
                  <input
                    type="number"
                    min={10}
                    max={300}
                    value={timeoutValues.llm_first_chunk_timeout_secs}
                    onChange={e =>
                      handleTimeoutChange('llm_first_chunk_timeout_secs', Number(e.target.value))
                    }
                    onBlur={() => handleTimeoutBlur('llm_first_chunk_timeout_secs')}
                    className="w-full px-3 py-2 bg-cinema-800 border border-cinema-600 rounded-lg text-white text-sm"
                  />
                  <p className="text-xs text-gray-500 mt-1">
                    模型冷启动（加载到显存）可能需要更长时间，可调至 120
                  </p>
                </div>
                {/* 单步超时 */}
                <div>
                  <label className="block text-xs text-gray-400 mb-1">
                    单步超时（秒）
                    <span className="text-gray-600 ml-1">
                      — 单个写作步骤（如质检、改写）的最大执行时间
                    </span>
                  </label>
                  <input
                    type="number"
                    min={10}
                    max={300}
                    value={timeoutValues.executor_step_timeout_secs}
                    onChange={e =>
                      handleTimeoutChange('executor_step_timeout_secs', Number(e.target.value))
                    }
                    onBlur={() => handleTimeoutBlur('executor_step_timeout_secs')}
                    className="w-full px-3 py-2 bg-cinema-800 border border-cinema-600 rounded-lg text-white text-sm"
                  />
                  <p className="text-xs text-gray-500 mt-1">单步超时应小于总超时，默认 90 秒</p>
                </div>
                {/* 生成总超时 */}
                <div>
                  <label className="block text-xs text-gray-400 mb-1">
                    生成总超时（秒）
                    <span className="text-gray-600 ml-1">
                      — 从开始创作到必须返回结果的总时间上限
                    </span>
                  </label>
                  <input
                    type="number"
                    min={30}
                    max={600}
                    value={timeoutValues.smart_execute_total_timeout_secs}
                    onChange={e =>
                      handleTimeoutChange(
                        'smart_execute_total_timeout_secs',
                        Number(e.target.value)
                      )
                    }
                    onBlur={() => handleTimeoutBlur('smart_execute_total_timeout_secs')}
                    className="w-full px-3 py-2 bg-cinema-800 border border-cinema-600 rounded-lg text-white text-sm"
                  />
                  <p className="text-xs text-gray-500 mt-1">
                    慢模型建议 300，快模型可设 120。应大于所有单步超时
                  </p>
                </div>
                {/* 前端超时 */}
                <div className="md:col-span-2">
                  <label className="block text-xs text-gray-400 mb-1">
                    前端超时（秒）
                    <span className="text-gray-600 ml-1">
                      — 界面等待后端响应的最长时间，超过此值弹出诊断卡片
                    </span>
                  </label>
                  <input
                    type="number"
                    min={30}
                    max={900}
                    value={timeoutValues.frontend_timeout_secs}
                    onChange={e =>
                      handleTimeoutChange('frontend_timeout_secs', Number(e.target.value))
                    }
                    onBlur={() => handleTimeoutBlur('frontend_timeout_secs')}
                    className="w-full px-3 py-2 bg-cinema-800 border border-cinema-600 rounded-lg text-white text-sm"
                  />
                  <p className="text-xs text-gray-500 mt-1">
                    应大于后端总超时（留 20-30 秒余量），默认 200
                  </p>
                </div>
              </div>
            </CardContent>
          </Card>
        </>
      )}

      {/* v0.19.0 提示词注册表入口 */}
      {show('prompts') && (
        <Card>
          <CardContent className="p-6">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-3">
                <div className="w-10 h-10 rounded-xl bg-cinema-gold/20 flex items-center justify-center">
                  <FileText className="w-5 h-5 text-cinema-gold" />
                </div>
                <div>
                  <h3 className="text-lg font-medium text-white">提示词注册表</h3>
                  <p className="text-sm text-gray-500">
                    查看和编辑所有 {promptCount} 条内置 LLM 提示词。支持按分类筛选、搜索、批量重置。
                  </p>
                </div>
              </div>
              <Button
                size="sm"
                variant="secondary"
                onClick={() => {
                  // 导航到提示词注册表标签页
                  const event = new CustomEvent('switch-settings-tab', {
                    detail: { tab: 'prompts' },
                  });
                  window.dispatchEvent(event);
                }}
              >
                <FileText className="w-4 h-4 mr-1" />
                打开注册表
              </Button>
            </div>
          </CardContent>
        </Card>
      )}

      {/* 编辑器设置 */}
      {show('appearance') && (
        <>
          <Card id="editor-settings-card" tabIndex={-1}>
            <CardContent className="p-6">
              <div className="flex items-center gap-3 mb-6">
                <div className="w-10 h-10 rounded-xl bg-cinema-gold/20 flex items-center justify-center">
                  <Settings2 className="w-5 h-5 text-cinema-gold" />
                </div>
                <div>
                  <h3 className="text-lg font-medium text-white">编辑器设置</h3>
                  <p className="text-sm text-gray-500">幕前写作界面的字体、风格等配置</p>
                </div>
              </div>
              <EditorSettings />
            </CardContent>
          </Card>

          {/* 颜色主题 */}
          <Card>
            <CardContent className="p-6">
              <div className="flex items-center gap-3 mb-6">
                <div className="w-10 h-10 rounded-xl bg-cinema-gold/20 flex items-center justify-center">
                  <Settings2 className="w-5 h-5 text-cinema-gold" />
                </div>
                <div>
                  <h3 className="text-lg font-medium text-white">颜色主题</h3>
                  <p className="text-sm text-gray-500">幕前写作界面的冷暖撞色色调</p>
                </div>
              </div>
              <ColorThemeSelector />
            </CardContent>
          </Card>
        </>
      )}
    </div>
  );
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
}
