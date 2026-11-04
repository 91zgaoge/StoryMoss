import { useMemo, useState } from 'react';
import { Bot, SlidersHorizontal, X } from 'lucide-react';
import { Card, CardContent } from '@/components/ui/Card';
import { useSettingsContext } from '@/hooks/useSettingsContext';
import type { AgentModelMapping, ModelConfig, TaskType, Complexity, Priority } from '@/types/llm';

const ALL_AGENTS: AgentModelMapping[] = [
  {
    agent_id: 'writer',
    agent_name: '写作助手',
    description: '根据上下文生成或改写章节内容',
  },
  {
    agent_id: 'inspector',
    agent_name: '质检员',
    description: '检查内容质量、逻辑连贯性、人物一致性',
  },
  {
    agent_id: 'outline_planner',
    agent_name: '大纲规划师',
    description: '设计故事大纲、章节结构',
  },
  {
    agent_id: 'style_mimic',
    agent_name: '风格模仿师',
    description: '分析并模仿特定文风',
  },
  {
    agent_id: 'plot_analyzer',
    agent_name: '情节分析师',
    description: '分析情节复杂度、检测漏洞',
  },
  {
    agent_id: 'memory_compressor',
    agent_name: '记忆压缩师',
    description: '将详细内容压缩为高层记忆摘要',
  },
  {
    agent_id: 'commentator',
    agent_name: '古典评点家',
    description: '以金圣叹风格对小说段落进行实时文学点评',
  },
  {
    agent_id: 'knowledge_distiller',
    agent_name: '知识蒸馏师',
    description: '将知识图谱蒸馏为高层故事摘要与世界观总结',
  },
];

function useModelOptions(type: ModelConfig['type']) {
  const { models } = useSettingsContext();
  return useMemo(() => {
    const filtered = models.filter(m => m.type === type);
    return [
      {
        id: '',
        name: `使用默认${type === 'chat' ? '聊天' : type === 'embedding' ? '嵌入' : '多模态'}模型`,
      },
      ...filtered.map(m => ({ id: m.id, name: m.name })),
    ];
  }, [models, type]);
}

function ModelSelect({
  value,
  options,
  onChange,
  disabled,
}: {
  value?: string;
  options: { id: string; name: string }[];
  onChange: (value: string) => void;
  disabled?: boolean;
}) {
  return (
    <select
      value={value || ''}
      onChange={e => onChange(e.target.value)}
      disabled={disabled}
      className="w-full bg-cinema-800 border border-cinema-700 rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:border-cinema-gold disabled:opacity-50"
    >
      {options.map(opt => (
        <option key={opt.id} value={opt.id}>
          {opt.name}
        </option>
      ))}
    </select>
  );
}

const TASK_TYPE_OPTIONS: { value: TaskType; label: string }[] = [
  { value: 'creative_writing', label: '创意写作' },
  { value: 'editing', label: '编辑改写' },
  { value: 'analysis', label: '分析推理' },
  { value: 'dialogue', label: '对话声音' },
  { value: 'summarization', label: '摘要压缩' },
  { value: 'brainstorming', label: '头脑风暴' },
  { value: 'proofreading', label: '校对错漏' },
  { value: 'world_building', label: '世界观构建' },
  { value: 'vision', label: '视觉理解' },
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

function PolicySelect<T extends string>({
  value,
  options,
  onChange,
  disabled,
  placeholder,
}: {
  value?: T;
  options: { value: T; label: string }[];
  onChange: (value: T | undefined) => void;
  disabled?: boolean;
  placeholder?: string;
}) {
  return (
    <select
      value={value || ''}
      onChange={e => onChange((e.target.value as T) || undefined)}
      disabled={disabled}
      className="w-full bg-cinema-800 border border-cinema-700 rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:border-cinema-gold disabled:opacity-50"
    >
      <option value="">{placeholder || '使用默认'}</option>
      {options.map(opt => (
        <option key={opt.value} value={opt.value}>
          {opt.label}
        </option>
      ))}
    </select>
  );
}

export function AgentConfig() {
  const {
    agentMappings: mappings,
    isLoading: mappingsLoading,
    updateAgentMapping,
  } = useSettingsContext();
  const chatOptions = useModelOptions('chat');
  const embeddingOptions = useModelOptions('embedding');
  const multimodalOptions = useModelOptions('multimodal');
  const [expandedAgent, setExpandedAgent] = useState<string | null>(null);

  const mergedAgents = useMemo(() => {
    const map = new Map(mappings.map(m => [m.agent_id, m]));
    return ALL_AGENTS.map(defaultAgent => {
      const existing = map.get(defaultAgent.agent_id);
      return {
        ...defaultAgent,
        chat_model_id: existing?.chat_model_id || '',
        embedding_model_id: existing?.embedding_model_id || '',
        multimodal_model_id: existing?.multimodal_model_id || '',
        task_type: existing?.task_type,
        complexity: existing?.complexity,
        budget_priority: existing?.budget_priority,
        speed_priority: existing?.speed_priority,
        constraints: existing?.constraints || [],
      };
    });
  }, [mappings]);

  const handleChange = (
    agent: AgentModelMapping,
    field: keyof AgentModelMapping,
    value: string | string[] | boolean | undefined
  ) => {
    updateAgentMapping({
      ...agent,
      [field]:
        value === undefined || (typeof value === 'string' && value === '') ? undefined : value,
    });
  };

  return (
    <Card>
      <CardContent className="p-6 space-y-6">
        <div className="flex items-start gap-4">
          <div className="p-3 bg-cinema-gold/10 rounded-xl">
            <Bot className="w-6 h-6 text-cinema-gold" />
          </div>
          <div>
            <h3 className="text-lg font-medium text-white">Agent 模型映射</h3>
            <p className="text-sm text-gray-400 mt-1">未指定时回退默认模型与内置策略。</p>
          </div>
        </div>

        {mappingsLoading ? (
          <div className="text-center py-12 text-gray-500">加载中...</div>
        ) : (
          <div className="space-y-4">
            {mergedAgents.map(agent => {
              const isExpanded = expandedAgent === agent.agent_id;
              return (
                <div
                  key={agent.agent_id}
                  className="border border-cinema-800 rounded-xl overflow-hidden"
                >
                  <div className="p-4 flex items-center justify-between gap-4 bg-cinema-900/50">
                    <div>
                      <div className="text-sm font-medium text-white flex items-center gap-2">
                        {agent.agent_name}
                        {agent.task_type && (
                          <span className="px-1.5 py-0.5 rounded text-[10px] bg-cinema-gold/20 text-cinema-gold">
                            {TASK_TYPE_OPTIONS.find(t => t.value === agent.task_type)?.label ||
                              agent.task_type}
                          </span>
                        )}
                      </div>
                      <div className="text-xs text-gray-500 mt-0.5">{agent.description}</div>
                    </div>
                    <button
                      onClick={() => setExpandedAgent(isExpanded ? null : agent.agent_id)}
                      className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-cinema-gold hover:bg-cinema-gold/10 rounded-lg transition-colors"
                    >
                      <SlidersHorizontal className="w-3.5 h-3.5" />
                      {isExpanded ? '收起策略' : '任务策略'}
                    </button>
                  </div>

                  <div className="p-4 grid grid-cols-1 md:grid-cols-3 gap-4 border-t border-cinema-800/50">
                    <div>
                      <label className="block text-xs text-gray-400 mb-1.5">聊天模型</label>
                      <ModelSelect
                        value={agent.chat_model_id}
                        options={chatOptions}
                        onChange={value => handleChange(agent, 'chat_model_id', value)}
                      />
                    </div>
                    <div>
                      <label className="block text-xs text-gray-400 mb-1.5">嵌入模型</label>
                      <ModelSelect
                        value={agent.embedding_model_id}
                        options={embeddingOptions}
                        onChange={value => handleChange(agent, 'embedding_model_id', value)}
                      />
                    </div>
                    <div>
                      <label className="block text-xs text-gray-400 mb-1.5">多模态模型</label>
                      <ModelSelect
                        value={agent.multimodal_model_id}
                        options={multimodalOptions}
                        onChange={value => handleChange(agent, 'multimodal_model_id', value)}
                      />
                    </div>
                  </div>

                  {/* v0.15.0: 强制锁定模型，网关不会根据算力档案替换 */}
                  <div className="px-4 pb-3">
                    <label className="flex items-center gap-2 cursor-pointer text-sm text-gray-400">
                      <input
                        type="checkbox"
                        checked={agent.force_locked || false}
                        onChange={e => handleChange(agent, 'force_locked', e.target.checked)}
                        className="rounded border-cinema-600 bg-cinema-800"
                      />
                      🔒 强制锁定此模型（网关不会自动替换为更快模型）
                    </label>
                    {agent.force_locked && (
                      <p className="text-xs text-cinema-gold/70 mt-1.5 ml-6">
                        已锁定：网关将始终使用你指定的模型，跳过智能调度
                      </p>
                    )}
                  </div>

                  {isExpanded && (
                    <div className="p-4 border-t border-cinema-800/50 bg-cinema-900/30 space-y-4">
                      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
                        <div>
                          <label className="block text-xs text-gray-400 mb-1.5">任务类型</label>
                          <PolicySelect
                            value={agent.task_type}
                            options={TASK_TYPE_OPTIONS}
                            onChange={value => handleChange(agent, 'task_type', value)}
                            placeholder="使用 Agent 默认"
                          />
                        </div>
                        <div>
                          <label className="block text-xs text-gray-400 mb-1.5">复杂度</label>
                          <PolicySelect
                            value={agent.complexity}
                            options={COMPLEXITY_OPTIONS}
                            onChange={value => handleChange(agent, 'complexity', value)}
                            placeholder="使用 Agent 默认"
                          />
                        </div>
                        <div>
                          <label className="block text-xs text-gray-400 mb-1.5">成本优先级</label>
                          <PolicySelect
                            value={agent.budget_priority}
                            options={PRIORITY_OPTIONS}
                            onChange={value => handleChange(agent, 'budget_priority', value)}
                            placeholder="使用 Agent 默认"
                          />
                        </div>
                        <div>
                          <label className="block text-xs text-gray-400 mb-1.5">速度优先级</label>
                          <PolicySelect
                            value={agent.speed_priority}
                            options={PRIORITY_OPTIONS}
                            onChange={value => handleChange(agent, 'speed_priority', value)}
                            placeholder="使用 Agent 默认"
                          />
                        </div>
                      </div>

                      <div>
                        <label className="block text-xs text-gray-400 mb-1.5">约束标签</label>
                        <ConstraintInput
                          constraints={agent.constraints || []}
                          onChange={values => handleChange(agent, 'constraints', values)}
                        />
                      </div>
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </CardContent>
    </Card>
  );
}

function ConstraintInput({
  constraints,
  onChange,
  disabled,
}: {
  constraints: string[];
  onChange: (values: string[]) => void;
  disabled?: boolean;
}) {
  const [input, setInput] = useState('');

  const addConstraint = () => {
    const trimmed = input.trim();
    if (!trimmed || constraints.includes(trimmed)) return;
    onChange([...constraints, trimmed]);
    setInput('');
  };

  const removeConstraint = (idx: number) => {
    onChange(constraints.filter((_, i) => i !== idx));
  };

  return (
    <div className="space-y-2">
      <div className="flex gap-2">
        <input
          type="text"
          value={input}
          onChange={e => setInput(e.target.value)}
          onKeyDown={e => {
            if (e.key === 'Enter') {
              e.preventDefault();
              addConstraint();
            }
          }}
          disabled={disabled}
          placeholder="例如 local_only、min_quality:high、requires:long_context"
          className="flex-1 bg-cinema-800 border border-cinema-700 rounded-lg px-3 py-2 text-sm text-white placeholder-gray-600 focus:outline-none focus:border-cinema-gold disabled:opacity-50"
        />
        <button
          type="button"
          onClick={addConstraint}
          disabled={disabled || !input.trim()}
          className="px-3 py-2 text-sm bg-cinema-800 border border-cinema-700 rounded-lg text-cinema-gold hover:bg-cinema-gold/10 disabled:opacity-50"
        >
          添加
        </button>
      </div>
      <div className="flex flex-wrap gap-2">
        {constraints.map((c, idx) => (
          <span
            key={`${c}-${idx}`}
            className="inline-flex items-center gap-1 px-2 py-1 rounded-md text-xs bg-cinema-800 text-gray-300 border border-cinema-700"
          >
            {c}
            <button
              type="button"
              onClick={() => removeConstraint(idx)}
              disabled={disabled}
              className="hover:text-cinema-gold disabled:opacity-50"
            >
              <X className="w-3 h-3" />
            </button>
          </span>
        ))}
      </div>
    </div>
  );
}
