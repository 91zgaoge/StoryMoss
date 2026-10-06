import { test, expect, type Locator, type Page } from '@playwright/test';
import {
  getMockTauriInitScript,
  type MockSmartExecuteStep,
  type MockTauriArgs,
} from './mock-tauri';

/**
 * 幕前 Agency Append（续写）主路径 E2E
 *
 * 锁定三条用户可感知契约：
 *  1. 续写正文进入编辑器并落库：smart_execute 返回的增量经 appendAiContent 写入
 *     编辑器正文，随后 update_scene 携带追加后的正文落库；
 *  2. 不重复：同一段续写重复到达时，编辑器/落库内容里只保留一份；
 *  3. 生成中重复提交不产生第二份内容，且不弹「需要您先处理」中断卡。
 *
 * mock 只实现 IPC 契约（src-tauri smart_execute -> PlanExecutionResult），不伪造前端行为。
 * 文思活跃（active）是幕前续写的直接追加路径；追加成功后会排队一次 auto-continue，
 * 因此 mock 把第二次 smart_execute 应答设为「已有进行中的创作任务」结构化错误来收敛流程，
 * 这同时覆盖了「撞上进行中 run 不弹中断卡」的回归。
 */

const OPENING_MARKER = '沈砚之握紧了那枚残缺的铜印';
const OPENING_TEXT = [
  '夜色像一层洇开的墨，沿着长街缓慢铺展。',
  '沈砚之握紧了那枚残缺的铜印，指腹能摸到边缘细密的缺口。',
  '这是父亲留下的唯一念想，也是他踏入这座城的全部理由。',
].join('\n');

const CONTINUATION_MARKER = '他推开茶楼的木门';
const CONTINUATION_TEXT = [
  '他推开茶楼的木门，铜铃在头顶轻响，湿冷的夜风被挡在身后。',
  '柜台后的掌柜抬起眼皮，目光在那枚铜印上停了一瞬，随即又垂了下去。',
  '「客官，打尖还是住店？」掌柜的声音压得很低，像是怕被楼上的什么人听见。',
].join('\n');

/** 与后端 map_active_run_conflict 同源的「已有进行中的创作任务」结构化错误。 */
const ACTIVE_RUN_CONFLICT = {
  code: 'VALIDATION_FAILED',
  message: '该故事已有进行中的创作任务',
  severity: 'UserAction',
  data: { field: 'active_run' },
};

function seededArgs(steps: MockSmartExecuteStep[]): MockTauriArgs {
  return {
    frontstageSeed: { content: OPENING_TEXT },
    smartExecuteSteps: steps,
  };
}

async function bootFrontstage(page: Page, args: MockTauriArgs): Promise<Locator> {
  await page.addInitScript(getMockTauriInitScript(), args);
  await page.goto('/frontstage.html');

  const editor = page.locator('.ProseMirror').first();
  await expect(editor).toBeVisible({ timeout: 15000 });
  // 等种子章节正文加载进编辑器（scene_id = e2e-scene-1 已随章节关联）
  await expect(editor).toContainText(OPENING_MARKER, { timeout: 15000 });
  return editor;
}

/** 文思被动 -> 文思活跃：AI 续写结果直接追加进正文（appendAiContent 路径）。 */
async function enableActiveWensi(page: Page): Promise<void> {
  const toggle = page.locator('.wensi-mode-toggle');
  await expect(toggle).toBeVisible({ timeout: 15000 });
  await toggle.click();
  await expect(toggle).toHaveAttribute('aria-label', /文思活跃/);
}

const instructionInput = (page: Page) => page.locator('textarea[aria-label="AI 指令输入"]');

async function submitInstruction(page: Page, text: string): Promise<void> {
  const input = instructionInput(page);
  await expect(input).toBeEnabled();
  await input.fill(text);
  await expect(input).toHaveValue(text);
  await input.press('Enter');
}

interface RecordedCall {
  cmd: string;
  args: any;
  at: number;
}

const callsOf = (page: Page, cmd: string): Promise<RecordedCall[]> =>
  page.evaluate(
    c => ((window as any).__calls as RecordedCall[]).filter(entry => entry.cmd === c),
    cmd
  );

const smartExecuteCount = (page: Page): Promise<number> =>
  page.evaluate(() => (window as any).__e2eMock.count('smart_execute'));

/**
 * 去 HTML / 去空白后统计整段文本出现次数。
 * 编辑器的 autoFormatText 会按句分段（插入 <p>），落库内容是 HTML，
 * 因此「同一段续写只出现一次」必须按归一化后的完整文本比较。
 */
function countNormalizedOccurrences(haystack: string, needle: string): number {
  const normalize = (s: string) => s.replace(/<[^>]*>/g, '').replace(/\s+/g, '');
  const normalizedNeedle = normalize(needle);
  if (!normalizedNeedle) return 0;
  return normalize(haystack).split(normalizedNeedle).length - 1;
}

function persistedContents(calls: RecordedCall[]): string[] {
  return calls
    .filter(entry => entry.cmd === 'update_scene')
    .map(entry => entry.args?.updates?.content)
    .filter((content: unknown): content is string => typeof content === 'string')
    .filter(content => content.includes(CONTINUATION_MARKER));
}

test.describe('幕前 Agency Append（续写）', () => {
  test.beforeEach(async ({ page }) => {
    await page.setViewportSize({ width: 1920, height: 1080 });
  });

  test('续写增量写入编辑器，并经 update_scene 落库', async ({ page }) => {
    const editor = await bootFrontstage(
      page,
      seededArgs([
        { finalContent: CONTINUATION_TEXT },
        // 文思活跃 auto-continue 撞上进行中的 run：收敛且不弹中断卡
        { error: ACTIVE_RUN_CONFLICT },
      ])
    );
    await enableActiveWensi(page);
    await submitInstruction(page, '续写');

    // 1) 续写正文进入编辑器正文
    await expect(editor).toContainText(CONTINUATION_MARKER, { timeout: 15000 });

    // 2) smart_execute 收到的是真实的续写请求 payload
    const smartCalls = await callsOf(page, 'smart_execute');
    expect(smartCalls.length).toBeGreaterThanOrEqual(1);
    expect(smartCalls[0].args.user_input).toBe('续写');
    expect(smartCalls[0].args.scene_id).toBe('e2e-scene-1');
    expect(smartCalls[0].args.intent_classification?.is_continuation).toBe(true);
    expect(smartCalls[0].args.intent_classification?.is_new_novel).toBe(false);
    expect(smartCalls[0].args.current_content).toContain(OPENING_MARKER);

    // 3) 落库：update_scene 携带「原文 + 追加增量」，且发生在 smart_execute 之后
    await expect
      .poll(async () => persistedContents(await callsOf(page, 'update_scene')).length, {
        timeout: 10000,
      })
      .toBeGreaterThanOrEqual(1);

    const sceneCalls = await callsOf(page, 'update_scene');
    const withMarker = sceneCalls.filter(
      entry =>
        typeof entry.args?.updates?.content === 'string' &&
        entry.args.updates.content.includes(CONTINUATION_MARKER)
    );
    expect(withMarker[0].args.scene_id).toBe('e2e-scene-1');
    expect(withMarker[0].args.updates.content).toContain(OPENING_MARKER);

    const ordered = await page.evaluate(marker => {
      const calls = (window as any).__calls as RecordedCall[];
      const smartIdx = calls.findIndex(entry => entry.cmd === 'smart_execute');
      const persistIdx = calls.findIndex(
        entry =>
          entry.cmd === 'update_scene' &&
          typeof entry.args?.updates?.content === 'string' &&
          entry.args.updates.content.includes(marker)
      );
      return { smartIdx, persistIdx };
    }, CONTINUATION_MARKER);
    expect(ordered.smartIdx).toBeGreaterThanOrEqual(0);
    expect(ordered.persistIdx).toBeGreaterThan(ordered.smartIdx);

    // 4) 不重复：原文与新增各只出现一次
    const text = await editor.innerText();
    expect(countNormalizedOccurrences(text, OPENING_TEXT)).toBe(1);
    expect(countNormalizedOccurrences(text, CONTINUATION_TEXT)).toBe(1);

    // 5) 不弹中断卡
    await expect(page.getByText('需要您先处理')).toHaveCount(0);
    await expect(page.getByText('前往设置')).toHaveCount(0);
  });

  test('同一段续写重复到达时只保留一份', async ({ page }) => {
    const editor = await bootFrontstage(
      page,
      seededArgs([
        { finalContent: CONTINUATION_TEXT },
        // auto-continue 又拿到同一段增量（后端重复投递/重放）
        { finalContent: CONTINUATION_TEXT },
        // 收敛，避免 auto-continue 无限续跑
        { error: ACTIVE_RUN_CONFLICT },
      ])
    );
    await enableActiveWensi(page);
    await submitInstruction(page, '续写');

    await expect(editor).toContainText(CONTINUATION_MARKER, { timeout: 15000 });
    // 等同一段增量第二次经 smart_execute 到达
    await expect.poll(() => smartExecuteCount(page), { timeout: 15000 }).toBeGreaterThanOrEqual(2);

    const text = await editor.innerText();
    expect(countNormalizedOccurrences(text, CONTINUATION_TEXT)).toBe(1);
    expect(countNormalizedOccurrences(text, OPENING_TEXT)).toBe(1);

    const persisted = persistedContents(await callsOf(page, 'update_scene'));
    expect(persisted.length).toBeGreaterThanOrEqual(1);
    for (const content of persisted) {
      expect(countNormalizedOccurrences(content, CONTINUATION_TEXT)).toBe(1);
    }
  });

  test('生成中重复提交不产生第二份内容，也不弹中断卡', async ({ page }) => {
    const editor = await bootFrontstage(
      page,
      seededArgs([
        // 第一次续写挂起，把「生成中」状态固定下来
        { hold: true, finalContent: CONTINUATION_TEXT },
        { error: ACTIVE_RUN_CONFLICT },
      ])
    );
    await enableActiveWensi(page);
    await submitInstruction(page, '续写');

    // 生成中：发送键被取消键替换，输入区禁用
    await expect(page.getByRole('button', { name: '取消生成' })).toBeVisible({ timeout: 15000 });
    await expect(instructionInput(page)).toBeDisabled();
    expect(await smartExecuteCount(page)).toBe(1);

    // 生成中再次提交（模拟竞态下漏过一次 Enter）
    await page.evaluate(() => {
      const ta = document.querySelector(
        'textarea[aria-label="AI 指令输入"]'
      ) as HTMLTextAreaElement;
      const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')!.set!;
      setter.call(ta, '续写');
      ta.dispatchEvent(new Event('input', { bubbles: true }));
    });
    await expect(instructionInput(page)).toHaveValue('续写');
    await page.evaluate(() => {
      const ta = document.querySelector(
        'textarea[aria-label="AI 指令输入"]'
      ) as HTMLTextAreaElement;
      // 绕过浏览器 disabled 面，逼真的「生成中漏过一次 Enter」竞态：应用层闸门仍须拦住
      ta.removeAttribute('disabled');
      ta.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true })
      );
    });
    // 提交路径确实执行（handleInputSubmit 清空输入框），但没有产生第二次生成
    await expect(instructionInput(page)).toHaveValue('');
    await expect.poll(() => smartExecuteCount(page)).toBe(1);
    await expect(page.getByText('需要您先处理')).toHaveCount(0);
    expect(await editor.innerText()).not.toContain(CONTINUATION_MARKER);

    // 放行第一次生成：正文只追加一份
    await page.evaluate(() => (window as any).__e2eMock.releaseSmartExecute());
    await expect(editor).toContainText(CONTINUATION_MARKER, { timeout: 15000 });

    // auto-continue 撞上进行中的 run：不弹「需要您先处理」中断卡
    await expect.poll(() => smartExecuteCount(page), { timeout: 15000 }).toBeGreaterThanOrEqual(2);
    await expect(page.getByText('需要您先处理')).toHaveCount(0);
    await expect(page.getByText('前往设置')).toHaveCount(0);

    expect(countNormalizedOccurrences(await editor.innerText(), CONTINUATION_TEXT)).toBe(1);
  });
});
