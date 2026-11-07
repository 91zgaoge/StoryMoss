import { test, expect } from '@playwright/test';
import { getMockTauriInitScript } from './mock-tauri';

test('Dashboard 创作路径与导航在桌面保持正常尺度', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  await page.addInitScript(getMockTauriInitScript());
  await page.goto('/index.html');
  await page.waitForTimeout(800);

  const pathTitle = page.getByText('幕前 Genesis Pipeline', { exact: true });
  const pathCard = page.getByRole('button', {
    name: '幕前 Genesis Pipeline',
    exact: true,
  });
  await expect(pathTitle).toBeVisible();
  const titleBox = await pathTitle.boundingBox();
  const cardBox = await pathCard.boundingBox();
  expect(titleBox).not.toBeNull();
  expect(cardBox).not.toBeNull();
  expect(cardBox!.width).toBeGreaterThanOrEqual(280);
  expect(titleBox!.height).toBeLessThanOrEqual(50);

  const launcher = page.getByRole('button', { name: '开幕前', exact: true });
  await expect(launcher).toBeVisible();
  const launcherPosition = await launcher
    .locator('..')
    .evaluate(element => getComputedStyle(element).position);
  expect(launcherPosition).not.toBe('fixed');
  await page.screenshot({
    path: 'test-results/studio-refinement/dashboard-1400x900.png',
  });

  await page.getByRole('button', { name: '故事', exact: true }).first().click();
  const group = page.getByRole('button', { name: '运维', exact: true });
  const fontSize = await group.evaluate(element => parseFloat(getComputedStyle(element).fontSize));
  expect(fontSize).toBeGreaterThanOrEqual(13);
});

test('设置模型角色在最小窗口保持可读宽度', async ({ page }) => {
  await page.setViewportSize({ width: 800, height: 600 });
  await page.addInitScript(getMockTauriInitScript(), { richBackstage: true });
  await page.goto('/index.html');
  await page.getByRole('button', { name: '设置', exact: true }).click();

  const description = page.getByText('主创续写与正文 — 质量优先。不要给管理和回流用。', {
    exact: true,
  });
  await expect(description).toBeVisible();
  const box = await description.boundingBox();
  expect(box).not.toBeNull();
  expect(box!.width).toBeGreaterThanOrEqual(240);
  expect(box!.height).toBeLessThanOrEqual(60);

  await page.waitForTimeout(400);
  await page.screenshot({
    path: 'test-results/studio-refinement/settings-models-800x600.png',
  });

  await page.getByTestId('model-role-tool').locator('select').selectOption('model-creative');
  await expect
    .poll(() =>
      page.evaluate(() =>
        (window as any).__calls.some((call: { cmd: string }) => call.cmd === 'set_active_model')
      )
    )
    .toBe(true);
});

test('Agent 列表完整展开且不侵入后续质检配置', async ({ page }) => {
  await page.setViewportSize({ width: 800, height: 600 });
  await page.addInitScript(getMockTauriInitScript(), { richBackstage: true });
  await page.goto('/index.html');
  await page.getByRole('button', { name: '设置', exact: true }).click();
  await page
    .getByTestId('settings-tabs')
    .getByRole('button', { name: 'Agent', exact: true })
    .click();
  await page.getByRole('button', { name: '任务策略', exact: true }).first().click();
  await expect(page.getByText('约束标签', { exact: true })).toBeVisible();

  const lastAgent = page.getByText('知识蒸馏师', { exact: true });
  const quality = page.getByText('Agent 质检配置', { exact: true }).last();
  await expect(lastAgent).toBeAttached();
  await expect(quality).toBeAttached();
  const lastBox = await lastAgent.boundingBox();
  const qualityBox = await quality.boundingBox();
  expect(lastBox).not.toBeNull();
  expect(qualityBox).not.toBeNull();
  expect(lastBox!.y + lastBox!.height).toBeLessThanOrEqual(qualityBox!.y);

  const scroller = page.locator('.settings-panel-scope');
  await scroller.evaluate(element => {
    element.scrollTop = element.scrollHeight;
  });
  await expect(quality).toBeInViewport();
  await page.waitForTimeout(400);
  await page.screenshot({
    path: 'test-results/studio-refinement/settings-agents-800x600.png',
  });
});

test('场景预览在中屏为执行区保留完整宽度并可滚动到达', async ({ page }) => {
  await page.setViewportSize({ width: 1024, height: 768 });
  await page.addInitScript(getMockTauriInitScript(), { richBackstage: true });
  await page.goto('/index.html');
  await page.waitForTimeout(800);
  await page.getByRole('button', { name: '场景', exact: true }).click();
  await page.getByText('雨夜封门', { exact: true }).click();
  await expect(page.locator('h1').getByText('雨夜封门', { exact: true })).toBeVisible();

  const workspace = page.getByTestId('scenes-workspace');
  const execution = page.getByTestId('scenes-execution-column');
  const workspaceBox = await workspace.boundingBox();
  const executionBox = await execution.boundingBox();
  expect(workspaceBox).not.toBeNull();
  expect(executionBox).not.toBeNull();
  expect(executionBox!.width).toBeGreaterThanOrEqual(workspaceBox!.width * 0.95);

  await workspace.evaluate(element => {
    element.scrollTop = element.scrollHeight;
  });
  await expect(execution).toBeInViewport();
  await page.waitForTimeout(400);
  await page.screenshot({
    path: 'test-results/studio-refinement/scenes-preview-1024x768.png',
  });

  await page.setViewportSize({ width: 1400, height: 900 });
  await workspace.evaluate(element => {
    element.scrollTop = 0;
  });
  const wideWorkspaceBox = await workspace.boundingBox();
  const wideExecutionBox = await execution.boundingBox();
  expect(wideWorkspaceBox).not.toBeNull();
  expect(wideExecutionBox).not.toBeNull();
  expect(wideExecutionBox!.width).toBeLessThanOrEqual(320);
  expect(Math.abs(wideExecutionBox!.y - wideWorkspaceBox!.y)).toBeLessThanOrEqual(1);
  await page.waitForTimeout(400);
  await page.screenshot({
    path: 'test-results/studio-refinement/scenes-preview-1400x900.png',
  });
});

test('场景预览在五档宽度不错位且可返回顶部', async ({ page }) => {
  await page.setViewportSize({ width: 1000, height: 800 });
  await page.addInitScript(getMockTauriInitScript(), { richBackstage: true });
  await page.goto('/index.html');
  await page.waitForTimeout(800);
  await page.getByRole('button', { name: '场景', exact: true }).click();
  await page.getByText('雨夜封门', { exact: true }).click();
  await expect(page.locator('h1').getByText('雨夜封门', { exact: true })).toBeVisible();

  const workspace = page.getByTestId('scenes-workspace');
  const timeline = page.getByTestId('scenes-timeline-column');
  const preview = page.getByTestId('scenes-preview-column');
  const execution = page.getByTestId('scenes-execution-column');
  const storyHeading = page.getByRole('heading', { name: '故事线', exact: true });
  const sceneHeading = page.locator('h1').getByText('雨夜封门', { exact: true });

  for (const width of [1000, 1024, 1279, 1280, 1400]) {
    await page.setViewportSize({ width, height: 800 });
    await workspace.evaluate(element => {
      element.scrollTop = 0;
    });
    await page.waitForTimeout(250);

    expect(await workspace.evaluate(element => element.scrollTop)).toBe(0);
    await expect(storyHeading).toBeInViewport();
    await expect(sceneHeading).toBeInViewport();

    const workspaceBox = await workspace.boundingBox();
    const timelineBox = await timeline.boundingBox();
    const previewBox = await preview.boundingBox();
    const executionBox = await execution.boundingBox();
    expect(workspaceBox).not.toBeNull();
    expect(timelineBox).not.toBeNull();
    expect(previewBox).not.toBeNull();
    expect(executionBox).not.toBeNull();

    const sceneHeadingBox = await sceneHeading.boundingBox();
    expect(sceneHeadingBox).not.toBeNull();
    expect(sceneHeadingBox!.y).toBeGreaterThanOrEqual(previewBox!.y - 1);
    expect(sceneHeadingBox!.y + sceneHeadingBox!.height).toBeLessThanOrEqual(
      previewBox!.y + previewBox!.height + 1
    );

    if (width < 1024) {
      expect(timelineBox!.y + timelineBox!.height).toBeLessThanOrEqual(previewBox!.y + 1);
      expect(previewBox!.y + previewBox!.height).toBeLessThanOrEqual(executionBox!.y + 1);
    } else if (width < 1280) {
      expect(Math.abs(timelineBox!.y - previewBox!.y)).toBeLessThanOrEqual(1);
      expect(timelineBox!.height).toBeGreaterThanOrEqual(workspaceBox!.height - 2);
      expect(previewBox!.height).toBeGreaterThanOrEqual(workspaceBox!.height - 2);
      expect(executionBox!.y).toBeGreaterThanOrEqual(previewBox!.y + previewBox!.height - 1);
      expect(executionBox!.width).toBeGreaterThanOrEqual(workspaceBox!.width * 0.95);
    } else {
      expect(Math.abs(timelineBox!.y - workspaceBox!.y)).toBeLessThanOrEqual(1);
      expect(Math.abs(previewBox!.y - workspaceBox!.y)).toBeLessThanOrEqual(1);
      expect(Math.abs(executionBox!.y - workspaceBox!.y)).toBeLessThanOrEqual(1);
    }

    await workspace.evaluate(element => {
      element.scrollTop = element.scrollHeight;
    });
    await expect(execution).toBeInViewport();
    await workspace.evaluate(element => {
      element.scrollTop = 0;
    });
    await expect(storyHeading).toBeInViewport();
    await expect(sceneHeading).toBeInViewport();
    await page.waitForTimeout(400);
    await page.screenshot({
      path: `test-results/studio-refinement/scenes-geometry-${width}.png`,
    });
  }

  await page.setViewportSize({ width: 1024, height: 800 });
  await workspace.evaluate(element => {
    element.scrollTop = element.scrollHeight;
  });
  await page.setViewportSize({ width: 1000, height: 800 });
  const resizedTimeline = await timeline.boundingBox();
  const resizedPreview = await preview.boundingBox();
  const resizedExecution = await execution.boundingBox();
  expect(resizedTimeline!.y + resizedTimeline!.height).toBeLessThanOrEqual(resizedPreview!.y + 1);
  expect(resizedPreview!.y + resizedPreview!.height).toBeLessThanOrEqual(resizedExecution!.y + 1);
});

test('场景编辑在窄屏把管线放到正文下方而不是挤占正文', async ({ page }) => {
  await page.setViewportSize({ width: 800, height: 600 });
  await page.addInitScript(getMockTauriInitScript(), { richBackstage: true });
  page.on('console', message => {
    if (message.type() === 'error') console.error(message.text());
  });
  await page.goto('/index.html');
  await page.waitForTimeout(800);
  await page.getByRole('button', { name: '场景', exact: true }).click();
  await page.getByText('雨夜封门', { exact: true }).click();
  await page.getByRole('button', { name: '编辑场景', exact: true }).click();
  await page.getByRole('button', { name: '起草', exact: true }).click();

  const editor = page.getByTestId('scene-editor');
  const rail = page.getByTestId('scene-editor-pipeline-rail');
  await expect(rail).toBeVisible();
  const editorBox = await editor.boundingBox();
  const railBox = await rail.boundingBox();
  expect(editorBox).not.toBeNull();
  expect(railBox).not.toBeNull();
  expect(railBox!.width).toBeGreaterThanOrEqual(editorBox!.width * 0.9);

  await editor.evaluate(element => {
    element.scrollTop = element.scrollHeight;
  });
  await expect(rail).toBeInViewport();
  await page.waitForTimeout(400);
  await page.screenshot({
    path: 'test-results/studio-refinement/scenes-editor-800x600.png',
  });

  await page.getByRole('button', { name: '保存', exact: true }).click();
  await expect
    .poll(() =>
      page.evaluate(() =>
        (window as any).__calls.some((call: { cmd: string }) => call.cmd === 'update_scene')
      )
    )
    .toBe(true);
});

test('代理工作室中屏堆叠后可滚动到现场面板', async ({ page }) => {
  await page.setViewportSize({ width: 1024, height: 768 });
  await page.addInitScript(getMockTauriInitScript());
  await page.goto('/index.html');
  await page.waitForTimeout(800);
  await page.getByRole('button', { name: '故事', exact: true }).first().click();
  await page.getByRole('button', { name: /^代理工作室/ }).click();

  const grid = page.getByTestId('agency-workspace-grid');
  const live = page.getByTestId('agency-live-panel');
  await expect(grid).toBeVisible();
  await grid.evaluate(element => {
    element.scrollTop = element.scrollHeight;
  });
  await expect(live).toBeInViewport();
  await page.waitForTimeout(400);
  await page.screenshot({
    path: 'test-results/studio-refinement/agency-studio-1024x768.png',
  });
});
