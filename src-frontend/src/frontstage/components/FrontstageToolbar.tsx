/**
 * FrontstageToolbar - 幕前界面工具栏
 * 
 * 极简设计，仅保留必要功能
 */

import { useState } from 'react';
import { Minimize2, Maximize2, X, Eye } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';

interface FrontstageToolbarProps {
  chapterTitle?: string;
  storyId?: string;
  onRequestGeneration: (context: string) => void;
}

export function FrontstageToolbar({ chapterTitle, storyId, onRequestGeneration }: FrontstageToolbarProps) {
  const [isCompact, setIsCompact] = useState(false);
  const [showSettings, setShowSettings] = useState(false);

  const handleClose = async () => {
    try {
      await invoke('hide_frontstage');
    } catch (error) {
      console.error('Failed to hide frontstage:', error);
    }
  };

  const handleToggleBackstage = async () => {
    try {
      await invoke('show_backstage', { story_id: storyId || null });
    } catch (error) {
      console.error('Failed to show backstage:', error);
    }
  };

  return (
    <header className={`frontstage-toolbar ${isCompact ? 'compact' : ''}`}>
      <div className="toolbar-left">
        <div className="logo-mark">
          <span className="logo-icon">草</span>
        </div>
        
        {!isCompact && (
          <div className="chapter-info">
            <span className="chapter-label">当前章节</span>
            <span className="chapter-title">{chapterTitle || '未命名章节'}</span>
          </div>
        )}
      </div>

      <div className="toolbar-right">
        <button
          className="toolbar-btn"
          onClick={handleToggleBackstage}
          title="打开幕后工作界面"
        >
          <Eye className="btn-icon" />
          {!isCompact && <span>幕后</span>}
        </button>

        <button
          className="toolbar-btn"
          onClick={() => setIsCompact(!isCompact)}
          title={isCompact ? '展开' : '收起'}
        >
          {isCompact ? <Maximize2 className="btn-icon" /> : <Minimize2 className="btn-icon" />}
        </button>

        <button
          className="toolbar-btn close-btn"
          onClick={handleClose}
          title="隐藏幕前界面"
        >
          <X className="btn-icon" />
        </button>
      </div>

      {showSettings && (
        <div className="toolbar-settings-popup">
          <div className="settings-item">
            <label>AI 提示频率</label>
            <select>
              <option>低</option>
              <option selected>中</option>
              <option>高</option>
            </select>
          </div>
        </div>
      )}
    </header>
  );
}