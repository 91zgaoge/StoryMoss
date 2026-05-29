import { useRef, useEffect, useState } from 'react';
import Editor from '@monaco-editor/react';
import { Maximize2, Minimize2, Type, Save } from 'lucide-react';
import { Button } from './ui/Button';

interface MonacoEditorProps {
  value: string;
  onChange: (value: string) => void;
  onSave?: () => void;
  placeholder?: string;
  readOnly?: boolean;
}

export function MonacoEditor({
  value,
  onChange,
  onSave,
  placeholder = '开始写作...',
  readOnly = false,
}: MonacoEditorProps) {
  const editorRef = useRef<any>(null);
  const [isFullscreen, setIsFullscreen] = useState(false);
  const [fontSize, setFontSize] = useState(16);

  // Handle keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 's') {
        e.preventDefault();
        onSave?.();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [onSave]);

  const handleEditorDidMount = (editor: any) => {
    editorRef.current = editor;
    editor.focus();
  };

  const handleEditorChange = (newValue: string | undefined) => {
    onChange(newValue || '');
  };

  const toggleFullscreen = () => {
    setIsFullscreen(!isFullscreen);
  };

  const increaseFontSize = () => setFontSize(s => Math.min(s + 2, 32));
  const decreaseFontSize = () => setFontSize(s => Math.max(s - 2, 12));

  return (
    <div
      className={`flex flex-col ${isFullscreen ? 'fixed inset-0 z-50 bg-cinema-950' : 'h-full'}`}
    >
      {/* Toolbar */}
      <div className="flex items-center justify-between px-4 py-2 bg-cinema-900 border-b border-cinema-800">
        <div className="flex items-center gap-2">
          <span className="text-sm text-gray-400">{value.length} 字符</span>
        </div>

        <div className="flex items-center gap-2">
          <Button variant="ghost" size="sm" onClick={decreaseFontSize} title="减小字体">
            <Type className="w-4 h-4" />
          </Button>
          <span className="text-sm text-gray-400 w-8 text-center">{fontSize}px</span>
          <Button variant="ghost" size="sm" onClick={increaseFontSize} title="增大字体">
            <Type className="w-4 h-4" />
          </Button>

          {onSave && (
            <Button variant="ghost" size="sm" onClick={onSave} title="保存 (Ctrl+S)">
              <Save className="w-4 h-4" />
            </Button>
          )}

          <Button
            variant="ghost"
            size="sm"
            onClick={toggleFullscreen}
            title={isFullscreen ? '退出全屏' : '全屏'}
          >
            {isFullscreen ? <Minimize2 className="w-4 h-4" /> : <Maximize2 className="w-4 h-4" />}
          </Button>
        </div>
      </div>

      {/* Editor */}
      <div className="flex-1">
        <Editor
          height="100%"
          defaultLanguage="markdown"
          value={value}
          onChange={handleEditorChange}
          onMount={handleEditorDidMount}
          options={{
            readOnly,
            fontSize,
            fontFamily: "'LXGW WenKai', 'Noto Serif SC', 'PingFang SC', serif",
            lineNumbers: 'on',
            wordWrap: 'on',
            minimap: { enabled: false },
            scrollBeyondLastLine: false,
            automaticLayout: true,
            padding: { top: 20 },
            scrollbar: {
              vertical: 'auto',
              horizontal: 'auto',
            },
            quickSuggestions: false,
            suggestOnTriggerCharacters: false,
            hover: { enabled: false },
            renderLineHighlight: 'all',
            lineHeight: 1.8,
            renderWhitespace: 'selection',
            formatOnPaste: false,
            formatOnType: false,
          }}
          theme="vs-dark"
          loading={
            <div className="flex items-center justify-center h-full text-gray-500">
              加载编辑器...
            </div>
          }
        />
      </div>
    </div>
  );
}
