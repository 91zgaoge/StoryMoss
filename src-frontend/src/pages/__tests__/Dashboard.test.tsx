import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { Dashboard } from '../Dashboard';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: false },
  },
});

const wrapper = ({ children }: { children: React.ReactNode }) => (
  <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
);

const loggedInvoke = vi.fn();
const setCurrentView = vi.fn();

// Mock Tauri services
vi.mock('@/services/tauri', () => ({
  loggedInvoke: (...args: any[]) => loggedInvoke(...args),
  createStoryWithWizard: vi.fn(),
}));

// Mock app store
vi.mock('@/stores/appStore', () => ({
  useAppStore: (selector: (state: any) => any) =>
    selector({
      stories: [],
      setStories: vi.fn(),
      setCurrentUser: vi.fn(),
      setCurrentStory: vi.fn(),
      setCurrentView,
      isLoading: false,
      currentStory: null,
      currentUser: null,
    }),
}));

// Mock useStories hook
vi.mock('@/hooks/useStories', () => ({
  useStories: () => ({
    data: [
      {
        id: 'story-1',
        title: '测试故事',
        genre: '科幻',
        character_count: 3,
        chapter_count: 5,
        word_count: 1200,
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      },
    ],
    isLoading: false,
  }),
  useCreateStory: () => ({ mutate: vi.fn(), isPending: false }),
}));

vi.mock('@/utils/logger', () => ({
  createLogger: () => ({ error: vi.fn(), debug: vi.fn() }),
}));

vi.mock('@/components/GenesisPanel', () => ({
  GenesisPanel: () => <div data-testid="genesis-panel" />,
}));

vi.mock('@/components/NovelCreationWizard', () => ({
  NovelCreationWizard: () => <div data-testid="novel-wizard" />,
}));

vi.mock('react-hot-toast', () => ({
  default: { success: vi.fn(), error: vi.fn() },
}));

describe('Dashboard', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    queryClient.clear();
    loggedInvoke.mockResolvedValue(undefined);
  });

  it('渲染统计卡并显示正确数值', async () => {
    render(<Dashboard />, { wrapper });

    await waitFor(() => {
      expect(screen.getByText('故事')).toBeInTheDocument();
    });

    expect(screen.getByText('1')).toBeInTheDocument();
    expect(screen.getByText('3')).toBeInTheDocument();
    expect(screen.getByText('5')).toBeInTheDocument();
    expect(screen.getByText('1.2k')).toBeInTheDocument();

    expect(screen.getByText('故事')).toBeInTheDocument();
    expect(screen.getByText('角色')).toBeInTheDocument();
    expect(screen.getByText('场景')).toBeInTheDocument();
    expect(screen.getByText('字数')).toBeInTheDocument();
  });

  it('点击场景统计卡跳转到 scenes 视图', async () => {
    render(<Dashboard />, { wrapper });

    await waitFor(() => {
      expect(screen.getByText('场景')).toBeInTheDocument();
    });

    await userEvent.click(screen.getByText('场景').closest('[class*="cursor-pointer"]')!);
    expect(setCurrentView).toHaveBeenCalledWith('scenes');
  });

  it('点击 AI 创建故事按钮调用 show_frontstage', async () => {
    render(<Dashboard />, { wrapper });

    const createBtn = await screen.findByRole('button', { name: /AI 创建故事/ });
    await userEvent.click(createBtn);

    await waitFor(() => {
      expect(loggedInvoke).toHaveBeenCalledWith('show_frontstage');
    });
  });

  it('点击 CreationPathGuide 的幕后 AI 向导打开 Wizard', async () => {
    render(<Dashboard />, { wrapper });

    await waitFor(() => {
      expect(screen.getByText('幕后 AI 向导')).toBeInTheDocument();
    });

    await userEvent.click(screen.getByRole('button', { name: /幕后 AI 向导/ }));
    expect(screen.getByTestId('novel-wizard')).toBeInTheDocument();
  });
});
