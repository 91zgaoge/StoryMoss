#!/usr/bin/env node
/**
 * 截取最新幕后 UI 截图，用于 UI 重构分析。
 * 覆盖 Sidebar + 16 个页面。
 */
const { chromium } = require('playwright');
const path = require('path');
const fs = require('fs');

const OUT = '/tmp/sf_ui_audit';
const BASE = 'http://localhost:5173/index.html';

const VIEWS = [
  'dashboard', 'stories', 'characters', 'scenes',
  'world_building', 'knowledge-graph', 'skills', 'mcp',
  'book-deconstruction', 'tasks', 'foreshadowing', 'narrative-analysis',
  'story-system', 'usage-stats', 'writing-stats', 'settings',
];

(async () => {
  if (!fs.existsSync(OUT)) fs.mkdirSync(OUT, { recursive: true });
  const browser = await chromium.launch({ headless: true });
  const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  const page = await ctx.newPage();

  // 收集 console 错误
  const errors = [];
  page.on('console', msg => {
    if (msg.type() === 'error') errors.push(`[${msg.type()}] ${msg.text()}`);
  });

  await page.goto(BASE, { waitUntil: 'networkidle', timeout: 30000 });
  await page.waitForTimeout(2000);

  for (const v of VIEWS) {
    const file = path.join(OUT, `${v}.png`);
    try {
      // 点击侧边栏导航
      const clicked = await page.evaluate((viewId) => {
        const items = document.querySelectorAll('[data-view], .nav-item, button');
        for (const el of items) {
          const text = (el.textContent || '').trim();
          const dv = el.getAttribute('data-view');
          if (dv === viewId || text.includes(viewId.replace(/-/g, ''))) {
            el.click();
            return true;
          }
        }
        return false;
      }, v);

      if (!clicked) {
        // 用文本匹配
        const labels = {
          'dashboard': '仪表盘', 'stories': '故事', 'characters': '角色',
          'scenes': '场景', 'world_building': '世界构建', 'knowledge-graph': '知识图谱',
          'skills': '技能', 'mcp': 'MCP', 'book-deconstruction': '拆书',
          'tasks': '任务', 'foreshadowing': '伏笔看板', 'narrative-analysis': '叙事分析',
          'story-system': 'Story System', 'usage-stats': '用量统计',
          'writing-stats': '写作统计', 'settings': '设置',
        };
        const label = labels[v];
        if (label) {
          await page.evaluate((lbl) => {
            const items = document.querySelectorAll('button, a, [role="button"], .nav-item');
            for (const el of items) {
              if ((el.textContent || '').trim().includes(lbl)) { el.click(); return; }
            }
          }, label);
        }
      }
      await page.waitForTimeout(1500);
      await page.screenshot({ path: file, fullPage: false });
      console.log(`✅ ${v}`);
    } catch (e) {
      console.log(`❌ ${v}: ${e.message.slice(0, 80)}`);
    }
  }

  // 截侧边栏单独
  try {
    await page.screenshot({ path: path.join(OUT, 'sidebar.png'), fullPage: false });
  } catch {}

  console.log(`\n截图完成: ${OUT}/`);
  if (errors.length) {
    console.log(`\nConsole 错误 (${errors.length}):`);
    errors.slice(0, 10).forEach(e => console.log(`  ${e.slice(0, 120)}`));
  }
  await browser.close();
})();
