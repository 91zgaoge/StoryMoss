import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

const { setCurrentView, EMPTY_CREW } = vi.hoisted(() => ({
  setCurrentView: vi.fn(),
  EMPTY_CREW: {
    story_id: 's1',
    story_title: '工作室书',
    runs: [],
    tasks: [],
    leases: [],
    corpus_count: 0,
    alerts: [],
  },
}));

vi.mock('@/services/api/agency', () => ({
  listBoard: vi.fn().mockResolvedValue([]),
  getRun: vi.fn().mockResolvedValue(null),
  listRuns: vi.fn().mockResolvedValue([]),
  listActivities: vi.fn().mockResolvedValue([]),
  listChatSessions: vi.fn().mockResolvedValue([]),
  listChatMessages: vi.fn().mockResolvedValue([]),
  crewStatus: vi.fn().mockResolvedValue(EMPTY_CREW),
  sendChat: vi.fn().mockResolvedValue({ session_id: 's1', message_id: 'm1', turn_task_id: 't1' }),
  newChatSession: vi.fn().mockResolvedValue({
    id: 's1',
    story_id: 's1',
    title: '新会话',
    target_role: 'director',
    archived: false,
    created_at: '2026-07-29T10:00:00+08:00',
    updated_at: '2026-07-29T10:00:00+08:00',
  }),
  directorExecute: vi.fn().mockResolvedValue([]),
  directorReject: vi.fn().mockResolvedValue(undefined),
  directorPatrol: vi.fn().mockResolvedValue(EMPTY_CREW),
  crewCancelTask: vi.fn().mockResolvedValue(undefined),
  crewRetryTask: vi.fn().mockResolvedValue('t1'),
  crewDispatch: vi.fn().mockResolvedValue('t1'),
  getEmotionCircuitSnapshot: vi.fn().mockResolvedValue({
    contract: null,
    states: [],
    tensions: [],
    debts: [],
    beats: [],
    probes: [],
    reader_reviews: [],
  }),
  resolveEmotionDebt: vi.fn().mockResolvedValue(undefined),
  runReaderPanel: vi.fn().mockResolvedValue({
    reviews: 0,
    scores: {
      identification: 0,
      sympathy: 0,
      investment: 0,
      tension: 0,
      curiosity: 0,
      trust: 0,
      moved: 0,
    },
    verdict: 'flat',
    moved_moments: [],
    flat_moments: [],
  }),
  suggestStoryEmotionContract: vi.fn().mockResolvedValue({
    story_id: 's1',
    core_question: '建议问题',
    reader_promise: '建议承诺',
    palette: [],
    ending_residue: '',
    forbidden: [],
    peak_budget: 1,
    version: 1,
  }),
  updateStoryEmotionContract: vi.fn().mockResolvedValue(undefined),
}));
vi.mock('@/stores/appStore', () => ({
  useAppStore: (sel: (s: Record<string, unknown>) => unknown) =>
    sel({
      currentStory: { id: 's1', title: '工作室书' },
      setCurrentView,
    }),
}));

import AgencyStudio from '../AgencyStudio';
import {
  crewStatus,
  directorExecute,
  getEmotionCircuitSnapshot,
  listBoard,
  listChatMessages,
  listChatSessions,
  listRuns,
  getRun,
  runReaderPanel,
  sendChat,
} from '@/services/api/agency';
import { useAgencyActivityStore } from '@/stores/agencyActivityStore';

const RUN_1 = {
  id: 'run-1',
  story_id: 's1',
  premise: '一个故事',
  status: 'completed',
  phase: 'assembly',
  lane: 'write',
  result_json: null,
  error_message: null,
  created_at: '2026-07-29T10:00:00+08:00',
  updated_at: '2026-07-29T10:05:00+08:00',
};

const BOARD_ITEM_1 = {
  id: 'b1',
  run_id: 'run-1',
  story_id: 's1',
  zone: 'asset' as const,
  item_type: 'world',
  key: '世界观',
  content: '内容',
  summary: '双星系统',
  version: 1,
  producer: 'producer',
  status: 'active',
  created_at: '2026-07-29T10:01:00+08:00',
  updated_at: '2026-07-29T10:01:00+08:00',
};

function renderStudio() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={qc}>
      <AgencyStudio />
    </QueryClientProvider>
  );
}

describe('AgencyStudio', () => {
  beforeEach(() => {
    useAgencyActivityStore.setState({
      activities: [],
      progress: [],
      activeRunId: null,
      storyId: null,
    });
    vi.mocked(listRuns).mockResolvedValue([]);
    vi.mocked(listBoard).mockResolvedValue([]);
    vi.mocked(getRun).mockResolvedValue(null);
    vi.mocked(listChatSessions).mockResolvedValue([]);
    vi.mocked(listChatMessages).mockResolvedValue([]);
    vi.mocked(crewStatus).mockResolvedValue(EMPTY_CREW);
    vi.mocked(getEmotionCircuitSnapshot).mockResolvedValue({
      contract: null,
      states: [],
      tensions: [],
      debts: [],
      beats: [],
      probes: [],
      reader_reviews: [],
    });
    vi.mocked(runReaderPanel).mockClear();
    vi.mocked(sendChat).mockClear();
    setCurrentView.mockClear();
  });

  it('情感页签显示契约/张力/债务并可运行读者面板', async () => {
    vi.mocked(listRuns).mockResolvedValue([RUN_1]);
    vi.mocked(getEmotionCircuitSnapshot).mockResolvedValue({
      contract: {
        story_id: 's1',
        core_question: '一个人能否在不失去自我的情况下被爱？',
        reader_promise: '看两个人如何学会靠近',
        palette: ['隐忍的渴望'],
        ending_residue: '苦尽甘来',
        forbidden: ['失忆'],
        peak_budget: 1,
        version: 1,
      },
      states: [
        {
          id: 'st1',
          story_id: 's1',
          character_id: 'c1',
          character_name: '林雪',
          scene_id: 'sc1',
          chapter_number: 3,
          beat_index: 1,
          vector: {
            joy: 0,
            sadness: 0.1,
            fear: 0.1,
            anger: 0.3,
            disgust: 0,
            surprise: 0,
            love: 0,
            trust: 0,
            guilt: 0,
            shame: 0.5,
            pride: 0,
            hope: 0,
          },
          intensity: 0.5,
          active_goal: '让他收回那句话',
          defense: '攻击型',
          cause_event: '他当众替她承认',
          source: 'commit',
          created_at: '2026-10-31T10:00:00+08:00',
        },
      ],
      tensions: [
        {
          id: 't1',
          story_id: 's1',
          source_character_id: 'c1',
          target_character_id: 'c2',
          source_name: '林雪',
          target_name: '阿岩',
          tension_type: '未揭穿的欺骗',
          bond: '感激',
          reverse_bond: '戒备',
          pressure: 0.8,
          accumulated_beats: 3,
          due_chapter: 5,
          status: 'open',
          last_escalated_chapter: 2,
          updated_at: '2026-10-31T10:00:00+08:00',
        },
      ],
      debts: [
        {
          id: 'd1',
          story_id: 's1',
          kind: 'promise',
          source_ref: null,
          description: '答应过带她离开',
          pressure: 0.7,
          due_chapter: 5,
          status: 'open',
          created_at: '2026-10-31T10:00:00+08:00',
        },
      ],
      beats: [],
      probes: [],
      reader_reviews: [],
    });
    renderStudio();
    fireEvent.click(await screen.findByText('情感'));
    expect(await screen.findByText(/看两个人如何学会靠近/)).toBeInTheDocument();
    expect(screen.getByText(/未揭穿的欺骗/)).toBeInTheDocument();
    expect(screen.getByText('兑现')).toBeInTheDocument();
    fireEvent.click(screen.getByText('运行'));
    await waitFor(() => expect(runReaderPanel).toHaveBeenCalledWith('s1'));
  });

  it('渲染剧组四角色与对话空态', async () => {
    renderStudio();
    expect((await screen.findAllByText('导演')).length).toBeGreaterThanOrEqual(1);
    expect((await screen.findAllByText('管理')).length).toBeGreaterThanOrEqual(1);
    expect((await screen.findAllByText('编审')).length).toBeGreaterThanOrEqual(1);
    expect((await screen.findAllByText('主创')).length).toBeGreaterThanOrEqual(1);
    expect(await screen.findByText(/剧组已就位/)).toBeInTheDocument();
    expect(await screen.findByText('任务进程')).toBeInTheDocument();
  });

  it('有历史 run 时水合 activeRunId 并显示黑板', async () => {
    vi.mocked(listRuns).mockResolvedValue([RUN_1]);
    vi.mocked(listBoard).mockResolvedValue([BOARD_ITEM_1]);
    vi.mocked(getRun).mockResolvedValue(RUN_1);

    renderStudio();
    fireEvent.click(await screen.findByText('黑板'));
    expect(await screen.findByText('世界观')).toBeInTheDocument();
  });

  it('run 选择器渲染多个 option', async () => {
    const RUN_2 = {
      ...RUN_1,
      id: 'run-2',
      premise: '故事二',
      status: 'running',
      phase: 'writing',
      created_at: '2026-07-29T11:00:00+08:00',
      updated_at: '2026-07-29T11:05:00+08:00',
    };
    vi.mocked(listRuns).mockResolvedValue([RUN_2, RUN_1]);

    renderStudio();
    const select = await screen.findByLabelText('run 选择');
    const options = select.querySelectorAll('option');
    expect(options).toHaveLength(2);
  });

  it('历史时间线从 board items 重建', async () => {
    vi.mocked(listRuns).mockResolvedValue([RUN_1]);
    vi.mocked(listBoard).mockResolvedValue([BOARD_ITEM_1]);
    vi.mocked(getRun).mockResolvedValue(RUN_1);

    renderStudio();
    fireEvent.click(await screen.findByText('时间线'));
    expect(await screen.findByText(/管理 创建 资产：世界观/)).toBeInTheDocument();
    expect(await screen.findByText(/运行启动/)).toBeInTheDocument();
    expect(await screen.findByText(/运行完成/)).toBeInTheDocument();
  });

  it('角色卡从 board items 重建最近动作与 run 状态', async () => {
    const BOARD_WRITER = {
      ...BOARD_ITEM_1,
      id: 'b2',
      zone: 'draft' as const,
      key: '首章',
      producer: 'lead_writer',
      created_at: '2026-07-29T10:02:00+08:00',
    };
    const BOARD_EDITOR = {
      ...BOARD_ITEM_1,
      id: 'b3',
      zone: 'review' as const,
      key: '质检结论',
      producer: 'editor_auditor',
      created_at: '2026-07-29T10:03:00+08:00',
    };
    vi.mocked(listRuns).mockResolvedValue([RUN_1]);
    vi.mocked(listBoard).mockResolvedValue([BOARD_ITEM_1, BOARD_WRITER, BOARD_EDITOR]);
    vi.mocked(getRun).mockResolvedValue(RUN_1);

    renderStudio();
    expect(await screen.findByText(/最近动作：创建 资产：世界观/)).toBeInTheDocument();
    expect(await screen.findByText(/最近动作：创建 草稿：首章/)).toBeInTheDocument();
    expect(await screen.findByText(/最近动作：创建 审查：质检结论/)).toBeInTheDocument();
    expect((await screen.findAllByText(/run 状态：assembly · 完成/)).length).toBeGreaterThanOrEqual(
      3
    );
  });

  it('listRuns 失败时显示错误提示而非静默空态', async () => {
    vi.mocked(listRuns).mockRejectedValue(new Error('IPC boom'));

    renderStudio();
    expect(await screen.findByText(/代理状态获取失败/)).toBeInTheDocument();
    expect(await screen.findByText(/IPC boom/)).toBeInTheDocument();
  });

  it('同一业务事件重复发时时间线只显示一次（业务键去重）', async () => {
    vi.mocked(listRuns).mockResolvedValue([RUN_1]);
    vi.mocked(getRun).mockResolvedValue(RUN_1);
    useAgencyActivityStore.setState({
      storyId: 's1',
      activeRunId: 'run-1',
      activities: [
        { run_id: 'run-1', role: 'producer', action: 'done', detail: '概念', at: 1000 },
        { run_id: 'run-1', role: 'producer', action: 'done', detail: '概念', at: 2000 },
        { run_id: 'run-1', role: 'producer', action: 'start', detail: '概念', at: 1500 },
      ],
      progress: [
        {
          run_id: 'run-1',
          phase: 'concept',
          status: 'running',
          message: '正在构思故事概念',
          at: 1100,
        },
        {
          run_id: 'run-1',
          phase: 'concept',
          status: 'running',
          message: '正在构思故事概念',
          at: 1300,
        },
      ],
    });

    renderStudio();
    fireEvent.click(await screen.findByText('时间线'));
    expect((await screen.findAllByText('管理 done 概念')).length).toBe(1);
    expect((await screen.findAllByText('管理 start 概念')).length).toBe(1);
    expect((await screen.findAllByText('concept running 正在构思故事概念')).length).toBe(1);
  });

  it('观察 run 显示观察中且轨迹为工作中', async () => {
    const OBSERVE_RUN = {
      ...RUN_1,
      id: 'observe-s1',
      premise: '观察',
      status: 'observing',
      phase: 'observe',
      lane: 'inspect',
    };
    vi.mocked(listRuns).mockResolvedValue([OBSERVE_RUN]);
    vi.mocked(getRun).mockResolvedValue(OBSERVE_RUN);
    useAgencyActivityStore.setState({
      storyId: 's1',
      activeRunId: 'observe-s1',
      activities: [
        { run_id: 'observe-s1', role: 'producer', action: 'start', detail: '资产回流', at: 1000 },
        {
          run_id: 'observe-s1',
          role: 'lead_writer',
          action: 'start',
          detail: '编译节拍',
          at: 1100,
        },
        {
          run_id: 'observe-s1',
          role: 'editor_auditor',
          action: 'start',
          detail: '后台审查',
          at: 1200,
        },
      ],
      progress: [],
    });

    renderStudio();
    expect((await screen.findAllByText(/观察中/)).length).toBeGreaterThanOrEqual(1);
    fireEvent.click(await screen.findByText('时间线'));
    expect(await screen.findByText('当前执行轨迹')).toBeInTheDocument();
    expect(screen.getByTestId('ai-thinking-spinner')).toBeInTheDocument();
    expect(await screen.findByText('管理 start 资产回流')).toBeInTheDocument();
    expect(await screen.findByText('主创 start 编译节拍')).toBeInTheDocument();
    expect(await screen.findByText('编审 start 后台审查')).toBeInTheDocument();
  });

  it('点击黑板资产卡跳到对应幕后页', async () => {
    const CHAR_ITEM = {
      ...BOARD_ITEM_1,
      id: 'b-char',
      item_type: 'character',
      key: 'character:苏会山',
      summary: '苏会山',
    };
    vi.mocked(listRuns).mockResolvedValue([RUN_1]);
    vi.mocked(listBoard).mockResolvedValue([CHAR_ITEM]);
    vi.mocked(getRun).mockResolvedValue(RUN_1);

    renderStudio();
    fireEvent.click(await screen.findByText('黑板'));
    fireEvent.click(await screen.findByText('character:苏会山'));
    expect(setCurrentView).toHaveBeenCalledWith('characters');
  });

  it('发送消息调用 agency_chat_send 并立即返回', async () => {
    vi.mocked(listChatSessions).mockResolvedValue([
      {
        id: 's1',
        story_id: 's1',
        title: '今晚',
        target_role: 'director',
        archived: false,
        created_at: '2026-07-29T10:00:00+08:00',
        updated_at: '2026-07-29T10:00:00+08:00',
      },
    ]);
    renderStudio();
    const textarea = await screen.findByLabelText('AI 指令输入');
    fireEvent.change(textarea, { target: { value: '让管理补齐世界观' } });
    fireEvent.click(screen.getByLabelText('发送'));
    await waitFor(() => {
      expect(sendChat).toHaveBeenCalledWith(
        expect.objectContaining({
          story_id: 's1',
          message: '让管理补齐世界观',
          target_role: 'director',
        })
      );
    });
  });

  it('审批卡确认调用 directorExecute', async () => {
    vi.mocked(listChatSessions).mockResolvedValue([
      {
        id: 's1',
        story_id: 's1',
        title: '今晚',
        target_role: 'director',
        archived: false,
        created_at: '2026-07-29T10:00:00+08:00',
        updated_at: '2026-07-29T10:00:00+08:00',
      },
    ]);
    vi.mocked(listChatMessages).mockResolvedValue([
      {
        id: 'm1',
        session_id: 's1',
        role: 'director',
        content: '编审发现 3 处视角越界，是否修订？',
        status: 'done',
        actions_json: JSON.stringify({
          confirmation: '确认修订',
          actions: [
            {
              id: 'a1',
              action_type: 'dispatch',
              params_json: JSON.stringify({
                type: 'dispatch',
                role: 'lead_writer',
                instruction: '按编审建议修订第 12 章',
              }),
            },
          ],
        }),
        created_at: '2026-07-29T10:00:00+08:00',
      },
    ]);

    renderStudio();
    expect(await screen.findByText('需要你确认')).toBeInTheDocument();
    fireEvent.click(await screen.findByText('确认执行'));
    await waitFor(() => {
      expect(directorExecute).toHaveBeenCalledWith(
        expect.objectContaining({ story_id: 's1', session_id: 's1', action_ids: ['a1'] })
      );
    });
  });
});
