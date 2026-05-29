import { BookOpen, Trash2, Loader2, CheckCircle2, AlertCircle } from 'lucide-react';
import type { ReferenceBookSummary } from '@/types/book-deconstruction';

interface BookListGridProps {
  books: ReferenceBookSummary[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onDelete: (id: string) => void;
}

export function BookListGrid({ books, selectedId, onSelect, onDelete }: BookListGridProps) {
  const getStatusIcon = (status: string) => {
    switch (status) {
      case 'completed':
        return <CheckCircle2 className="w-4 h-4 text-green-500" />;
      case 'failed':
        return <AlertCircle className="w-4 h-4 text-red-500" />;
      case 'pending':
      case 'extracting':
      case 'analyzing':
        return <Loader2 className="w-4 h-4 text-cinema-gold animate-spin" />;
      case 'cancelled':
        return <AlertCircle className="w-4 h-4 text-orange-500" />;
      default:
        return null;
    }
  };

  const getStatusLabel = (status: string) => {
    const map: Record<string, string> = {
      pending: '等待中',
      extracting: '提取中',
      analyzing: '分析中',
      completed: '已完成',
      failed: '失败',
      cancelled: '已取消',
    };
    return map[status] || status;
  };

  const formatWordCount = (count?: number) => {
    if (!count) return '';
    if (count >= 10000) return `${(count / 10000).toFixed(1)}万字`;
    return `${count}字`;
  };

  return (
    <div className="grid grid-cols-1 gap-3">
      {books.map(book => (
        <div
          key={book.id}
          onClick={() => onSelect(book.id)}
          className={`flex items-center gap-3 p-3 rounded-xl cursor-pointer transition-all ${
            selectedId === book.id
              ? 'bg-cinema-gold/10 border border-cinema-gold/30'
              : 'bg-cinema-900 border border-cinema-800 hover:border-cinema-700'
          }`}
        >
          <div className="w-10 h-10 rounded-lg bg-cinema-800 flex items-center justify-center flex-shrink-0">
            <BookOpen className="w-5 h-5 text-cinema-gold" />
          </div>
          <div className="flex-1 min-w-0">
            <h4 className="text-sm font-medium text-white truncate">{book.title}</h4>
            <div className="flex items-center gap-2 mt-1">
              <span className="text-xs text-gray-500">{book.author || '未知作者'}</span>
              {book.word_count && (
                <span className="text-xs text-gray-600">{formatWordCount(book.word_count)}</span>
              )}
            </div>
          </div>
          <div className="flex items-center gap-2">
            {getStatusIcon(book.analysis_status)}
            <span className="text-xs text-gray-500">{getStatusLabel(book.analysis_status)}</span>
            {book.analysis_status !== 'analyzing' && book.analysis_status !== 'extracting' && (
              <button
                onClick={e => {
                  e.stopPropagation();
                  onDelete(book.id);
                }}
                className="p-1.5 rounded-lg hover:bg-red-500/10 text-gray-500 hover:text-red-400 transition-colors"
              >
                <Trash2 className="w-3.5 h-3.5" />
              </button>
            )}
          </div>
        </div>
      ))}
    </div>
  );
}
