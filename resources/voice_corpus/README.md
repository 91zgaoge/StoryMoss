# 人类语料校准样本（P2-7，v0.72.0）

供 `story_system::human_voice` 的语料校准探针使用：

```bash
cd src-tauri
cargo test --lib corpus_calibration -- --ignored --nocapture
# 可选：STORYMOSS_VOICE_CORPUS=/path/to/dir STORYMOSS_VOICE_OUT=/tmp/report.json
```

| 文件 | 来源 | 说明 |
|---|---|---|
| `pg25328_豆棚閒話.txt` | Project Gutenberg #25328 | 清·艾衲居士，白话短篇集 |
| `pg24230_今古奇觀.txt` | Project Gutenberg #24230 | 明·抱瓮老人辑，白话短篇集 |
| `pg25393_施公案.txt` | Project Gutenberg #25393 | 清·公案小说（对话密集） |
| `pg23938_木蘭奇女傳.txt` | Project Gutenberg #23938 | 清·章回小说 |
| `pg25422_玉樓春.txt` | Project Gutenberg #25422 | 清·才子佳人小说 |
| `pg24185_比目魚.txt` | Project Gutenberg #24185 | 李渔，白话小说 |

- 每篇取**正文前 2 万字符**（非全文），仅用于统计分布与规则误报率测量。
- 均为公有领域作品（PG 收录，作者身后逾百年）；此处保留 PG 的公有领域声明精神，
  仅作测试语料，不用于分发。
- **register 边界**：这批语料是明清白话，与现代网文的语域不同（更少语气词、更多
  文言残留、段落更整）。它给出的是「人类文本在这些规则上的下限参照」；阈值若要
  按现代语域调整，需要作者自己的旧稿（未开源，故不在仓库内）。
- 结论与数据见 `docs/audits/2026-10-14-human-voice-corpus-calibration.md`。
