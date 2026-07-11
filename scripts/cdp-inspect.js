#!/usr/bin/env node
/**
 * StoryMoss CDP 页面检查与截图脚本
 * 使用 Playwright + Chrome DevTools Protocol 遍历应用所有视图，
 * 捕获截图并提取关键交互元素信息，用于生成产品说明文档。
 */

const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');

const OUTPUT_DIR = path.join(__dirname, '..', 'docs', 'product-screenshots');
const APP_URL = process.env.APP_URL || 'http://localhost:5173/index.html';
const FRONTSTAGE_URL = process.env.FRONTSTAGE_URL || 'http://localhost:5173/frontstage.html';

const VIEWS = [
  { id: 'dashboard', name: '仪表盘', description: '应用首页，展示故事统计与快捷入口' },
  { id: 'stories', name: '故事', description: '管理所有故事项目' },
  { id: 'characters', name: '角色', description: '管理故事角色与关系' },
  { id: 'scenes', name: '场景', description: '管理故事场景与情节' },
  { id: 'world_building', name: '世界构建', description: '构建故事世界观' },
  { id: 'knowledge-graph', name: '知识图谱', description: '可视化故事知识网络' },
  { id: 'skills', name: '技能', description: '配置 AI 创作技能' },
  { id: 'mcp', name: 'MCP', description: '模型上下文协议配置' },
  { id: 'book-deconstruction', name: '拆书', description: '拆解参考书籍结构' },
  { id: 'tasks', name: '任务', description: '查看与管理后台任务' },
  { id: 'foreshadowing', name: '伏笔看板', description: '管理故事伏笔与回收' },
  { id: 'narrative-analysis', name: '叙事分析', description: '分析叙事结构' },
  { id: 'story-system', name: 'Story System', description: '故事系统配置' },
  { id: 'usage-stats', name: '用量统计', description: '查看 AI 用量' },
  { id: 'writing-stats', name: '写作统计', description: '查看写作数据统计' },
  { id: 'settings', name: '设置', description: '应用与模型设置' },
];

function ensureDir(dir) {
  if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });
}

function sanitizeFilename(name) {
  return name.replace(/[^a-zA-Z0-9\u4e00-\u9fa5_-]/g, '_');
}

async function sleep(ms) {
  return new Promise(r => setTimeout(r, ms));
}

async function extractInteractiveElements(cdpSession) {
  // 使用 CDP Runtime.evaluate 执行 DOM 查询，提取按钮、链接、输入框等
  const { result } = await cdpSession.send('Runtime.evaluate', {
    expression: `
      (() => {
        const elements = [];
        const selector = 'button, a, [role="button"], [role="link"], input, textarea, select, [contenteditable="true"], .ProseMirror, [data-interactive]';
        document.querySelectorAll(selector).forEach((el, idx) => {
          const rect = el.getBoundingClientRect();
          if (rect.width === 0 || rect.height === 0) return;
          const text = (el.innerText || el.textContent || el.value || el.placeholder || el.getAttribute('aria-label') || '').trim().slice(0, 80);
          if (!text && !el.id) return;
          elements.push({
            tag: el.tagName.toLowerCase(),
            text,
            ariaLabel: el.getAttribute('aria-label') || undefined,
            title: el.title || undefined,
            className: el.className?.slice?.(0, 100) || '',
            id: el.id || undefined,
            href: el.href || undefined,
            type: el.type || undefined,
            disabled: el.disabled || undefined,
            rect: { x: Math.round(rect.x), y: Math.round(rect.y), w: Math.round(rect.width), h: Math.round(rect.height) },
          });
        });
        return elements.slice(0, 80);
      })()
    `,
    returnByValue: true,
  });
  return result.value || [];
}

async function captureView(page, cdpSession, view) {
  console.log(`📸 正在捕获视图: ${view.name} (${view.id})`);

  // 通过 Sidebar 按钮切换视图：先找到所有导航按钮，按顺序点击
  const navItems = [
    '仪表盘', '故事', '角色', '世界构建', '场景', '知识图谱',
    '技能', 'MCP', '拆书', '任务', '伏笔看板', '叙事分析',
    'Story System', '用量统计', '写作统计', '设置'
  ];
  const targetIndex = navItems.indexOf(view.name);

  if (targetIndex >= 0) {
    // 找到 Sidebar 的 nav 区域内的所有按钮，按索引点击
    await page.evaluate((idx) => {
      const nav = document.querySelector('aside nav');
      if (nav) {
        const buttons = nav.querySelectorAll('button');
        if (buttons[idx]) buttons[idx].click();
      }
    }, targetIndex);
  }

  // 等待渲染完成
  await sleep(2000);

  // 截图
  const filenameBase = `${String(VIEWS.indexOf(view) + 1).padStart(2, '0')}_${sanitizeFilename(view.id)}`;
  const screenshotPath = path.join(OUTPUT_DIR, `${filenameBase}.png`);
  await page.screenshot({ path: screenshotPath, fullPage: true });

  // 提取交互元素
  const elements = await extractInteractiveElements(cdpSession);

  // 获取页面标题
  const title = await page.title();

  // 保存结构化数据
  const dataPath = path.join(OUTPUT_DIR, `${filenameBase}.json`);
  const data = {
    viewId: view.id,
    viewName: view.name,
    description: view.description,
    title,
    screenshot: path.basename(screenshotPath),
    capturedAt: new Date().toISOString(),
    elements,
    pageText: (await page.locator('main, [role="main"], body').first().innerText({ timeout: 2000 }).catch(() => '')).slice(0, 2000),
  };
  fs.writeFileSync(dataPath, JSON.stringify(data, null, 2), 'utf-8');

  console.log(`   ✅ 已保存: ${screenshotPath}, ${elements.length} 个交互元素`);
  return data;
}

async function captureFrontstage(browser) {
  console.log('🎭 正在捕获幕前界面 (Frontstage)');
  const page = await browser.newPage();
  const cdpSession = await page.context().newCDPSession(page);

  await page.goto(FRONTSTAGE_URL, { waitUntil: 'networkidle' });
  await sleep(2000);

  const screenshotPath = path.join(OUTPUT_DIR, '00_frontstage.png');
  await page.screenshot({ path: screenshotPath, fullPage: true });

  const elements = await extractInteractiveElements(cdpSession);
  const title = await page.title();

  const dataPath = path.join(OUTPUT_DIR, '00_frontstage.json');
  const data = {
    viewId: 'frontstage',
    viewName: '幕前写作',
    description: '沉浸式写作界面，专注创作正文内容',
    title,
    screenshot: path.basename(screenshotPath),
    capturedAt: new Date().toISOString(),
    elements,
    pageText: (await page.locator('body').first().innerText({ timeout: 2000 }).catch(() => '')).slice(0, 2000),
  };
  fs.writeFileSync(dataPath, JSON.stringify(data, null, 2), 'utf-8');

  console.log(`   ✅ 已保存: ${screenshotPath}, ${elements.length} 个交互元素`);
  await page.close();
  return data;
}

async function main() {
  ensureDir(OUTPUT_DIR);
  console.log(`输出目录: ${OUTPUT_DIR}`);

  // 启动 Chromium 并启用 CDP 远程调试
  const browser = await chromium.launch({
    headless: true,
    args: ['--remote-debugging-port=9223', '--no-sandbox', '--disable-setuid-sandbox'],
  });

  try {
    // 捕获幕前界面
    await captureFrontstage(browser);

    // 捕获幕后所有视图
    const page = await browser.newPage();
    const cdpSession = await page.context().newCDPSession(page);

    await page.setViewportSize({ width: 1920, height: 1080 });
    await page.goto(APP_URL, { waitUntil: 'networkidle' });
    await sleep(2000);

    const results = [];
    for (const view of VIEWS) {
      try {
        const data = await captureView(page, cdpSession, view);
        results.push(data);
      } catch (err) {
        console.error(`   ❌ 捕获失败 ${view.id}:`, err.message);
      }
    }

    // 汇总报告
    const summaryPath = path.join(OUTPUT_DIR, '_summary.json');
    fs.writeFileSync(summaryPath, JSON.stringify({
      capturedAt: new Date().toISOString(),
      totalViews: results.length + 1,
      views: results.map(r => ({ id: r.viewId, name: r.viewName, screenshot: r.screenshot, elements: r.elements.length })),
    }, null, 2), 'utf-8');

    console.log(`\n✅ 全部完成，共捕获 ${results.length + 1} 个视图`);
    console.log(`📁 输出目录: ${OUTPUT_DIR}`);
    await page.close();
  } finally {
    await browser.close();
  }
}

main().catch((err) => {
  console.error('脚本执行失败:', err);
  process.exit(1);
});
