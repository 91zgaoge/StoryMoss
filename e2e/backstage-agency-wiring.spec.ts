import { test, expect } from '@playwright/test';
import { getMockTauriInitScript } from './mock-tauri';

/**
 * v0.91.0 P1 调度闭合验收：
 * 级联中心 / Tasks 代理任务 / SceneEditor / ExecutionPanel 均接入
 * 导演主工作流（预填聊天或直接操作代理任务）。
 */

async function openBackstage(page: import('@playwright/test').Page) {
  await page.setViewportSize({ width: 1400, height: 900 });
  await page.addInitScript(getMockTauriInitScript(), { richBackstage: true });
  await page.goto('/index.html');
  await page.waitForTimeout(800);
  await page.getByRole('button', { name: '故事', exact: true }).first().click();
}

test('级联中心可把改稿影响交给导演', async ({ page }) => {
  await openBackstage(page);
  const aside = page.locator('aside');
  await aside.getByRole('button', { name: /^运维/ }).click();
  await aside.getByRole('button', { name: /^级联中心/ }).click();

  await expect(page.getByTestId('cascade-batches')).toBeVisible();
  await page.getByRole('button', { name: '交给导演', exact: true }).first().click();

  const directorInput = page.getByPlaceholder(/对导演说/);
  await expect(directorInput).toHaveValue(/级联影响/);
});

test('Tasks 页代理任务页签可重试导演派发的任务', async ({ page }) => {
  await openBackstage(page);
  const aside = page.locator('aside');
  await aside.getByRole('button', { name: /^运维/ }).click();
  await aside.getByRole('button', { name: /^任务/ }).click();

  await page.getByRole('radio', { name: '代理任务', exact: true }).click();
  const list = page.getByTestId('agency-task-list');
  await expect(list).toBeVisible();
  await expect(list).toContainText('续写第2章');
  await page.getByRole('button', { name: '重试', exact: true }).click();

  await expect
    .poll(() =>
      page.evaluate(() =>
        (window as any).__calls.some(
          (call: { cmd: string }) => call.cmd === 'agency_crew_retry_task'
        )
      )
    )
    .toBe(true);
});

test('场景编辑器交给导演预填修订指令', async ({ page }) => {
  await page.addInitScript(getMockTauriInitScript(), { richBackstage: true });
  await page.setViewportSize({ width: 1400, height: 900 });
  await page.goto('/index.html');
  await page.waitForTimeout(800);
  await page.getByRole('button', { name: '场景', exact: true }).click();
  await page.getByText('雨夜封门', { exact: true }).click();
  await page.getByRole('button', { name: '编辑场景', exact: true }).click();
  await page.getByRole('button', { name: '交给导演', exact: true }).click();

  const directorInput = page.getByPlaceholder(/对导演说/);
  await expect(directorInput).toHaveValue(/修订场景 #1/);
});

test('场景执行面板 AI 续写交给导演预填', async ({ page }) => {
  await page.addInitScript(getMockTauriInitScript(), { richBackstage: true });
  await page.setViewportSize({ width: 1400, height: 900 });
  await page.goto('/index.html');
  await page.waitForTimeout(800);
  await page.getByRole('button', { name: '场景', exact: true }).click();
  await page.getByText('雨夜封门', { exact: true }).click();
  await page.getByText('AI 续写', { exact: true }).click();

  const directorInput = page.getByPlaceholder(/对导演说/);
  await expect(directorInput).toHaveValue(/续写下一章/);
});
