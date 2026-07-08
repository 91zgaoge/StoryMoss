import { Sparkles, Wand2, Zap, ArrowRight } from 'lucide-react';
import { Card, CardContent } from '@/components/ui/Card';

interface PathItem {
  id: string;
  icon: React.ElementType;
  title: string;
  subtitle: string;
  description: string;
  badge?: string;
}

const paths: PathItem[] = [
  {
    id: 'frontstage',
    icon: Sparkles,
    title: '幕前 Genesis Pipeline',
    subtitle: 'smart_execute',
    description: '智能创作流程-创世。推荐新用户：30–60 秒生成开篇，后台自动完成策略、世界、场景、伏笔、知识图谱与合同播种。',
    badge: '推荐',
  },
  {
    id: 'wizard',
    icon: Wand2,
    title: '幕后 AI 向导',
    subtitle: 'NovelCreationWizard',
    description: '分步选择世界观、角色、文风与首场景，将预置资产写入已有故事。适合希望对创作要素做显式决策的用户。',
  },
  {
    id: 'quick',
    icon: Zap,
    title: '幕后快速创作',
    subtitle: 'runCreationWorkflow',
    description: '多阶段创作引擎，按选定模式（AI 全自动 / AI 初稿 + 精修 / 我初稿 + AI 润色）快速产出内容。',
  },
];

export function CreationPathGuide() {
  return (
    <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
      {paths.map(path => {
        const Icon = path.icon;
        return (
          <Card key={path.id} className="bg-cinema-800/50 border-cinema-700">
            <CardContent className="p-4">
              <div className="flex items-start gap-3">
                <div className="p-2 rounded-lg bg-cinema-900 text-cinema-gold">
                  <Icon className="w-5 h-5" />
                </div>
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2 flex-wrap">
                    <h3 className="font-display font-semibold text-white text-sm">{path.title}</h3>
                    {path.badge && (
                      <span className="text-[10px] px-1.5 py-0.5 rounded bg-cinema-gold/20 text-cinema-gold border border-cinema-gold/20">
                        {path.badge}
                      </span>
                    )}
                  </div>
                  <p className="text-[10px] text-gray-500 font-mono mt-0.5">{path.subtitle}</p>
                  <p className="text-xs text-gray-400 mt-2 leading-relaxed">{path.description}</p>
                </div>
              </div>
            </CardContent>
          </Card>
        );
      })}
    </div>
  );
}

export function CreationPathGuideCompact({ onFrontstage }: { onFrontstage?: () => void }) {
  return (
    <div className="p-3 rounded-xl bg-cinema-800/50 border border-cinema-700 space-y-2">
      <p className="text-xs text-gray-400 flex items-center gap-1.5">
        <ArrowRight className="w-3 h-3 text-cinema-gold" />
        三条创作路径说明
      </p>
      <div className="space-y-1.5">
        {paths.map(path => {
          const Icon = path.icon;
          return (
            <div key={path.id} className="flex items-center gap-2 text-xs">
              <Icon className="w-3.5 h-3.5 text-cinema-gold" />
              <span className="text-white font-medium">{path.title}</span>
              <span className="text-gray-500 truncate">— {path.description.slice(0, 24)}...</span>
            </div>
          );
        })}
      </div>
      {onFrontstage && (
        <button
          onClick={onFrontstage}
          className="w-full mt-1 text-xs text-cinema-gold hover:text-cinema-gold/80 flex items-center justify-center gap-1 py-1"
        >
          去幕前走 Genesis Pipeline
          <ArrowRight className="w-3 h-3" />
        </button>
      )}
    </div>
  );
}
