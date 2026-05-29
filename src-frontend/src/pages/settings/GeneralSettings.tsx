import { useState, useEffect } from 'react';
import {
  Settings2,
  BookOpen,
  Zap,
  Bot,
  Sparkles,
  RefreshCw,
  Download,
  PenTool,
} from 'lucide-react';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { useUpdater } from '@/hooks/useUpdater';
import { useSettings, useSaveSettings } from '@/hooks/useSettings';
import { EditorSettings } from '@/components/EditorSettings';
import {
  colorThemeList,
  applyColorTheme,
  loadColorTheme,
  type ColorThemeId,
} from '@/frontstage/config/colorThemes';
import { cn } from '@/utils/cn';
import { normalizeFloat, formatDisplayFloat } from '@/utils/numberFormat';

// 颜色主题选择器组件
function ColorThemeSelector() {
  const [currentTheme, setCurrentTheme] = useState<ColorThemeId>(() => loadColorTheme());

  const handleSelect = (themeId: ColorThemeId) => {
    setCurrentTheme(themeId);
    applyColorTheme(themeId);
    localStorage.setItem('storyforge-color-theme', themeId);
  };

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

// 通用设置组件
export function GeneralSettings() {
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

  const { data: settings } = useSettings();
  const saveSettingsMutation = useSaveSettings();
  const [concurrency, setConcurrency] = useState(settings?.book_deconstruction_concurrency ?? 3);
  const [rewriteThreshold, setRewriteThreshold] = useState(settings?.rewrite_threshold ?? 0.75);
  const [maxFeedbackLoops, setMaxFeedbackLoops] = useState(settings?.max_feedback_loops ?? 2);
  const [writingStrategy, setWritingStrategy] = useState(
    settings?.writing_strategy ?? {
      run_mode: 'fast' as const,
      conflict_level: 50,
      pace: 'balanced' as const,
      ai_freedom: 'medium' as const,
    }
  );

  // 同步设置值
  useEffect(() => {
    if (settings?.book_deconstruction_concurrency !== undefined) {
      setConcurrency(settings.book_deconstruction_concurrency);
    }
    if (settings?.rewrite_threshold !== undefined) {
      setRewriteThreshold(settings.rewrite_threshold);
    }
    if (settings?.max_feedback_loops !== undefined) {
      setMaxFeedbackLoops(settings.max_feedback_loops);
    }
    if (settings?.writing_strategy !== undefined) {
      setWritingStrategy(settings.writing_strategy);
    }
  }, [
    settings?.book_deconstruction_concurrency,
    settings?.rewrite_threshold,
    settings?.max_feedback_loops,
    settings?.writing_strategy,
  ]);

  const handleConcurrencyChange = (value: number) => {
    setConcurrency(value);
    // 防抖保存：300ms 后保存
    const timer = setTimeout(() => {
      if (settings) {
        saveSettingsMutation.mutate({
          ...settings,
          book_deconstruction_concurrency: value,
        });
      }
    }, 300);
    return () => clearTimeout(timer);
  };

  const handleRewriteThresholdChange = (value: number) => {
    const normalized = normalizeFloat(value, 2);
    setRewriteThreshold(normalized);
    const timer = setTimeout(() => {
      if (settings) {
        saveSettingsMutation.mutate({
          ...settings,
          rewrite_threshold: normalized,
        });
      }
    }, 300);
    return () => clearTimeout(timer);
  };

  const handleMaxFeedbackLoopsChange = (value: number) => {
    setMaxFeedbackLoops(value);
    const timer = setTimeout(() => {
      if (settings) {
        saveSettingsMutation.mutate({
          ...settings,
          max_feedback_loops: value,
        });
      }
    }, 300);
    return () => clearTimeout(timer);
  };

  const handleWritingStrategyChange = (partial: Partial<typeof writingStrategy>) => {
    const next = { ...writingStrategy, ...partial };
    setWritingStrategy(next);
    const timer = setTimeout(() => {
      if (settings) {
        saveSettingsMutation.mutate({
          ...settings,
          writing_strategy: next,
        });
      }
    }, 300);
    return () => clearTimeout(timer);
  };

  return (
    <div className="space-y-6">
      {/* 版本信息 */}
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

      {/* 拆书分析并发设置 */}
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
              <span className="text-lg font-bold text-cinema-gold font-mono">{concurrency}</span>
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
                远程 API 建议 1~5，本地模型（Ollama/vLLM）建议 10~50。 当前设置会在下次拆书时生效。
              </p>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Agent 配置 */}
      <Card>
        <CardContent className="p-6">
          <div className="flex items-center gap-3 mb-6">
            <div className="w-10 h-10 rounded-xl bg-cinema-gold/20 flex items-center justify-center">
              <Bot className="w-5 h-5 text-cinema-gold" />
            </div>
            <div>
              <h3 className="text-lg font-medium text-white">Agent 质检配置</h3>
              <p className="text-sm text-gray-500">调整 Writer → Inspector 闭环优化的质检严格度</p>
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
                <span className="text-lg font-bold text-cinema-gold font-mono">
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
                <span className="text-lg font-bold text-cinema-gold font-mono">
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

      {/* 写作策略 */}
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
                <span className="text-lg font-bold text-cinema-gold font-mono">
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
                        pace: opt.id as typeof writingStrategy.pace,
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
                        ai_freedom: opt.id as typeof writingStrategy.ai_freedom,
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

      {/* 编辑器设置 */}
      <Card>
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
