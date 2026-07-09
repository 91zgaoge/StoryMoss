import { useState } from 'react';
import { Search, Plus } from 'lucide-react';
import toast from 'react-hot-toast';
import { useQueryClient } from '@tanstack/react-query';
import { useAppStore } from '@/stores/appStore';
import { listStories } from '@/services/tauri';
import {
  useUploadBook,
  useReferenceBooks,
  useDeleteBook,
  useConvertToStory,
  useBookAnalysis,
  useBookAnalysisStatus,
  useCancelBookAnalysis,
} from '@/hooks/useBookDeconstruction';
import { BookUploadPanel } from '@/components/book-deconstruction/BookUploadPanel';
import { BookListGrid } from '@/components/book-deconstruction/BookListGrid';
import { BookDetailView } from '@/components/book-deconstruction/BookDetailView';
import { AnalysisProgress } from '@/components/book-deconstruction/AnalysisProgress';

export function BookDeconstruction() {
  const [selectedBookId, setSelectedBookId] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [showUpload, setShowUpload] = useState(false);

  const queryClient = useQueryClient();
  const setCurrentStory = useAppStore(s => s.setCurrentStory);
  const setCurrentView = useAppStore(s => s.setCurrentView);

  const { data: books, isLoading } = useReferenceBooks();
  const uploadMutation = useUploadBook();
  const deleteMutation = useDeleteBook();
  const convertMutation = useConvertToStory();

  const selectedAnalysis = useBookAnalysis(selectedBookId);
  const selectedStatus = useBookAnalysisStatus(selectedBookId);
  const cancelMutation = useCancelBookAnalysis();

  const filteredBooks = books?.filter(
    book =>
      book.title.toLowerCase().includes(searchQuery.toLowerCase()) ||
      (book.author?.toLowerCase().includes(searchQuery.toLowerCase()) ?? false)
  );

  const handleUpload = async (filePath: string) => {
    setShowUpload(false);
    try {
      const bookId = await uploadMutation.mutateAsync(filePath);
      setSelectedBookId(bookId);
      toast.success('上传成功，开始分析...');
    } catch (error) {
      toast.error(`上传失败: ${error}`);
    }
  };

  const handleDelete = async (bookId: string) => {
    if (!confirm('确定要删除这本书的分析结果吗？')) return;
    try {
      await deleteMutation.mutateAsync(bookId);
      if (selectedBookId === bookId) {
        setSelectedBookId(null);
      }
      toast.success('删除成功');
    } catch (error) {
      toast.error(`删除失败: ${error}`);
    }
  };

  const handleConvertToStory = async () => {
    if (!selectedBookId) return;
    try {
      const storyId = await convertMutation.mutateAsync(selectedBookId);
      await queryClient.invalidateQueries({ queryKey: ['stories'] });
      const stories = await listStories();
      const story = stories.find(s => s.id === storyId);
      if (story) {
        setCurrentStory(story);
      }
      setCurrentView('scenes');
      toast.success(story ? `已转为故事项目：${story.title}` : '已转为故事项目');
    } catch (error) {
      toast.error(`转换失败: ${error}`);
    }
  };

  const handleCancel = async () => {
    if (!selectedBookId) return;
    if (!confirm('确定要取消当前分析吗？已处理的部分不会被保存。')) return;
    try {
      await cancelMutation.mutateAsync(selectedBookId);
      toast.success('分析已取消');
    } catch (error) {
      toast.error(`取消失败: ${error}`);
    }
  };

  const renderContent = () => {
    if (showUpload) {
      return (
        <div className="max-w-2xl mx-auto mt-8">
          <BookUploadPanel onUpload={handleUpload} isUploading={uploadMutation.isPending} />
        </div>
      );
    }

    if (!selectedBookId) {
      return (
        <div className="flex flex-col items-center justify-center h-full text-gray-500">
          <div className="w-16 h-16 rounded-full bg-cinema-800 flex items-center justify-center mb-4">
            <Search className="w-8 h-8" />
          </div>
          <p className="text-lg font-medium mb-2">选择一本书查看分析结果</p>
          <p className="text-sm">或上传一本新小说开始分析</p>
        </div>
      );
    }

    // 分析中 / 已取消状态
    if (
      selectedStatus?.status === 'pending' ||
      selectedStatus?.status === 'extracting' ||
      selectedStatus?.status === 'analyzing' ||
      selectedStatus?.status === 'cancelled'
    ) {
      return (
        <AnalysisProgress
          progress={selectedStatus?.progress || 0}
          currentStep={selectedStatus?.current_step || '正在分析...'}
          status={selectedStatus?.status}
          onCancel={selectedStatus?.status !== 'cancelled' ? handleCancel : undefined}
          isCancelling={cancelMutation.isPending}
          activeThreads={selectedStatus?.active_threads}
          maxThreads={selectedStatus?.max_threads}
        />
      );
    }

    // 分析失败
    if (selectedStatus?.status === 'failed') {
      return (
        <div className="flex flex-col items-center justify-center h-full text-gray-500">
          <div className="text-red-400 text-lg font-medium mb-2">分析失败</div>
          <p className="text-sm">{selectedStatus.error || '未知错误'}</p>
        </div>
      );
    }

    // 分析完成
    if (selectedAnalysis.data) {
      return (
        <BookDetailView
          analysis={selectedAnalysis.data}
          onConvertToStory={handleConvertToStory}
          isConverting={convertMutation.isPending}
        />
      );
    }

    return (
      <div className="flex items-center justify-center h-full">
        <div className="text-gray-500">加载中...</div>
      </div>
    );
  };

  return (
    <div className="flex h-full bg-cinema-950">
      {/* 左侧栏 */}
      <div className="w-80 border-r border-cinema-800 flex flex-col bg-cinema-900">
        {/* 头部 */}
        <div className="p-4 border-b border-cinema-800">
          <div className="flex items-center justify-between mb-3">
            <h2 className="text-lg font-bold text-white">拆书</h2>
            <button
              onClick={() => {
                setShowUpload(true);
                setSelectedBookId(null);
              }}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-cinema-gold/20 text-cinema-gold text-sm hover:bg-cinema-gold/30 transition-colors"
            >
              <Plus className="w-4 h-4" />
              上传
            </button>
          </div>
          <div className="relative">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-500" />
            <input
              type="text"
              value={searchQuery}
              onChange={e => setSearchQuery(e.target.value)}
              placeholder="搜索书籍..."
              className="w-full pl-9 pr-3 py-2 rounded-lg bg-cinema-800 border border-cinema-700 text-sm text-white placeholder-gray-500 focus:outline-none focus:border-cinema-gold/50"
            />
          </div>
        </div>

        {/* 书籍列表 */}
        <div className="flex-1 overflow-auto p-3">
          {isLoading ? (
            <div className="text-center py-8 text-gray-500">加载中...</div>
          ) : filteredBooks && filteredBooks.length > 0 ? (
            <BookListGrid
              books={filteredBooks}
              selectedId={selectedBookId}
              onSelect={id => {
                setSelectedBookId(id);
                setShowUpload(false);
              }}
              onDelete={handleDelete}
            />
          ) : (
            <div className="text-center py-8 text-gray-500">
              {searchQuery ? '未找到匹配的书籍' : '暂无分析记录'}
            </div>
          )}
        </div>
      </div>

      {/* 右侧内容区 */}
      <div className="flex-1 overflow-hidden">{renderContent()}</div>
    </div>
  );
}
