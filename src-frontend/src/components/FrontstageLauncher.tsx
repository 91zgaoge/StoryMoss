/**
 * FrontstageLauncher - 幕前窗口启动器
 *
 * 提供快速打开/关闭幕前窗口的按钮
 */

import { useState, useEffect } from 'react';
import { BookOpen, ExternalLink } from 'lucide-react';
import { loggedInvoke } from '@/services/tauri';
import { createLogger } from '@/utils/logger';
import toast from 'react-hot-toast';

const launcherLogger = createLogger('ui:FrontstageLauncher');

interface FrontstageLauncherProps {
  isOpen: boolean;
  onToggle: () => void;
}

export function FrontstageLauncher({ isOpen, onToggle }: FrontstageLauncherProps) {
  const [isVisible, setIsVisible] = useState(false);

  useEffect(() => {
    // Check if Tauri API is available
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
      await loggedInvoke<unknown>('show_frontstage');
      toast.success('幕前写作界面已打开');
      onToggle();
    } catch (error) {
      launcherLogger.error('Failed to open frontstage', { error });
      toast.error('无法打开幕前界面');
    }
  };

  if (!isVisible) return null;

  return (
    <div className="fixed top-4 right-4 z-50 flex items-center gap-2">
      <button
        onClick={handleOpenFrontstage}
        className={`
          flex items-center gap-2 px-4 py-2 rounded-xl font-medium text-sm
          transition-all duration-300 shadow-lg
          ${
            isOpen
              ? 'bg-cinema-gold/20 text-cinema-gold border border-cinema-gold/30'
              : 'bg-cinema-800 text-white hover:bg-cinema-700 border border-cinema-700'
          }
        `}
      >
        <BookOpen className="w-4 h-4" />
        <span>{isOpen ? '幕前已开启' : '开幕前'}</span>
        <ExternalLink className="w-3 h-3 opacity-60" />
      </button>
    </div>
  );
}
