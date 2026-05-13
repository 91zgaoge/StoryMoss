// CINEMA-AI Frontend v2.0
// Complete UI implementation

const app = {
    state: {
        currentView: 'dashboard',
        currentStory: null,
        stories: [],
        characters: [],
        chapters: [],
        skills: [],
        mcpServers: [],
        settings: null,
        isLoading: false,
        lastRefresh: null,
        autoRefreshInterval: null
    },

    // Get Tauri invoke function
    get invoke() {
        if (window.__TAURI__?.invoke) return window.__TAURI__.invoke;
        if (typeof mockTauri !== 'undefined') return mockTauri.invoke;
        return null;
    },

    // Get Tauri event listen function
    get listen() {
        if (window.__TAURI__?.event?.listen) return window.__TAURI__.event.listen;
        return null;
    },

    // Initialize application
    async init() {
        console.log('Initializing CINEMA-AI v2.0...');

        if (!this.invoke) {
            this.showError('Tauri API not found. Please run via Tauri.');
            return;
        }

        try {
            // Load initial data
            await this.loadDashboard();
            this.render();
            lucide.createIcons();

            // Setup event listeners for state sync
            this.setupEventListeners();

            // Setup cleanup on window close
            window.addEventListener('beforeunload', () => {
                this.cleanup();
            });

            // Start auto-refresh mechanism
            this.startAutoRefresh();
        } catch (err) {
            console.error('Initialization failed:', err);
            this.showError('Failed to initialize: ' + err.message);
        }
    },

    // Setup event listeners for state synchronization
    async setupEventListeners() {
        if (!this.listen) {
            console.warn('Tauri event API not available, falling back to polling');
            return;
        }

        try {
            // Listen for story events (using correct event names with hyphens)
            await this.listen('story-created', (event) => {
                console.log('Story created:', event.payload);
                this.handleStoryEvent('created', event.payload);
            });

            await this.listen('story-updated', (event) => {
                console.log('Story updated:', event.payload);
                this.handleStoryEvent('updated', event.payload);
            });

            await this.listen('story-deleted', (event) => {
                console.log('Story deleted:', event.payload);
                this.handleStoryEvent('deleted', event.payload);
            });

            await this.listen('story-selected', (event) => {
                console.log('Story selected:', event.payload);
                this.handleStoryEvent('selected', event.payload);
            });

            // Listen for character events (using correct event names with hyphens)
            await this.listen('character-created', (event) => {
                console.log('Character created:', event.payload);
                this.handleCharacterEvent('created', event.payload);
            });

            await this.listen('character-updated', (event) => {
                console.log('Character updated:', event.payload);
                this.handleCharacterEvent('updated', event.payload);
            });

            await this.listen('character-deleted', (event) => {
                console.log('Character deleted:', event.payload);
                this.handleCharacterEvent('deleted', event.payload);
            });

            // Listen for chapter events (using correct event names with hyphens)
            await this.listen('chapter-created', (event) => {
                console.log('Chapter created:', event.payload);
                this.handleChapterEvent('created', event.payload);
            });

            await this.listen('chapter-updated', (event) => {
                console.log('Chapter updated:', event.payload);
                this.handleChapterEvent('updated', event.payload);
            });

            await this.listen('chapter-deleted', (event) => {
                console.log('Chapter deleted:', event.payload);
                this.handleChapterEvent('deleted', event.payload);
            });

            // Listen for additional sync events
            await this.listen('data-refresh', (event) => {
                console.log('Data refresh requested:', event.payload);
                this.handleDataRefresh(event.payload);
            });

            await this.listen('character-relationships-updated', (event) => {
                console.log('Character relationships updated:', event.payload);
                this.handleCharacterRelationshipsUpdate(event.payload);
            });

            console.log('Event listeners setup complete');
        } catch (err) {
            console.error('Failed to setup event listeners:', err);
        }
    },

    // Handle story events with consistency check
    handleStoryEvent(action, data) {
        switch (action) {
            case 'created':
                this.state.stories.push(data);
                Views.toast(`新故事已创建: ${data.title}`, 'success');
                break;
            case 'updated':
                const storyIndex = this.state.stories.findIndex(s => s.id === data.id);
                if (storyIndex !== -1) {
                    this.state.stories[storyIndex] = data;
                    if (this.state.currentStory && this.state.currentStory.id === data.id) {
                        this.state.currentStory = data;
                    }
                    Views.toast(`故事已更新: ${data.title}`, 'info');
                }
                break;
            case 'deleted':
                this.state.stories = this.state.stories.filter(s => s.id !== data.id);
                if (this.state.currentStory && this.state.currentStory.id === data.id) {
                    this.state.currentStory = null;
                    this.state.characters = [];
                    this.state.chapters = [];
                }
                Views.toast(`故事已删除: ${data.title}`, 'warning');
                break;
            case 'selected':
                // Handle story selection event from backend
                const selectedStory = this.state.stories.find(s => s.id === data.story_id);
                if (selectedStory) {
                    this.state.currentStory = selectedStory;
                    Views.toast(`已选择故事: ${selectedStory.title}`, 'info');
                }
                break;
        }

        // Trigger consistency check after story events
        this.scheduleConsistencyCheck();
        this.render();
        lucide.createIcons();
    },

    // Handle character events with consistency check
    handleCharacterEvent(action, data) {
        // Only update if it's for the current story
        if (!this.state.currentStory || data.story_id !== this.state.currentStory.id) {
            return;
        }

        switch (action) {
            case 'created':
                this.state.characters.push(data);
                Views.toast(`新角色已创建: ${data.name}`, 'success');
                break;
            case 'updated':
                const charIndex = this.state.characters.findIndex(c => c.id === data.id);
                if (charIndex !== -1) {
                    this.state.characters[charIndex] = data;
                    Views.toast(`角色已更新: ${data.name}`, 'info');
                }
                break;
            case 'deleted':
                this.state.characters = this.state.characters.filter(c => c.id !== data.id);
                Views.toast(`角色已删除: ${data.name}`, 'warning');
                break;
        }

        // Trigger consistency check after character events
        this.scheduleConsistencyCheck();

        if (this.state.currentView === 'characters' || this.state.currentView === 'dashboard') {
            this.render();
            lucide.createIcons();
        }
    },

    // Handle chapter events with consistency check
    handleChapterEvent(action, data) {
        // Only update if it's for the current story
        if (!this.state.currentStory || data.story_id !== this.state.currentStory.id) {
            return;
        }

        switch (action) {
            case 'created':
                this.state.chapters.push(data);
                Views.toast(`新章节已创建: ${data.title}`, 'success');
                break;
            case 'updated':
                const chapterIndex = this.state.chapters.findIndex(c => c.id === data.id);
                if (chapterIndex !== -1) {
                    this.state.chapters[chapterIndex] = data;
                    Views.toast(`章节已更新: ${data.title}`, 'info');
                }
                break;
            case 'deleted':
                this.state.chapters = this.state.chapters.filter(c => c.id !== data.id);
                Views.toast(`章节已删除: ${data.title}`, 'warning');
                break;
        }

        // Trigger consistency check after chapter events
        this.scheduleConsistencyCheck();

        if (this.state.currentView === 'chapters' || this.state.currentView === 'dashboard') {
            this.render();
            lucide.createIcons();
        }
    },

    // Load dashboard data (missing method)
    async loadDashboard() {
        await this.loadStories();
        this.state.lastRefresh = Date.now();
    },

    // Load stories data
    async loadStories() {
        this.state.stories = await this.invoke('list_stories');
        this.state.lastRefresh = Date.now();
    },

    // Load characters for current story
    async loadCharacters() {
        if (this.state.currentStory) {
            this.state.characters = await this.invoke('get_story_characters', {
                story_id: this.state.currentStory.id
            });
        } else {
            this.state.characters = [];
        }
        this.state.lastRefresh = Date.now();
    },

    // Load chapters for current story
    async loadChapters() {
        if (this.state.currentStory) {
            this.state.chapters = await this.invoke('get_story_chapters', {
                story_id: this.state.currentStory.id
            });
        } else {
            this.state.chapters = [];
        }
        this.state.lastRefresh = Date.now();
    },

    // Load skills data
    async loadSkills() {
        this.state.skills = await this.invoke('list_skills');
        this.state.lastRefresh = Date.now();
    },

    // Load settings data
    async loadSettings() {
        this.state.settings = await this.invoke('get_settings');
        this.state.lastRefresh = Date.now();
    },

    // Auto-refresh mechanism
    startAutoRefresh() {
        // Refresh every 30 seconds when not actively editing
        this.state.autoRefreshInterval = setInterval(async () => {
            if (!this.state.isLoading && document.visibilityState === 'visible') {
                await this.refreshCurrentView();
            }
        }, 30000);

        // Also refresh when window becomes visible
        document.addEventListener('visibilitychange', async () => {
            if (document.visibilityState === 'visible') {
                const timeSinceRefresh = Date.now() - (this.state.lastRefresh || 0);
                if (timeSinceRefresh > 10000) { // 10 seconds threshold
                    await this.refreshCurrentView();
                }
            }
        });
    },

    // Refresh current view data with smart sync
    async refreshCurrentView() {
        try {
            switch (this.state.currentView) {
                case 'dashboard':
                    await this.loadDashboard();
                    // Also load characters and chapters for current story
                    if (this.state.currentStory) {
                        // Use canonical state for dashboard refresh
                        try {
                            const canonicalState = await this.invoke('get_canonical_state', {
                                story_id: this.state.currentStory.id
                            });
                            this.state.currentStory = canonicalState.story;
                            this.state.characters = canonicalState.characters;
                            this.state.chapters = canonicalState.chapters;
                        } catch (err) {
                            console.warn('Canonical state refresh failed, using individual calls:', err);
                            await this.loadCharacters();
                            await this.loadChapters();
                        }
                    }
                    break;
                case 'stories':
                    await this.loadStories();
                    break;
                case 'characters':
                    await this.loadCharacters();
                    break;
                case 'chapters':
                    await this.loadChapters();
                    break;
                case 'skills':
                    await this.loadSkills();
                    break;
                case 'settings':
                    await this.loadSettings();
                    break;
            }
            this.render();
            lucide.createIcons();
        } catch (err) {
            console.warn('Auto-refresh failed:', err);
        }
    },

    // Force refresh all data using canonical state
    async forceRefresh() {
        this.state.isLoading = true;
        this.render();

        try {
            // Use canonical state sync for comprehensive refresh
            if (this.state.currentStory) {
                const canonicalState = await this.invoke('get_canonical_state', {
                    story_id: this.state.currentStory.id
                });

                // Update state with canonical data
                this.state.currentStory = canonicalState.story;
                this.state.characters = canonicalState.characters;
                this.state.chapters = canonicalState.chapters;

                Views.toast('数据已同步至最新状态', 'success');
            } else {
                await this.loadDashboard();
            }

            await this.loadSkills();
        } catch (err) {
            // Fallback to individual loading if canonical state fails
            console.warn('Canonical state sync failed, falling back:', err);
            await this.loadDashboard();
            if (this.state.currentStory) {
                await this.loadCharacters();
                await this.loadChapters();
            }
            await this.loadSkills();
            Views.toast('数据已刷新', 'success');
        }

        this.state.isLoading = false;
        this.render();
        lucide.createIcons();
    },

    // Load stories
    async loadStories() {
        this.state.stories = await this.invoke('list_stories');
    },

    // Load characters
    async loadCharacters() {
        if (!this.state.currentStory) return;
        this.state.characters = await this.invoke('get_story_characters', {
            story_id: this.state.currentStory.id
        });
    },

    // Load chapters
    async loadChapters() {
        if (!this.state.currentStory) return;
        this.state.chapters = await this.invoke('get_story_chapters', {
            story_id: this.state.currentStory.id
        });
    },

    // Load skills
    async loadSkills() {
        this.state.skills = await this.invoke('get_skills');
    },

    // Load settings
    async loadSettings() {
        this.state.settings = await this.invoke('get_settings');
    },

    // Navigate to view
    async navigate(view) {
        this.state.currentView = view;
        this.state.isLoading = true;
        this.render();

        try {
            // Load data for the specific view
            switch (view) {
                case 'dashboard':
                    await this.loadDashboard();
                    if (this.state.currentStory) {
                        await this.loadCharacters();
                        await this.loadChapters();
                    }
                    break;
                case 'stories':
                    await this.loadStories();
                    break;
                case 'characters':
                    if (!this.state.currentStory) {
                        Views.toast('请先选择一个故事', 'warning');
                        this.state.isLoading = false;
                        this.navigate('stories');
                        return;
                    }
                    await this.loadCharacters();
                    break;
                case 'chapters':
                    if (!this.state.currentStory) {
                        Views.toast('请先选择一个故事', 'warning');
                        this.state.isLoading = false;
                        this.navigate('stories');
                        return;
                    }
                    await this.loadChapters();
                    break;
                case 'skills':
                    await this.loadSkills();
                    break;
                case 'settings':
                    await this.loadSettings();
                    break;
                case 'mcp':
                    // MCP servers data loading if needed
                    break;
            }
        } catch (err) {
            Views.toast('加载数据失败: ' + err.message, 'error');
        }

        this.state.isLoading = false;
        this.render();
        lucide.createIcons();
    },

    // Render main UI
    render() {
        const appEl = document.getElementById('app');

        if (this.state.isLoading && !this.state.stories.length) {
            appEl.innerHTML = this.renderLoading();
            return;
        }

        let content;
        switch (this.state.currentView) {
            case 'dashboard':
                content = Views.dashboard({
                    stories_count: this.state.stories.length,
                    characters_count: this.state.characters.length,
                    chapters_count: this.state.chapters.length,
                    current_story: this.state.currentStory
                });
                break;
            case 'stories':
                content = Views.storiesList(this.state.stories);
                break;
            case 'characters':
                content = Views.characters(this.state.characters);
                break;
            case 'chapters':
                content = Views.chapters(this.state.chapters);
                break;
            case 'skills':
                content = Views.skills(this.state.skills);
                break;
            case 'mcp':
                content = Views.mcpConfig(this.state.mcpServers);
                break;
            case 'settings':
                content = Views.settings(this.state.settings);
                break;
            default:
                content = Views.dashboard({});
        }

        appEl.innerHTML = `
            <div class="flex h-screen">
                ${Views.sidebar(this.state.currentView)}
                <main class="flex-1 overflow-auto p-8">
                    ${this.state.isLoading ? '<div class="flex items-center justify-center h-full"><div class="loading-dots text-2xl">加载中</div></div>' : content}
                </main>
            </div>
        `;
    },

    // Render loading screen
    renderLoading() {
        return `
            <div class="flex h-screen items-center justify-center bg-cinema-950 film-grain">
                <div class="text-center relative">
                    <!-- Cinematic loading animation -->
                    <div class="relative w-24 h-24 mx-auto mb-8">
                        <div class="absolute inset-0 rounded-full border-2 border-cinema-gold/20"></div>
                        <div class="absolute inset-2 rounded-full border-2 border-cinema-gold/30 animate-pulse"></div>
                        <div class="absolute inset-4 rounded-full border-2 border-t-cinema-gold border-r-transparent border-b-cinema-gold/50 border-l-transparent animate-spin" style="animation-duration: 2s;"></div>
                        <div class="absolute inset-0 flex items-center justify-center">
                            <i data-lucide="film" class="w-8 h-8 text-cinema-gold"></i>
                        </div>
                    </div>
                    <h2 class="font-display text-2xl text-white mb-2">正在准备工作室</h2>
                    <p class="font-body text-gray-500 italic">"好戏即将开场..."</p>
                </div>
            </div>
        `;
    },

    // Show error screen - Cinematic
    showError(message) {
        document.getElementById('app').innerHTML = `
            <div class="flex h-screen items-center justify-center bg-cinema-950">
                <div class="film-grain"></div>
                <div class="text-center max-w-md p-10 glass-cinema rounded-2xl border border-red-500/20 relative">
                    <div class="w-20 h-20 rounded-full bg-red-500/10 flex items-center justify-center mx-auto mb-6 border border-red-500/30">
                        <i data-lucide="alert-triangle" class="w-10 h-10 text-red-400"></i>
                    </div>
                    <h1 class="font-display text-3xl font-bold text-white mb-4">初始化失败</h1>
                    <p class="font-body text-gray-400 mb-8 italic">${message}</p>
                    <button onclick="location.reload()" class="group px-8 py-3 bg-gradient-to-r from-red-500 to-red-600 rounded-xl font-body font-semibold text-white hover:shadow-lg hover:shadow-red-500/20 transition-all duration-300">
                        <span class="flex items-center gap-2">
                            <i data-lucide="refresh-cw" class="w-4 h-4 group-hover:rotate-180 transition-transform duration-500"></i>
                            重试
                        </span>
                    </button>
                </div>
            </div>
        `;
        lucide.createIcons();
    },

    // Data consistency check mechanism
    scheduleConsistencyCheck() {
        // Debounce consistency checks to avoid excessive calls
        if (this.consistencyCheckTimeout) {
            clearTimeout(this.consistencyCheckTimeout);
        }

        this.consistencyCheckTimeout = setTimeout(async () => {
            await this.performConsistencyCheck();
        }, 2000); // Check after 2 seconds of inactivity
    },

    async performConsistencyCheck() {
        if (!this.state.currentStory) return;

        try {
            const canonicalState = await this.invoke('get_canonical_state', {
                story_id: this.state.currentStory.id
            });

            // Check for discrepancies
            let hasDiscrepancies = false;

            // Check characters count
            if (this.state.characters.length !== canonicalState.characters.length) {
                hasDiscrepancies = true;
                console.log('Character count mismatch detected');
            }

            // Check chapters count
            if (this.state.chapters.length !== canonicalState.chapters.length) {
                hasDiscrepancies = true;
                console.log('Chapter count mismatch detected');
            }

            // If discrepancies found, sync to canonical state
            if (hasDiscrepancies) {
                this.state.currentStory = canonicalState.story;
                this.state.characters = canonicalState.characters;
                this.state.chapters = canonicalState.chapters;

                console.log('Data synchronized to canonical state');
                Views.toast('数据已自动同步', 'info');
                this.render();
                lucide.createIcons();
            }
        } catch (err) {
            console.warn('Consistency check failed:', err);
        }
    },

    // Cleanup function
    cleanup() {
        if (this.state.autoRefreshInterval) {
            clearInterval(this.state.autoRefreshInterval);
            this.state.autoRefreshInterval = null;
        }

        if (this.consistencyCheckTimeout) {
            clearTimeout(this.consistencyCheckTimeout);
            this.consistencyCheckTimeout = null;
        }
    },

    // Modal management
    showModal(type) {
        const modalContainer = document.getElementById('modal-container');
        let content;

        switch (type) {
            case 'createStory':
                content = Views.createStoryModal();
                break;
            default:
                return;
        }

        modalContainer.innerHTML = content;
        lucide.createIcons();
    },

    closeModal() {
        document.getElementById('modal-container').innerHTML = '';
    },

    // Form handlers
    async handleCreateStory(e) {
        e.preventDefault();
        const formData = new FormData(e.target);

        try {
            await this.invoke('create_story', {
                title: formData.get('title'),
                description: formData.get('description'),
                genre: formData.get('genre')
            });
            this.closeModal();
            Views.toast('故事创建成功', 'success');

            // Auto-refresh stories and dashboard
            await this.loadStories();
            await this.loadDashboard();
            this.navigate('stories');
        } catch (err) {
            Views.toast('创建失败: ' + err.message, 'error');
        }
    },

    async saveSettings(e) {
        e.preventDefault();
        const formData = new FormData(e.target);

        try {
            await this.invoke('save_settings', {
                llm: {
                    provider: formData.get('provider'),
                    api_key: formData.get('api_key'),
                    model: formData.get('model'),
                    temperature: parseFloat(formData.get('temperature')),
                    max_tokens: parseInt(formData.get('max_tokens'))
                }
            });
            Views.toast('设置已保存', 'success');
        } catch (err) {
            Views.toast('保存失败: ' + err.message, 'error');
        }
    },

    // Story selection with enhanced sync
    async selectStory(storyId) {
        const story = this.state.stories.find(s => s.id === storyId);
        if (story) {
            this.state.currentStory = story;
            document.getElementById('current-story-name').textContent = story.title;

            // Use sync_story_data for comprehensive data loading
            try {
                await this.invoke('sync_story_data', { story_id: storyId });

                // Load fresh data after sync
                await this.loadCharacters();
                await this.loadChapters();

                Views.toast(`已选择并同步: ${story.title}`, 'success');
                this.navigate('chapters');
            } catch (err) {
                // Fallback to individual loading if sync fails
                console.warn('Story sync failed, falling back:', err);
                try {
                    await this.loadCharacters();
                    await this.loadChapters();
                    Views.toast(`已选择: ${story.title}`, 'success');
                    this.navigate('chapters');
                } catch (fallbackErr) {
                    Views.toast('加载故事数据失败: ' + fallbackErr.message, 'error');
                }
            }
        }
    },

    // Skill management
    async toggleSkill(skillId, enabled) {
        try {
            if (enabled) {
                await this.invoke('enable_skill', { skill_id: skillId });
            } else {
                await this.invoke('disable_skill', { skill_id: skillId });
            }
            Views.toast(enabled ? '技能已启用' : '技能已禁用', 'success');
        } catch (err) {
            Views.toast('操作失败: ' + err.message, 'error');
        }
    },

    filterSkills(category) {
        // Implement skill filtering
        console.log('Filter skills by:', category);
    },

    // Chapter editing
    selectChapter(chapterId) {
        console.log('Selected chapter:', chapterId);
    },

    // Character editing
    editCharacter(characterId) {
        console.log('Edit character:', characterId);
    },

    // MCP management
    testMcpServer(serverId) {
        Views.toast('测试连接: ' + serverId, 'info');
    },

    deleteMcpServer(serverId) {
        Views.toast('删除服务器: ' + serverId, 'warning');
    },

    // Handle data refresh events
    handleDataRefresh(data) {
        const { story_id, data_type } = data;

        // Only refresh if it's for the current story or global refresh
        if (!story_id || (this.state.currentStory && this.state.currentStory.id === story_id)) {
            switch (data_type) {
                case 'all':
                    // Refresh all data for current story
                    if (this.state.currentStory) {
                        this.loadStoryData(this.state.currentStory.id);
                    }
                    break;
                case 'characters':
                    if (this.state.currentStory) {
                        this.loadCharacters(this.state.currentStory.id);
                    }
                    break;
                case 'chapters':
                    if (this.state.currentStory) {
                        this.loadChapters(this.state.currentStory.id);
                    }
                    break;
                case 'characterRelationships':
                case 'foreshadowings':
                case 'storyOutlines':
                case 'writingStyle':
                    // These are handled by specific refresh logic
                    console.log(`Data refresh for ${data_type} requested`);
                    break;
                default:
                    console.log(`Unknown data refresh type: ${data_type}`);
            }
        }
    },

    // Handle character relationships update
    handleCharacterRelationshipsUpdate(data) {
        const { story_id } = data;

        // Only handle if it's for the current story
        if (this.state.currentStory && this.state.currentStory.id === story_id) {
            // Trigger a refresh of character data to get updated relationships
            this.loadCharacters(story_id);
            Views.toast('角色关系已更新', 'info');
        }
    },

    // Schedule consistency check to avoid too frequent checks
    scheduleConsistencyCheck() {
        if (this.consistencyCheckTimeout) {
            clearTimeout(this.consistencyCheckTimeout);
        }

        this.consistencyCheckTimeout = setTimeout(() => {
            this.performConsistencyCheck();
        }, 1000); // Wait 1 second before checking
    },

    // Perform consistency check between frontend state and backend
    async performConsistencyCheck() {
        if (!this.state.currentStory) return;

        try {
            // Check if current story data is consistent
            const backendStory = await invoke('get_story', { id: this.state.currentStory.id });
            if (backendStory && backendStory.title !== this.state.currentStory.title) {
                console.log('Story title inconsistency detected, syncing...');
                this.state.currentStory = backendStory;
                this.render();
            }

            // Check character count consistency
            const backendCharacters = await invoke('get_story_characters', { storyId: this.state.currentStory.id });
            if (backendCharacters && backendCharacters.length !== this.state.characters.length) {
                console.log('Character count inconsistency detected, syncing...');
                this.state.characters = backendCharacters;
                this.render();
            }

            // Check chapter count consistency
            const backendChapters = await invoke('get_story_chapters', { storyId: this.state.currentStory.id });
            if (backendChapters && backendChapters.length !== this.state.chapters.length) {
                console.log('Chapter count inconsistency detected, syncing...');
                this.state.chapters = backendChapters;
                this.render();
            }

        } catch (error) {
            console.error('Consistency check failed:', error);
        }
    }
};

// Initialize on load
document.addEventListener('DOMContentLoaded', () => app.init());
