import { test, expect } from '@playwright/test';
import { getMockTauriInitScript } from './mock-tauri';

/**
 * v0.74.0：语料库 + 代理工作室（导演对话/现场）最小端到端探针。
 * 只验证用户可点路径与 IPC 契约，不依赖真实模型。
 */
test.describe('语料库与代理工作室', () => {
  test.beforeEach(async ({ page }) => {
    await page.setViewportSize({ width: 1680, height: 1000 });
  });

  test('语料页可加载、选中、切换 AI 提炼', async ({ page }) => {
    const errors: string[] = [];
    page.on('pageerror', err => errors.push(err.message));

    await page.addInitScript(getMockTauriInitScript());
    await page.goto('/index.html');

    await page.locator('nav').getByRole('button', { name: '故事', exact: true }).click();
    await expect(page.locator('aside')).toBeVisible({ timeout: 10000 });
    await page.getByRole('heading', { name: '测试故事' }).first().click();

    await page.locator('aside').getByRole('button', { name: /语料/ }).click();
    await expect(page.getByText('语料库 · 测试故事')).toBeVisible({ timeout: 10000 });
    await expect(page.getByText('老街雨夜').first()).toBeVisible();

    await page.getByText('老街雨夜').first().click();
    await expect(page.getByText('AI 提炼').first()).toBeVisible();
    await page.getByText('AI 提炼').first().click();
    await expect(page.getByText('核心事实').first()).toBeVisible();
    expect(errors.filter(e => !e.includes('enablePersistence'))).toHaveLength(0);
  });

  test('代理工作室显示剧组、对话与现场，发送消息走 agency_chat_send', async ({ page }) => {
    const errors: string[] = [];
    page.on('pageerror', err => errors.push(err.message));

    await page.addInitScript(getMockTauriInitScript());
    await page.goto('/index.html');

    await page.locator('nav').getByRole('button', { name: '故事', exact: true }).click();
    await expect(page.locator('aside')).toBeVisible({ timeout: 10000 });
    await page.getByRole('heading', { name: '测试故事' }).first().click();

    await page.locator('aside').getByRole('button', { name: /代理工作室/ }).click();
    await expect(page.getByText('代理工作室 · 测试故事')).toBeVisible({ timeout: 10000 });
    await expect(page.getByText('剧组').first()).toBeVisible();
    await expect(page.getByText(/剧组已就位/)).toBeVisible();
    await expect(page.getByText('任务进程')).toBeVisible();

    await page.getByLabel('AI 指令输入').fill('让管理补齐世界观');
    await page.getByLabel('发送').click();

    const call = await page.evaluate(() =>
      (window as any).__calls?.find((c: any) => c.cmd === 'agency_chat_send')
    );
    expect(call, '应调用 agency_chat_send').toBeTruthy();
    expect(call.args.message).toBe('让管理补齐世界观');
    expect(call.args.target_role).toBe('director');
    expect(errors.filter(e => !e.includes('enablePersistence'))).toHaveLength(0);
  });
});
