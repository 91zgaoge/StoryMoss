import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { ContractsTab } from '../story-system/ContractsTab';

const getContractTree = vi.fn();
const getRuntimeContract = vi.fn();
const listGenesisRuns = vi.fn();

vi.mock('@/services/tauri', () => ({
  getContractTree: (...args: unknown[]) => getContractTree(...args),
  getRuntimeContract: (...args: unknown[]) => getRuntimeContract(...args),
  createMasterSetting: vi.fn(),
  createChapterContract: vi.fn(),
  listGenesisRuns: (...args: unknown[]) => listGenesisRuns(...args),
  logFeatureUsage: vi.fn(),
}));

vi.mock('@/stores/appStore', () => ({
  useAppStore: (selector: (state: unknown) => unknown) =>
    selector({
      currentStory: { id: 'story-1', title: 'Test Story', genre: '奇幻', tone: '热血' },
      setCurrentView: vi.fn(),
    }),
}));

vi.mock('react-hot-toast', () => ({
  default: { success: vi.fn(), error: vi.fn() },
}));

/** 后端 get_contract_tree 返回 DB 行；chapters 以合同 UUID 为 key */
const treeWithChapter1 = {
  master_setting: {
    id: 'ms-1',
    story_id: 'story-1',
    contract_type: 'MASTER_SETTING',
    contract_json: JSON.stringify({ genre: '奇幻', core_tone: '热血' }),
    version: 1,
    created_at: '2026-01-01',
    updated_at: '2026-01-01',
  },
  volumes: {
    'uuid-vol-1': {
      id: 'uuid-vol-1',
      story_id: 'story-1',
      contract_type: 'VOLUME',
      contract_json: JSON.stringify({
        volume_number: 1,
        start_chapter: 1,
        end_chapter: 60,
        goal: '站稳柳林集',
      }),
      version: 1,
      created_at: '2026-01-01',
      updated_at: '2026-01-01',
    },
  },
  chapters: {
    'uuid-ch-1': {
      id: 'uuid-ch-1',
      story_id: 'story-1',
      contract_type: 'CHAPTER',
      contract_json: JSON.stringify({ chapter_number: 1, chapter_directive: { goal: '开篇' } }),
      version: 1,
      created_at: '2026-01-01',
      updated_at: '2026-01-01',
    },
  },
  reviews: {},
};

/** 后端 get_runtime_contract 返回解析后的领域结构体（无 contract_json 字段） */
const runtimeParsedShape = {
  master_setting: {
    schema_version: '1.0',
    contract_type: 'MASTER_SETTING',
    generator_version: 'v1',
    genre: '奇幻',
    core_tone: '热血',
    pacing_strategy: '正常',
    anti_patterns: [],
    world_rules: [],
  },
  chapter_contract: {
    schema_version: '1.0',
    contract_type: 'CHAPTER',
    generator_version: 'v1',
    chapter_number: 1,
    chapter_directive: {
      goal: '完成第1章的情节推进',
      must_cover_nodes: [],
      forbidden_zones: [],
      time_anchor: null,
      chapter_span: null,
    },
  },
};

describe('ContractsTab', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listGenesisRuns.mockResolvedValue([]);
  });

  it('运行时合同按解析后结构体渲染，不崩溃', async () => {
    getContractTree.mockResolvedValue(treeWithChapter1);
    getRuntimeContract.mockResolvedValue(runtimeParsedShape);

    render(<ContractsTab storyId="story-1" selectedChapter={1} onChapterChange={vi.fn()} />);

    expect(await screen.findByText(/核心基调: 热血/)).toBeInTheDocument();
    expect(screen.getByText(/体裁: 奇幻/)).toBeInTheDocument();
    expect(screen.getByText(/章节目标: 完成第1章的情节推进/)).toBeInTheDocument();
  });

  it('卷纲（VOLUME 合同）在合同树卡里展示（v0.70.0）', async () => {
    getContractTree.mockResolvedValue(treeWithChapter1);
    getRuntimeContract.mockResolvedValue(runtimeParsedShape);

    render(<ContractsTab storyId="story-1" selectedChapter={1} onChapterChange={vi.fn()} />);

    expect(await screen.findByTestId('contracts-volume-1')).toBeInTheDocument();
    expect(screen.getByText(/第 1 卷 · 第 1–60 章/)).toBeInTheDocument();
    expect(screen.getByText('站稳柳林集')).toBeInTheDocument();
  });

  it('CHAPTER_1 播种状态按 contract_json 的 chapter_number 识别（而非 UUID key）', async () => {
    getContractTree.mockResolvedValue(treeWithChapter1);
    getRuntimeContract.mockResolvedValue(runtimeParsedShape);

    render(<ContractsTab storyId="story-1" selectedChapter={1} onChapterChange={vi.fn()} />);

    await screen.findByText(/核心基调/);
    // MASTER_SETTING 与 CHAPTER_1 两张卡都应显示「已播种」
    expect(screen.getAllByText('已播种')).toHaveLength(2);
  });

  it('无 MASTER_SETTING 时 get_runtime_contract 报错被捕获，页面不崩溃', async () => {
    getContractTree.mockResolvedValue({
      master_setting: null,
      volumes: {},
      chapters: {},
      reviews: {},
    });
    getRuntimeContract.mockRejectedValue(new Error('缺少 MASTER_SETTING 合同'));

    render(<ContractsTab storyId="story-1" selectedChapter={1} onChapterChange={vi.fn()} />);

    expect(await screen.findByText('暂无合同，请先创建 MASTER_SETTING')).toBeInTheDocument();
    expect(screen.getAllByText('未播种')).toHaveLength(2);
  });

  it('chapter_contract 为 null 时不显示章节目标行', async () => {
    getContractTree.mockResolvedValue(treeWithChapter1);
    getRuntimeContract.mockResolvedValue({ ...runtimeParsedShape, chapter_contract: null });

    render(<ContractsTab storyId="story-1" selectedChapter={2} onChapterChange={vi.fn()} />);

    expect(await screen.findByText(/核心基调: 热血/)).toBeInTheDocument();
    expect(screen.queryByText(/章节目标/)).not.toBeInTheDocument();
  });
});
