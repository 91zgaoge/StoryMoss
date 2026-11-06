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

interface CreationPathGuideProps {
  onFrontstage?: () => void;
  onWizard?: () => void;
  onQuick?: () => void;
}

const paths: PathItem[] = [
  {
    id: 'frontstage',
    icon: Sparkles,
    title: '幕前 Genesis Pipeline',
    subtitle: 'smart_execute',
    description: '30–60 秒生成开篇，设定自动在后台完善',
    badge: '推荐',
  },
  {
    id: 'wizard',
    icon: Wand2,
    title: '幕后 AI 向导',
    subtitle: 'NovelCreationWizard',
    description: '分步决定世界观、角色、文风与首场景',
  },
  {
    id: 'quick',
    icon: Zap,
    title: '幕后快速创作',
    subtitle: 'runCreationWorkflow',
    description: 'AI 全自动 / 初稿精修 / 初稿润色，按所选模式产出',
  },
];

const callbackMap: Record<string, keyof CreationPathGuideProps> = {
  frontstage: 'onFrontstage',
  wizard: 'onWizard',
  quick: 'onQuick',
};

export function CreationPathGuide({ onFrontstage, onWizard, onQuick }: CreationPathGuideProps) {
  const callbacks = { onFrontstage, onWizard, onQuick };

  return (
    <div className="@container">
      <div className="grid grid-cols-1 gap-3 @2xl:grid-cols-3">
        {paths.map(path => {
          const Icon = path.icon;
          const callbackName = callbackMap[path.id];
          const callback = callbackName ? callbacks[callbackName] : undefined;
          return (
            <Card
              key={path.id}
              hover
              className={`group h-full border-borderSubtle bg-cinema-900/45 ${callback ? 'cursor-pointer' : ''}`}
              onClick={callback}
              role={callback ? 'button' : undefined}
              tabIndex={callback ? 0 : undefined}
              onKeyDown={
                callback
                  ? event => {
                      if (event.key === 'Enter' || event.key === ' ') {
                        event.preventDefault();
                        callback();
                      }
                    }
                  : undefined
              }
              aria-label={callback ? path.title : undefined}
            >
              <CardContent className="p-4">
                <div className="flex items-start gap-3">
                  <div className="shrink-0 rounded-lg border border-cinema-gold/15 bg-cinema-gold/10 p-2.5 text-cinema-gold">
                    <Icon className="h-5 w-5" />
                  </div>
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 flex-wrap">
                      <h3 className="font-body text-sm font-semibold leading-6 text-ai-ink">
                        {path.title}
                      </h3>
                      {path.badge && (
                        <span className="text-[10px] px-1.5 py-0.5 rounded bg-cinema-gold/20 text-cinema-gold border border-cinema-gold/20">
                          {path.badge}
                        </span>
                      )}
                    </div>
                    <p className="mt-0.5 font-mono text-xs text-ai-ink-3">{path.subtitle}</p>
                    <p className="mt-2 text-xs leading-5 text-ai-ink-2">{path.description}</p>
                  </div>
                </div>
              </CardContent>
            </Card>
          );
        })}
      </div>
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
