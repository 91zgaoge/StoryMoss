import { useState } from 'react';
import { Star, Edit2, Key, Globe, RotateCcw, ChevronDown, ChevronUp, Trash2 } from 'lucide-react';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { cn } from '@/utils/cn';
import type { ModelConfig, ConnectionTestResult } from '@/types/llm';
import { getModelProviders } from '@/services/settings';
import { formatLatencyWithQuality } from '@/utils/numberFormat';

export function ModelCard({
  model,
  isActive,
  connectionStatus,
  onEdit,
  onSetActive,
  onRetry,
  onDelete,
}: {
  model: ModelConfig;
  isActive?: boolean;
  connectionStatus?: {
    result?: ConnectionTestResult;
    isChecking?: boolean;
  };
  onEdit: () => void;
  onSetActive: () => void;
  onRetry?: () => void;
  onDelete?: () => void;
}) {
  const [showSteps, setShowSteps] = useState(false);
  const isDefault = model.is_default;
  const providerMeta = getModelProviders().find(p => p.id === model.provider);
  const requiresApiKey = providerMeta?.requiresApiKey ?? true;
  // 后端 api_key 返回 '***' 表示有密钥，null/undefined/'' 表示无密钥
  const hasApiKey = model.api_key === '***' || (!!model.api_key && model.api_key !== '');

  return (
    <Card className={cn(isActive && 'border-cinema-gold ring-1 ring-cinema-gold/30')}>
      <CardContent className="p-5">
        <div className="flex items-start justify-between">
          <div className="flex items-center gap-4">
            {/* 提供商图标 */}
            <div
              className={cn(
                'w-12 h-12 rounded-xl flex items-center justify-center',
                isActive ? 'bg-cinema-gold/20' : 'bg-cinema-800'
              )}
            >
              {model.provider === 'openai' && (
                <span
                  className={cn(
                    'font-bold text-lg',
                    isActive ? 'text-cinema-gold' : 'text-green-400'
                  )}
                >
                  O
                </span>
              )}
              {model.provider === 'anthropic' && (
                <span
                  className={cn(
                    'font-bold text-lg',
                    isActive ? 'text-cinema-gold' : 'text-orange-400'
                  )}
                >
                  A
                </span>
              )}
              {model.provider === 'ollama' && (
                <span
                  className={cn(
                    'font-bold text-lg',
                    isActive ? 'text-cinema-gold' : 'text-blue-400'
                  )}
                >
                  L
                </span>
              )}
              {model.provider === 'azure' && (
                <span
                  className={cn(
                    'font-bold text-lg',
                    isActive ? 'text-cinema-gold' : 'text-blue-500'
                  )}
                >
                  Az
                </span>
              )}
              {!['openai', 'anthropic', 'ollama', 'azure'].includes(model.provider) && (
                <Globe className={cn('w-6 h-6', isActive ? 'text-cinema-gold' : 'text-gray-400')} />
              )}
            </div>

            <div>
              <div className="flex items-center gap-2">
                <h3 className="font-semibold text-white text-lg">{model.name}</h3>
                {isActive && (
                  <span className="px-2 py-0.5 bg-cinema-gold text-black text-xs rounded-full font-medium flex items-center gap-1">
                    <Star className="w-3 h-3" />
                    当前使用
                  </span>
                )}
                {!isActive && isDefault && (
                  <span className="px-2 py-0.5 bg-cinema-gold/20 text-cinema-gold text-xs rounded-full">
                    默认
                  </span>
                )}
                {!model.enabled && (
                  <span className="px-2 py-0.5 bg-red-500/20 text-red-400 text-xs rounded-full">
                    禁用
                  </span>
                )}
              </div>
              <p className="text-sm text-gray-500">
                {model.provider} · {model.model}
              </p>
              {model.description && (
                <p className="text-sm text-gray-400 mt-1">{model.description}</p>
              )}
            </div>
          </div>

          <div className="flex items-center gap-3">
            {connectionStatus && connectionStatus.isChecking && (
              <span className="flex items-center gap-1.5 text-gray-400 text-sm">
                <span className="w-3.5 h-3.5 rounded-full border-2 border-current border-t-transparent animate-spin" />
                {connectionStatus.result?.steps?.find(s => s.status === 'running')?.name ||
                  '检测中'}
              </span>
            )}
            {connectionStatus &&
              !connectionStatus.isChecking &&
              connectionStatus.result?.success && (
                <span className="flex items-center gap-1.5 text-green-400 text-sm">
                  <span className="w-2.5 h-2.5 rounded-full bg-green-400" />
                  {formatLatencyWithQuality(connectionStatus.result.latency)}
                </span>
              )}
            {connectionStatus &&
              !connectionStatus.isChecking &&
              !connectionStatus.result?.success && (
                <span className="flex items-center gap-1.5 text-red-400 text-sm">
                  <span className="w-2.5 h-2.5 rounded-full bg-red-400" />
                  连接失败
                  {onRetry && (
                    <button onClick={onRetry} className="ml-1 text-xs underline hover:text-red-300">
                      重试
                    </button>
                  )}
                </span>
              )}
            {requiresApiKey && !hasApiKey && (
              <span className="flex items-center gap-1 text-amber-400 text-sm">
                <Key className="w-4 h-4" />
                需配置API Key
              </span>
            )}
            {!isActive && (
              <button
                onClick={onSetActive}
                className="px-3 py-1.5 text-xs font-medium text-cinema-gold bg-cinema-gold/10 hover:bg-cinema-gold/20 rounded-lg transition-colors flex items-center gap-1"
                title="设为当前使用"
              >
                <Star className="w-3.5 h-3.5" />
                设为当前
              </button>
            )}
            <Button variant="ghost" size="sm" onClick={onEdit}>
              <Edit2 className="w-4 h-4" />
            </Button>
            {onDelete && (
              <Button
                variant="ghost"
                size="sm"
                onClick={onDelete}
                title="删除模型配置"
                className="text-red-400 hover:text-red-300 hover:bg-red-500/10"
              >
                <Trash2 className="w-4 h-4" />
              </Button>
            )}
          </div>
        </div>

        {/* 连接失败步骤详情 */}
        {connectionStatus &&
          !connectionStatus.isChecking &&
          !connectionStatus.result?.success &&
          connectionStatus.result?.steps &&
          connectionStatus.result.steps.length > 0 && (
            <div className="mt-4">
              <button
                onClick={() => setShowSteps(v => !v)}
                className="flex items-center gap-1 text-xs text-red-400/80 hover:text-red-300 transition-colors"
              >
                {showSteps ? (
                  <ChevronUp className="w-3 h-3" />
                ) : (
                  <ChevronDown className="w-3 h-3" />
                )}
                {showSteps ? '收起详情' : '查看检测详情'}
              </button>
              {showSteps && (
                <div className="mt-2 space-y-1.5">
                  {connectionStatus.result.steps.map((step, idx) => (
                    <div key={idx} className="flex items-center gap-2 text-xs">
                      <span
                        className={cn(
                          'w-1.5 h-1.5 rounded-full flex-shrink-0',
                          step.status === 'success'
                            ? 'bg-green-400'
                            : step.status === 'failed'
                              ? 'bg-red-400'
                              : step.status === 'running'
                                ? 'bg-amber-400 animate-pulse'
                                : 'bg-gray-500'
                        )}
                      />
                      <span
                        className={cn(
                          step.status === 'success'
                            ? 'text-green-400/80'
                            : step.status === 'failed'
                              ? 'text-red-400/80'
                              : step.status === 'running'
                                ? 'text-amber-400/80'
                                : 'text-gray-500'
                        )}
                      >
                        {step.name}
                        {step.detail && <span className="ml-1 opacity-70">— {step.detail}</span>}
                      </span>
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}

        {/* 能力标签 */}
        {'capabilities' in model && model.capabilities && (
          <div className="flex flex-wrap gap-2 mt-4">
            {model.capabilities.map(cap => (
              <span key={cap} className="px-2 py-1 bg-cinema-800 text-gray-400 text-xs rounded-lg">
                {cap}
              </span>
            ))}
          </div>
        )}

        {'dimensions' in model && (
          <div className="mt-4 text-sm text-gray-500">
            维度: {model.dimensions} · 最大输入: {model.max_input_tokens} tokens
          </div>
        )}
      </CardContent>
    </Card>
  );
}
