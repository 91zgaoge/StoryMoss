import { test, expect } from '@playwright/test';

/**
 * StoryForge 应用测试套件
 * 测试核心功能：双界面、场景管理、版本控制等
 */
test.describe('StoryForge 应用测试', () => {
  
  test.beforeEach(async ({ page }) => {
    // 每个测试前设置视口
    await page.setViewportSize({ width: 1920, height: 1080 });
  });

  test.describe('🎭 幕前界面测试', () => {
    
    test('幕前界面加载和截图', async ({ page }) => {
      await page.goto('/frontstage.html');
      await page.waitForTimeout(3000);
      
      // 截图
      await page.screenshot({ 
        path: 'e2e/screenshots/frontstage_initial.png', 
        fullPage: true 
      });
      
      console.log('✅ 幕前界面已截图');
    });

    test('幕前界面交互元素检查', async ({ page }) => {
      await page.goto('/frontstage.html');
      await page.waitForTimeout(2000);
      
      // 检查是否存在编辑器区域
      const editor = page.locator('.ProseMirror, [contenteditable="true"], .editor');
      const hasEditor = await editor.count() > 0;
      
      if (hasEditor) {
        console.log('✅ 找到编辑器元素');
        await editor.screenshot({ path: 'e2e/screenshots/frontstage_editor.png' });
      } else {
        console.log('⚠️ 未找到编辑器元素，可能是加载中');
      }
      
      // 截图整个页面
      await page.screenshot({ 
        path: 'e2e/screenshots/frontstage_full.png', 
        fullPage: true 
      });
    });
  });

  test.describe('🔧 幕后界面测试', () => {
    
    test('幕后仪表盘加载', async ({ page }) => {
      await page.goto('/index.html');
      await page.waitForTimeout(3000);
      
      await page.screenshot({ 
        path: 'e2e/screenshots/backstage_dashboard.png', 
        fullPage: true 
      });
      
      console.log('✅ 幕后仪表盘已截图');
    });

    test('场景管理页面', async ({ page }) => {
      await page.goto('/index.html#/scenes');
      await page.waitForTimeout(3000);
      
      await page.screenshot({ 
        path: 'e2e/screenshots/scenes_page.png', 
        fullPage: true 
      });
      
      console.log('✅ 场景管理页面已截图');
    });

    test('角色管理页面', async ({ page }) => {
      await page.goto('/index.html#/characters');
      await page.waitForTimeout(3000);
      
      await page.screenshot({ 
        path: 'e2e/screenshots/characters_page.png', 
        fullPage: true 
      });
      
      console.log('✅ 角色管理页面已截图');
    });

    test('故事列表页面', async ({ page }) => {
      await page.goto('/index.html#/stories');
      await page.waitForTimeout(3000);
      
      await page.screenshot({ 
        path: 'e2e/screenshots/stories_page.png', 
        fullPage: true 
      });
      
      console.log('✅ 故事列表页面已截图');
    });
  });

  test.describe('📜 版本管理功能测试', () => {
    
    test('版本时间线组件', async ({ page }) => {
      // 导航到场景页面（假设有版本管理功能）
      await page.goto('/index.html#/scenes');
      await page.waitForTimeout(3000);
      
      // 查找版本相关的元素
      const versionElements = page.locator('text=/version|版本/i');
      const count = await versionElements.count();
      
      console.log(`找到 ${count} 个版本相关元素`);
      
      await page.screenshot({ 
        path: 'e2e/screenshots/version_timeline.png', 
        fullPage: true 
      });
    });
  });

  test.describe('📊 响应式测试', () => {
    
    test('不同分辨率下的幕前界面', async ({ page }) => {
      const viewports = [
        { width: 1920, height: 1080, name: 'desktop' },
        { width: 1366, height: 768, name: 'laptop' },
        { width: 768, height: 1024, name: 'tablet' },
      ];

      for (const viewport of viewports) {
        await page.setViewportSize(viewport);
        await page.goto('/frontstage.html');
        await page.waitForTimeout(2000);
        
        await page.screenshot({
          path: `e2e/screenshots/frontstage_${viewport.name}.png`,
          fullPage: true
        });
        
        console.log(`✅ ${viewport.name} 分辨率截图完成`);
      }
    });
  });

  test.describe('🎯 功能交互测试', () => {

    test('页面导航流畅度', async ({ page }) => {
      await page.goto('/index.html');
      await page.waitForTimeout(2000);

      // 测试导航到不同页面
      const pages = ['#/stories', '#/characters', '#/scenes', '#/settings'];

      for (const route of pages) {
        const startTime = Date.now();
        await page.goto(`/index.html${route}`);
        await page.waitForLoadState('networkidle');
        const loadTime = Date.now() - startTime;

        console.log(`页面 ${route} 加载时间: ${loadTime}ms`);

        await page.screenshot({
          path: `e2e/screenshots/nav_${route.replace('#/', '')}.png`,
          fullPage: true
        });
      }
    });
  });

  test.describe('💾 数据持久化核心断言', () => {

    test('保存章节后重进 Frontstage，内容仍存在', async ({ page }) => {
      const TEST_CONTENT = '这是E2E测试内容，保存后应仍然存在。';

      // 捕获控制台错误，便于调试
      const consoleErrors: string[] = [];
      page.on('console', (msg) => {
        if (msg.type() === 'error') {
          consoleErrors.push(msg.text());
        }
      });
      page.on('pageerror', (err) => {
        consoleErrors.push(err.message);
      });

      // 注入 mock Tauri API，模拟后端数据持久化
      await page.addInitScript(() => {
        let mockContent = '';
        const mockChapter = {
          id: 'test-chapter-1',
          story_id: 'test-story-1',
          title: '测试章节',
          chapter_number: 1,
          content: ''
        };

        (window as any).__TAURI_INTERNALS__ = {
          invoke: async (cmd: string, args?: any) => {
            if (cmd === 'list_stories') {
              return [{ id: 'test-story-1', title: '测试故事' }];
            }
            if (cmd === 'get_story_chapters') {
              mockChapter.content = mockContent;
              return [mockChapter];
            }
            if (cmd === 'get_story_scenes') {
              return [];
            }
            if (cmd === 'get_chapter') {
              mockChapter.content = mockContent;
              return mockChapter;
            }
            if (cmd === 'update_chapter') {
              mockContent = args?.content || '';
              mockChapter.content = mockContent;
              return null;
            }
            if (cmd === 'notify_backstage_content_changed') {
              return null;
            }
            if (cmd === 'get_subscription_status') {
              return { tier: 'free', status: 'active', daily_used: 0, daily_limit: 10, quota_resets_at: '' };
            }
            if (cmd === 'get_quota_detail') {
              return { auto_write_used: 0, auto_write_limit: 10, auto_revise_used: 0, auto_revise_limit: 10 };
            }
            if (cmd === 'check_auto_write_quota' || cmd === 'check_auto_revise_quota') {
              return { allowed: true, remaining: 10, daily_limit: 10, daily_used: 0 };
            }
            if (cmd === 'plugin:event|listen') {
              return () => {};
            }
            // 其他命令静默返回 null，避免未定义错误阻断 UI
            return null;
          },
          transformCallback: (callback: any) => {
            const id = Math.random().toString(36).substring(2);
            (window as any)[`__tauri_callback_${id}`] = callback;
            return id;
          }
        };
      });

      await page.goto('/frontstage.html');
      await page.waitForTimeout(3000);

      // 断言编辑器已加载（增加超时到 10 秒，给异步初始化更多时间）
      const editor = page.locator('.ProseMirror, [contenteditable="true"]').first();
      try {
        await expect(editor).toBeVisible({ timeout: 10000 });
      } catch (e) {
        // 失败时输出控制台错误和截图，便于调试
        console.error('=== Console errors ===', consoleErrors);
        await page.screenshot({ path: 'e2e/screenshots/frontstage_editor_failed.png', fullPage: true });
        throw e;
      }

      // 在编辑器中输入内容
      await editor.click();
      await editor.fill(TEST_CONTENT);

      // 等待自动保存触发（FrontstageApp 中 autoSave 的 debounce 为 2000ms）
      await page.waitForTimeout(3500);

      // 断言编辑器中确实包含输入内容
      const textBeforeReload = await editor.innerText();
      expect(textBeforeReload).toContain(TEST_CONTENT);

      // 刷新页面模拟"重进 Frontstage"
      await page.reload();
      await page.waitForTimeout(3000);

      // 重新获取编辑器并断言内容仍然存在
      const editorAfterReload = page.locator('.ProseMirror, [contenteditable="true"]').first();
      try {
        await expect(editorAfterReload).toBeVisible({ timeout: 10000 });
      } catch (e) {
        console.error('=== Console errors after reload ===', consoleErrors);
        await page.screenshot({ path: 'e2e/screenshots/frontstage_reload_failed.png', fullPage: true });
        throw e;
      }
      const textAfterReload = await editorAfterReload.innerText();
      expect(textAfterReload).toContain(TEST_CONTENT);

      console.log('✅ 保存后重进 Frontstage，内容持久化验证通过');
    });
  });
});
