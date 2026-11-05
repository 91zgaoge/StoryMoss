import { useState } from 'react';
import { Route, Play, AlertCircle, CheckCircle2, Clock, DollarSign, Cpu } from 'lucide-react';
import { Card, CardContent } from '@/components/ui/Card';
import { Input, Select } from '@/components/ui/controls';
import { Button } from '@/components/ui/Button';
import { useSimulateRoute } from '@/hooks/useRouter';
import type { Complexity, Priority, RoutingDecision, RoutingRequest, TaskType } from '@/types/llm';
import { cn } from '@/utils/cn';

const TASK_OPTIONS: { value: TaskType; label: string }[] = [
  { value: 'creative_writing', label: '创意写作' },
  { value: 'editing', label: '编辑/改写' },
  { value: 'analysis', label: '分析/推理' },
  { value: 'dialogue', label: '对话/角色声音' },
  { value: 'summarization', label: '摘要' },
  { value: 'brainstorming', label: '头脑风暴' },
  { value: 'proofreading', label: '校对' },
  { value: 'world_building', label: '世界观构建' },
  { value: 'vision', label: '多模态视觉' },
  { value: 'image_generation', label: '图像生成' },
];

const COMPLEXITY_OPTIONS: { value: Complexity; label: string }[] = [
  { value: 'low', label: '低' },
  { value: 'medium', label: '中' },
  { value: 'high', label: '高' },
  { value: 'critical', label: '关键' },
];

const PRIORITY_OPTIONS: { value: Priority; label: string }[] = [
  { value: 'low', label: '低' },
  { value: 'medium', label: '中' },
  { value: 'high', label: '高' },
];

export function RouteSimulator() {
  const [task, setTask] = useState<TaskType>('creative_writing');
  const [complexity, setComplexity] = useState<Complexity>('medium');
  const [budgetPriority, setBudgetPriority] = useState<Priority>('low');
  const [speedPriority, setSpeedPriority] = useState<Priority>('low');
  const [estimatedTokens, setEstimatedTokens] = useState<number>(0);
  const [result, setResult] = useState<RoutingDecision | null>(null);

  const simulate = useSimulateRoute();

  const handleSimulate = () => {
    const request: RoutingRequest = {
      task,
      complexity,
      budget_priority: budgetPriority,
      speed_priority: speedPriority,
      estimated_input_tokens: estimatedTokens,
      constraints: [],
    };
    simulate.mutate(request, {
      onSuccess: setResult,
    });
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-3">
        <Route className="w-6 h-6 text-cinema-gold" />
        <div>
          <h2 className="text-xl font-semibold text-white">路由模拟器</h2>
        </div>
      </div>

      <Card>
        <CardContent className="p-6 space-y-6">
          <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
            <SelectField label="任务类型" value={task} onChange={setTask} options={TASK_OPTIONS} />
            <SelectField
              label="任务复杂度"
              value={complexity}
              onChange={setComplexity}
              options={COMPLEXITY_OPTIONS}
            />
            <SelectField
              label="成本优先级"
              value={budgetPriority}
              onChange={setBudgetPriority}
              options={PRIORITY_OPTIONS}
            />
            <SelectField
              label="速度优先级"
              value={speedPriority}
              onChange={setSpeedPriority}
              options={PRIORITY_OPTIONS}
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-300 mb-2">
              预计输入 Token 数
            </label>
            <Input
              type="number"
              min={0}
              step={256}
              value={estimatedTokens}
              onChange={e => setEstimatedTokens(Number(e.target.value))}
              className="md:w-64"
            />
            <p className="text-xs text-gray-500 mt-1">用于过滤上下文长度不足的模型，0 表示不限制</p>
          </div>

          <Button
            variant="primary"
            onClick={handleSimulate}
            isLoading={simulate.isPending}
            disabled={simulate.isPending}
          >
            <Play className="w-4 h-4 mr-2" />
            运行路由模拟
          </Button>
        </CardContent>
      </Card>

      {simulate.isError && (
        <Card className="border-red-900/50">
          <CardContent className="p-4 flex items-start gap-3">
            <AlertCircle className="w-5 h-5 text-red-400 mt-0.5" />
            <div>
              <h3 className="text-sm font-medium text-red-200">路由失败</h3>
              <p className="text-sm text-red-300 mt-1">
                {simulate.error instanceof Error
                  ? simulate.error.message
                  : '无法找到满足条件的可用模型'}
              </p>
            </div>
          </CardContent>
        </Card>
      )}

      {result && (
        <Card className="border-cinema-gold/30">
          <CardContent className="p-6 space-y-4">
            <div className="flex items-center gap-3">
              <CheckCircle2 className="w-5 h-5 text-green-400" />
              <h3 className="text-lg font-medium text-white">路由决策结果</h3>
            </div>

            <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
              <ResultItem
                icon={<Cpu className="w-4 h-4" />}
                label="选中模型"
                value={result.model_name}
              />
              <ResultItem
                icon={<DollarSign className="w-4 h-4" />}
                label="预估成本"
                value={
                  result.estimated_cost > 0
                    ? `$${result.estimated_cost.toFixed(4)} / 1K output`
                    : '免费/未配置'
                }
              />
              <ResultItem
                icon={<Clock className="w-4 h-4" />}
                label="预估耗时"
                value={`${result.estimated_time_ms} ms`}
              />
            </div>

            <div className="bg-cinema-900/50 rounded-lg p-4">
              <p className="text-sm text-gray-400 mb-1">决策理由</p>
              <p className="text-sm text-gray-200">{result.reason}</p>
            </div>
          </CardContent>
        </Card>
      )}
    </div>
  );
}

function SelectField<T extends string>({
  label,
  value,
  onChange,
  options,
}: {
  label: string;
  value: T;
  onChange: (value: T) => void;
  options: { value: T; label: string }[];
}) {
  return (
    <div>
      <label className="block text-sm font-medium text-gray-300 mb-2">{label}</label>
      <Select value={value} onChange={e => onChange(e.target.value as T)}>
        {options.map(opt => (
          <option key={opt.value} value={opt.value}>
            {opt.label}
          </option>
        ))}
      </Select>
    </div>
  );
}

function ResultItem({
  icon,
  label,
  value,
}: {
  icon: React.ReactNode;
  label: string;
  value: string;
}) {
  return (
    <div className="bg-cinema-900/50 rounded-lg p-4">
      <div className="flex items-center gap-2 text-cinema-gold mb-1">
        {icon}
        <span className="text-xs font-medium uppercase tracking-wider">{label}</span>
      </div>
      <p className={cn('text-white font-medium', value.length > 30 && 'text-sm')}>{value}</p>
    </div>
  );
}
