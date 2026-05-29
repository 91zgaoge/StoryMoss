/**
 * Settings Page - 工作室配置
 *
 * 功能：
 * - 统一模型管理（聊天/嵌入/多模态/图像）
 * - Agent模型映射
 * - 设置导出/导入
 */

import { useState, useEffect } from 'react';
import {
  Settings2,
  Download,
  Upload,
  Bot,
  Compass,
  User,
  GitBranch,
  BarChart3,
  Cpu,
} from 'lucide-react';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { useSettings, useExportSettings, useImportSettings } from '@/hooks/useSettings';
import { cn } from '@/utils/cn';
import { logFeatureUsage } from '@/services/tauri';

import { UnifiedModelManager } from './settings/UnifiedModelManager';
import { StatsSettings } from './settings/StatsSettings';
import { MethodologySettings } from './settings/MethodologySettings';
import { WorkflowSettings } from './settings/WorkflowSettings';
import { GeneralSettings } from './settings/GeneralSettings';
import { AccountSettings } from './settings/AccountSettings';

type TabType = 'models' | 'agents' | 'methodology' | 'workflows' | 'general' | 'account' | 'stats';

export function Settings() {
  const [activeTab, setActiveTab] = useState<TabType>('models');

  const { data: settings, isLoading: settingsLoading } = useSettings();
  const exportSettings = useExportSettings();
  const importSettings = useImportSettings();

  const isLoading = settingsLoading;

  // Feature usage telemetry
  useEffect(() => {
    if (activeTab === 'stats') {
      logFeatureUsage('feature_stats', 'opened');
    }
  }, [activeTab]);

  // 处理设置导入
  const handleImport = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) {
      importSettings.mutate(file);
    }
  };

  return (
    <div className="p-8 space-y-6 animate-fade-in">
      {/* 头部 */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="font-display text-3xl font-bold text-white">工作室配置</h1>
          <p className="text-gray-400">配置LLM模型和全局设置</p>
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

      {/* 标签页 */}
      <div className="flex items-center gap-2 border-b border-cinema-800 pb-4 overflow-x-auto">
        <TabButton
          active={activeTab === 'models'}
          onClick={() => setActiveTab('models')}
          icon={<Cpu className="w-4 h-4" />}
          label="模型管理"
        />
        <TabButton
          active={activeTab === 'agents'}
          onClick={() => setActiveTab('agents')}
          icon={<Bot className="w-4 h-4" />}
          label="Agent配置"
        />
        <TabButton
          active={activeTab === 'methodology'}
          onClick={() => setActiveTab('methodology')}
          icon={<Compass className="w-4 h-4" />}
          label="创作方法论"
        />
        <TabButton
          active={activeTab === 'workflows'}
          onClick={() => setActiveTab('workflows')}
          icon={<GitBranch className="w-4 h-4" />}
          label="工作流"
        />
        <TabButton
          active={activeTab === 'general'}
          onClick={() => setActiveTab('general')}
          icon={<Settings2 className="w-4 h-4" />}
          label="通用设置"
        />
        <TabButton
          active={activeTab === 'stats'}
          onClick={() => setActiveTab('stats')}
          icon={<BarChart3 className="w-4 h-4" />}
          label="数据统计"
        />
        <TabButton
          active={activeTab === 'account'}
          onClick={() => setActiveTab('account')}
          icon={<User className="w-4 h-4" />}
          label="账号与登录"
        />
      </div>

      {/* 内容区域 */}
      {isLoading ? (
        <div className="text-center py-12 text-gray-500">加载中...</div>
      ) : (
        <>
          {activeTab === 'models' && <UnifiedModelManager />}
          {activeTab === 'agents' && <AgentConfig />}
          {activeTab === 'methodology' && <MethodologySettings />}
          {activeTab === 'workflows' && <WorkflowSettings />}
          {activeTab === 'general' && <GeneralSettings />}
          {activeTab === 'stats' && <StatsSettings />}
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

// Agent配置组件
function AgentConfig() {
  return (
    <Card>
      <CardContent className="p-8 text-center">
        <Bot className="w-16 h-16 text-gray-600 mx-auto mb-4" />
        <h3 className="text-lg font-medium text-white mb-2">Agent模型映射</h3>
        <p className="text-gray-500">为不同的Agent配置专用的LLM模型</p>
        <p className="text-sm text-gray-600 mt-4">功能开发中...</p>
      </CardContent>
    </Card>
  );
}
