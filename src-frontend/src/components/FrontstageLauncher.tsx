import { useState, useEffect } from 'react';
import { BookOpen, ExternalLink } from 'lucide-react';
import { loggedInvoke } from '@/services/tauri';
import { useAppStore } from '@/stores/appStore';
import { createLogger } from '@/utils/logger';
import { cn } from '@/utils/cn';
import toast from 'react-hot-toast';

const launcherLogger = createLogger('ui:FrontstageLauncher');

interface FrontstageLauncherProps {
  isOpen?: boolean;
  onToggle?: () => void;
  variant?: 'button' | 'icon';
}

export function FrontstageLauncher({
  isOpen = false,
  onToggle,
  variant = 'button',
}: FrontstageLauncherProps) {
  const [isVisible, setIsVisible] = useState(false);
  const currentStory = useAppStore(s => s.currentStory);

  useEffect(() => {
    const checkTauri = async () => {
      try {
        await loggedInvoke<unknown>('get_window_state');
        setIsVisible(true);
      } catch {
        setIsVisible(false);
      }
    };
    checkTauri();
  }, []);

  const handleOpenFrontstage = async () => {
    try {
      if (currentStory) {
        await loggedInvoke<unknown>('open_story_in_frontstage', {
          story_id: currentStory.id,
          title: currentStory.title,
        });
      } else {
        await loggedInvoke<unknown>('show_frontstage');
      }
      toast.success('幕前写作界面已打开');
      onToggle?.();
    } catch (error) {
      launcherLogger.error('Failed to open frontstage', { error });
      toast.error('无法打开幕前界面');
    }
  };

  if (!isVisible) return null;

  const label = isOpen ? '幕前已开启' : '开幕前';

  return (
    <div className={cn(variant === 'icon' ? 'w-full' : 'flex items-center gap-2')}>
      <button
        type="button"
        aria-label={label}
        title={label}
        onClick={handleOpenFrontstage}
        className={cn(
          'flex items-center justify-center gap-2 font-medium transition-colors',
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cinema-gold/50',
          variant === 'icon'
            ? 'aspect-square w-full rounded-panel text-ai-ink-2 hover:bg-cinema-800/60 hover:text-cinema-gold'
            : 'rounded-lg border border-cinema-700 bg-cinema-800 px-4 py-2 text-sm text-ai-ink hover:bg-cinema-700'
        )}
      >
        <BookOpen className="h-5 w-5 shrink-0" />
        {variant === 'button' && (
          <>
            <span>{label}</span>
            <ExternalLink className="h-3.5 w-3.5 opacity-60" />
          </>
        )}
      </button>
    </div>
  );
}
