// Mock Tauri API for web demo
const mockTauri = {
  invoke: async (cmd, args) => {
    console.log(`Mock invoke: ${cmd}`, args);
    switch (cmd) {
      case "get_state":
        return {
          metadata: {
            title: "我的小说",
            current_chapter: 5,
            last_updated: new Date().toISOString(),
          },
          characters: {
            char_001: {
              name: "李明",
              base_profile: { background: "前特种兵" },
              dynamic_traits: [{ trait: "多疑", confidence: 0.8 }],
            },
            char_002: {
              name: "小红",
              base_profile: { background: "记者" },
              dynamic_traits: [],
            },
          },
          writing_style: { tone: "dark", pacing: "medium" },
          quality_metrics: { consistency_score: 0.95 },
        };
      case "generate_chapter":
        await new Promise((r) => setTimeout(r, 1500));
        return {
          chapter_number: args.chapterNumber,
          content: `# 第${args.chapterNumber}章\n\n${args.outline}\n\n这是一个AI生成的示例章节内容...`,
          metadata: { word_count: 1500, model_used: "gpt-4", cost: 0.05 },
        };
      default:
        return {};
    }
  },
};
