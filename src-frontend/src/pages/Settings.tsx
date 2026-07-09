/**
 * Settings Page - 工作室配置（v0.26.39 七 Tab 重组）
 *
 * 模型 | Agent | 写作 | 提示词 | 外观 | 关于 | 账号
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
} from 'lucide-react';
import { Button } from '@/components/ui/Button';
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

export type SettingsTabType =
  | 'models'
  | 'agents'
  | 'writing'
  | 'prompts'
  | 'appearance'
  | 'about'
  | 'account';

type ModelSubTab = 'manage' | 'routing' | 'health';

const SETTINGS_TABS: { id: SettingsTabType; label: string; icon: React.ElementType }[] = [
  { id: 'models', label: '模型', icon: Cpu },
  { id: 'agents', label: 'Agent', icon: Bot },
  { id: 'writing', label: '写作', icon: PenTool },
  { id: 'prompts', label: '提示词', icon: FileText },
  { id: 'appearance', label: '外观', icon: Palette },
  { id: 'about', label: '关于', icon: Info },
  { id: 'account', label: '账号', icon: User },
];

function normalizeSettingsTab(raw: string | null | undefined): SettingsTabType | null {
  if (!raw) return null;
  if (raw === 'editor' || raw === 'appearance') return 'appearance';
  if (raw === 'general') return 'about';
  if (raw === 'methodology' || raw === 'workflows') return 'writing';
  if (raw === 'routing' || raw === 'health') return 'models';
  if (raw === 'stats') return 'about'; // 统计已迁至数据洞察
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
    <div className="p-8 space-y-6 animate-fade-in">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="font-display text-3xl font-bold text-white">工作室配置</h1>
          <p className="text-gray-400">模型、写作、提示词与账号</p>
        </div>
        <div className="flex items-center gap-3">
          <Button
            variant="ghost"
            onClick={() => exportSettings.mutate()}
            isLoading={exportSettings.isPending}
          >
            <Download className="w-4 h-4 mr-2" />
            导出设置
          </Button>
          <label className="cursor-pointer inline-flex items-center gap-2 px-4 py-2 text-gray-400 hover:text-white hover:bg-cinema-800/50 rounded-xl transition-all">
            <input type="file" accept=".json" className="hidden" onChange={handleImport} />
            {importSettings.isPending ? (
              <span className="w-4 h-4 border-2 border-current border-t-transparent rounded-full animate-spin" />
            ) : (
              <Upload className="w-4 h-4" />
            )}
            导入设置
          </label>
        </div>
      </div>

      <div
        className="flex items-center gap-2 border-b border-cinema-800 pb-4 overflow-x-auto"
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

      {isLoading ? (
        <div className="text-center py-12 text-gray-500">加载中...</div>
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
                  ] as const
                ).map(sub => {
                  const Icon = sub.icon;
                  return (
                    <button
                      key={sub.id}
                      type="button"
                      onClick={() => setModelSubTab(sub.id)}
                      className={cn(
                        'flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-sm transition-colors',
                        modelSubTab === sub.id
                          ? 'bg-cinema-800 text-cinema-gold border border-cinema-gold/30'
                          : 'text-gray-400 hover:text-white hover:bg-cinema-800/50'
                      )}
                    >
                      <Icon className="w-3.5 h-3.5" />
                      {sub.label}
                    </button>
                  );
                })}
              </div>
              {modelSubTab === 'manage' && <UnifiedModelManager />}
              {modelSubTab === 'routing' && <RouteSimulator />}
              {modelSubTab === 'health' && <ModelHealthPanel />}
            </div>
          )}
          {activeTab === 'agents' && (
            <div className="space-y-6">
              <AgentConfig />
              <GeneralSettings sections={['agent']} />
            </div>
          )}
          {activeTab === 'writing' && (
            <div className="space-y-6">
              <MethodologySettings />
              <WorkflowSettings />
              <GeneralSettings sections={['writing']} />
            </div>
          )}
          {activeTab === 'prompts' && <PromptsPanel />}
          {activeTab === 'appearance' && <GeneralSettings sections={['appearance']} />}
          {activeTab === 'about' && <GeneralSettings sections={['about']} />}
          {activeTab === 'account' && <AccountSettings />}
        </>
      )}
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
      onClick={onClick}
      className={cn(
        'flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition-colors whitespace-nowrap',
        active ? 'bg-cinema-gold text-black' : 'text-gray-400 hover:text-white hover:bg-cinema-800'
      )}
    >
      {icon}
      {label}
    </button>
  );
}
