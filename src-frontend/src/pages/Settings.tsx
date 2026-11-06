/**
 * Settings Page - 工作室配置（机械backstage风格）
 *
 * 模型 | Agent | 写作 | 提示词 | 扩展 | 外观 | 关于 | 账号
 */

import { useState, useEffect } from 'react';
import {
  Download,
  Upload,
  Bot,
  User,
  Cpu,
  Route,
  HeartPulse,
  FileText,
  PenTool,
  Palette,
  Info,
  Plug,
  Clock,
} from 'lucide-react';
import { Button } from '@/components/ui/Button';
import { Panel } from '@/components/ui/Panel';
import { StudioNavRail } from '@/components/ui/StudioNavRail';
import { useExportSettings, useImportSettings } from '@/hooks/useSettings';
import { useSettingsContext } from '@/hooks/useSettingsContext';
import { cn } from '@/utils/cn';
import { useAppStore } from '@/stores/appStore';

import { UnifiedModelManager } from './settings/UnifiedModelManager';
import { RouteSimulator } from './settings/RouteSimulator';
import { ModelHealthPanel } from './settings/ModelHealthPanel';
import { MethodologySettings } from './settings/MethodologySettings';
import { WorkflowSettings } from './settings/WorkflowSettings';
import { GeneralSettings } from './settings/GeneralSettings';
import { AgentConfig } from './settings/AgentConfig';
import { AccountSettings } from './settings/AccountSettings';
import { PromptsPanel } from './settings/PromptsPanel';
import { Mcp } from './Mcp';

export type SettingsTabType =
  | 'models'
  | 'agents'
  | 'writing'
  | 'prompts'
  | 'extensions'
  | 'appearance'
  | 'about'
  | 'account';

type ModelSubTab = 'manage' | 'routing' | 'health' | 'timeout';

const SETTINGS_TABS: { id: SettingsTabType; label: string; icon: React.ElementType }[] = [
  { id: 'models', label: '模型', icon: Cpu },
  { id: 'agents', label: 'Agent', icon: Bot },
  { id: 'writing', label: '写作', icon: PenTool },
  { id: 'prompts', label: '提示词', icon: FileText },
  { id: 'extensions', label: '扩展', icon: Plug },
  { id: 'appearance', label: '外观', icon: Palette },
  { id: 'about', label: '关于', icon: Info },
  { id: 'account', label: '账号', icon: User },
];

function normalizeSettingsTab(raw: string | null | undefined): SettingsTabType | null {
  if (!raw) return null;
  if (raw === 'editor' || raw === 'appearance') return 'appearance';
  if (raw === 'general') return 'about';
  if (raw === 'methodology' || raw === 'workflows') return 'writing';
  if (raw === 'routing' || raw === 'health' || raw === 'timeout' || raw === 'timeouts')
    return 'models';
  if (raw === 'stats') return 'about'; // 统计已迁至数据洞察
  if (raw === 'mcp') return 'extensions';
  if (SETTINGS_TABS.some(t => t.id === raw)) return raw as SettingsTabType;
  return null;
}

export function Settings() {
  const settingsTab = useAppStore(s => s.settingsTab);
  const setSettingsTab = useAppStore(s => s.setSettingsTab);
  const [activeTab, setActiveTab] = useState<SettingsTabType>(
    () => normalizeSettingsTab(settingsTab) ?? 'models'
  );
  const [modelSubTab, setModelSubTab] = useState<ModelSubTab>('manage');

  const { isLoading } = useSettingsContext();
  const exportSettings = useExportSettings();
  const importSettings = useImportSettings();

  // 消费 store 深链
  useEffect(() => {
    const normalized = normalizeSettingsTab(settingsTab);
    if (normalized) {
      setActiveTab(normalized);
      if (settingsTab === 'routing') setModelSubTab('routing');
      if (settingsTab === 'health') setModelSubTab('health');
      if (settingsTab === 'timeout' || settingsTab === 'timeouts') setModelSubTab('timeout');
      setSettingsTab(null);
      if (normalized === 'appearance') {
        setTimeout(() => {
          document
            .getElementById('editor-settings-card')
            ?.scrollIntoView({ behavior: 'smooth', block: 'center' });
        }, 100);
      }
    }
  }, [settingsTab, setSettingsTab]);

  useEffect(() => {
    const handler = (e: Event) => {
      const detail = (e as CustomEvent).detail;
      const normalized = normalizeSettingsTab(detail?.tab);
      if (normalized) {
        setActiveTab(normalized);
        if (detail?.tab === 'routing') setModelSubTab('routing');
        if (detail?.tab === 'health') setModelSubTab('health');
        if (detail?.tab === 'timeout' || detail?.tab === 'timeouts') setModelSubTab('timeout');
      }
    };
    window.addEventListener('switch-settings-tab', handler);
    return () => window.removeEventListener('switch-settings-tab', handler);
  }, []);

  useEffect(() => {
    const handler = (e: Event) => {
      const detail = (e as CustomEvent).detail;
      if (detail?.view === 'settings' && detail?.panel === 'editor') {
        setActiveTab('appearance');
        setTimeout(() => {
          document
            .getElementById('editor-settings-card')
            ?.scrollIntoView({ behavior: 'smooth', block: 'center' });
        }, 100);
      }
      if (detail?.view === 'settings' && detail?.panel === 'account') {
        setActiveTab('account');
      }
    };
    window.addEventListener('backstage-navigate-to-panel', handler);
    return () => window.removeEventListener('backstage-navigate-to-panel', handler);
  }, []);

  const handleImport = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) {
      importSettings.mutate(file);
    }
  };

  return (
    <div className="flex h-full min-h-0 bg-cinema-950 font-body text-ai-ink">
      {/* 左侧导航轨 */}
      <StudioNavRail activeView="settings" />

      {/* 主区 */}
      <main className="flex min-w-0 min-h-0 flex-1 flex-col">
        <header className="flex h-16 shrink-0 flex-wrap items-center justify-between gap-3 border-b border-borderSubtle bg-cinema-900/70 px-5 backdrop-blur-sm sm:px-6">
          <h1 className="font-body text-base font-semibold text-ai-ink">工作室配置</h1>
          <div className="flex items-center gap-3">
            <Button
              variant="ghost"
              onClick={() => exportSettings.mutate()}
              isLoading={exportSettings.isPending}
            >
              <Download className="w-4 h-4 mr-2" />
              导出设置
            </Button>
            <label className="cursor-pointer inline-flex items-center justify-center font-medium transition-all active:scale-95 px-4 py-2 text-sm rounded-panel bg-transparent border border-cinema-600 text-cinema-gold hover:bg-cinema-800 focus-within:outline-none focus-within:ring-2 focus-within:ring-cinema-gold/30 disabled:opacity-50 disabled:cursor-not-allowed">
              <input type="file" accept=".json" className="hidden" onChange={handleImport} />
              {importSettings.isPending ? (
                <span className="w-4 h-4 border-2 border-current border-t-transparent rounded-full animate-spin mr-2" />
              ) : (
                <Upload className="w-4 h-4 mr-2" />
              )}
              导入设置
            </label>
          </div>
        </header>

        <div
          className="settings-panel-scope min-h-0 flex-1 overflow-y-auto"
          data-testid="settings-scroll"
        >
          <div className="mx-auto flex max-w-[1440px] flex-col gap-6 p-4 sm:p-6 xl:grid xl:grid-cols-[196px_minmax(0,1fr)] xl:items-start">
            {/* Tab 水平轨 / 宽屏设置导航 */}
            <div
              className="flex items-center gap-2 overflow-x-auto pb-2 xl:sticky xl:top-0 xl:flex-col xl:items-stretch xl:gap-1 xl:overflow-visible xl:pb-0"
              data-testid="settings-tabs"
            >
              {SETTINGS_TABS.map(tab => {
                const Icon = tab.icon;
                return (
                  <TabButton
                    key={tab.id}
                    active={activeTab === tab.id}
                    onClick={() => setActiveTab(tab.id)}
                    icon={<Icon className="w-4 h-4" />}
                    label={tab.label}
                  />
                );
              })}
            </div>

            <div className="min-w-0">
              {isLoading ? (
                <div className="text-center py-12 text-cinema-gold/50">加载中...</div>
              ) : (
                <>
                  {activeTab === 'models' && (
                    <div className="space-y-4">
                      <div className="flex items-center gap-2">
                        {(
                          [
                            { id: 'manage' as const, label: '管理', icon: Cpu },
                            { id: 'routing' as const, label: '路由模拟', icon: Route },
                            { id: 'health' as const, label: '健康', icon: HeartPulse },
                            { id: 'timeout' as const, label: '超时', icon: Clock },
                          ] as const
                        ).map(sub => {
                          const Icon = sub.icon;
                          return (
                            <button
                              key={sub.id}
                              type="button"
                              aria-current={modelSubTab === sub.id ? 'true' : undefined}
                              onClick={() => setModelSubTab(sub.id)}
                              className={cn(
                                'flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cinema-gold/50',
                                modelSubTab === sub.id
                                  ? 'bg-cinema-gold/10 text-cinema-gold border border-cinema-gold/30'
                                  : 'text-cinema-gold/60 hover:text-cinema-gold hover:bg-cinema-800/50'
                              )}
                            >
                              <Icon className="w-3.5 h-3.5" />
                              {sub.label}
                            </button>
                          );
                        })}
                      </div>
                      {modelSubTab === 'manage' && (
                        <Panel title="模型管理">
                          <UnifiedModelManager />
                        </Panel>
                      )}
                      {modelSubTab === 'routing' && (
                        <Panel title="路由模拟">
                          <RouteSimulator />
                        </Panel>
                      )}
                      {modelSubTab === 'health' && (
                        <Panel title="模型健康">
                          <ModelHealthPanel />
                        </Panel>
                      )}
                      {modelSubTab === 'timeout' && <GeneralSettings sections={['timeouts']} />}
                    </div>
                  )}
                  {activeTab === 'agents' && (
                    <div className="space-y-4">
                      <Panel title="Agent 配置">
                        <AgentConfig />
                      </Panel>
                      <GeneralSettings sections={['agent']} />
                    </div>
                  )}
                  {activeTab === 'writing' && (
                    <div className="space-y-4">
                      <Panel title="方法论">
                        <MethodologySettings />
                      </Panel>
                      <Panel title="工作流">
                        <WorkflowSettings />
                      </Panel>
                      <GeneralSettings sections={['writing']} />
                    </div>
                  )}
                  {activeTab === 'prompts' && (
                    <Panel title="提示词注册表">
                      <PromptsPanel />
                    </Panel>
                  )}
                  {activeTab === 'extensions' && (
                    <div className="space-y-3">
                      <p
                        className="text-xs text-cinema-gold/50 leading-relaxed"
                        data-testid="extensions-note"
                      >
                        MCP 扩展连接不进入默认续写热路径；仅供高级工具调用与外部服务器联调。
                      </p>
                      <Panel title="MCP 扩展">
                        <Mcp />
                      </Panel>
                    </div>
                  )}
                  {activeTab === 'appearance' && <GeneralSettings sections={['appearance']} />}
                  {activeTab === 'about' && <GeneralSettings sections={['about']} />}
                  {activeTab === 'account' && <AccountSettings />}
                </>
              )}
            </div>
          </div>
        </div>
      </main>
    </div>
  );
}

function TabButton({
  active,
  onClick,
  icon,
  label,
}: {
  active: boolean;
  onClick: () => void;
  icon: React.ReactNode;
  label: string;
}) {
  return (
    <button
      type="button"
      aria-current={active ? 'true' : undefined}
      onClick={onClick}
      className={cn(
        'flex items-center gap-2 whitespace-nowrap rounded-lg px-4 py-2.5 text-left text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cinema-gold/50 xl:w-full',
        active
          ? 'bg-cinema-gold/10 text-cinema-gold'
          : 'text-ai-ink-2 hover:bg-cinema-800/60 hover:text-ai-ink'
      )}
    >
      {icon}
      {label}
    </button>
  );
}
