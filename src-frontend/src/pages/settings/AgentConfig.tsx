import { useMemo } from 'react';
import { Bot } from 'lucide-react';
import { Card, CardContent } from '@/components/ui/Card';
import { useAgentMappings, useUpdateAgentMapping, useModels } from '@/hooks/useSettings';
import type { AgentModelMapping, ModelConfig } from '@/types/llm';

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
  const { data: models = [] } = useModels();
  return useMemo(() => {
    const filtered = models.filter(m => m.type === type);
    return [
      { id: '', name: `使用默认${type === 'chat' ? '聊天' : type === 'embedding' ? '嵌入' : '多模态'}模型` },
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

export function AgentConfig() {
  const { data: mappings = [], isLoading: mappingsLoading } = useAgentMappings();
  const updateMapping = useUpdateAgentMapping();
  const chatOptions = useModelOptions('chat');
  const embeddingOptions = useModelOptions('embedding');
  const multimodalOptions = useModelOptions('multimodal');

  const mergedAgents = useMemo(() => {
    const map = new Map(mappings.map(m => [m.agent_id, m]));
    return ALL_AGENTS.map(defaultAgent => {
      const existing = map.get(defaultAgent.agent_id);
      return {
        ...defaultAgent,
        chat_model_id: existing?.chat_model_id || '',
        embedding_model_id: existing?.embedding_model_id || '',
        multimodal_model_id: existing?.multimodal_model_id || '',
      };
    });
  }, [mappings]);

  const handleChange = (agent: AgentModelMapping, field: keyof AgentModelMapping, value: string) => {
    updateMapping.mutate({
      ...agent,
      [field]: value || undefined,
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
            <p className="text-sm text-gray-400 mt-1">
              为不同创作 Agent 指定专用模型。未指定时回退到默认模型。
            </p>
          </div>
        </div>

        {mappingsLoading ? (
          <div className="text-center py-12 text-gray-500">加载中...</div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="border-b border-cinema-800">
                  <th className="text-left py-3 px-4 text-sm font-medium text-gray-400">Agent</th>
                  <th className="text-left py-3 px-4 text-sm font-medium text-gray-400">聊天模型</th>
                  <th className="text-left py-3 px-4 text-sm font-medium text-gray-400">嵌入模型</th>
                  <th className="text-left py-3 px-4 text-sm font-medium text-gray-400">多模态模型</th>
                </tr>
              </thead>
              <tbody>
                {mergedAgents.map(agent => (
                  <tr key={agent.agent_id} className="border-b border-cinema-800/50 last:border-0">
                    <td className="py-4 px-4">
                      <div className="text-sm font-medium text-white">{agent.agent_name}</div>
                      <div className="text-xs text-gray-500 mt-0.5">{agent.description}</div>
                    </td>
                    <td className="py-4 px-4 min-w-[200px]">
                      <ModelSelect
                        value={agent.chat_model_id}
                        options={chatOptions}
                        onChange={value => handleChange(agent, 'chat_model_id', value)}
                        disabled={updateMapping.isPending}
                      />
                    </td>
                    <td className="py-4 px-4 min-w-[200px]">
                      <ModelSelect
                        value={agent.embedding_model_id}
                        options={embeddingOptions}
                        onChange={value => handleChange(agent, 'embedding_model_id', value)}
                        disabled={updateMapping.isPending}
                      />
                    </td>
                    <td className="py-4 px-4 min-w-[200px]">
                      <ModelSelect
                        value={agent.multimodal_model_id}
                        options={multimodalOptions}
                        onChange={value => handleChange(agent, 'multimodal_model_id', value)}
                        disabled={updateMapping.isPending}
                      />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
