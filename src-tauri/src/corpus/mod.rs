pub mod commands;
pub mod models;
pub mod repository;
pub mod service;

use crate::db::DbPool;

/// 把作者固定的语料渲染成续写只读块。没有 pinned 语料时返回 None。
/// 只取摘要/提炼要点/短正文，最多 4 条、总长 ≤1200 字，避免顶掉正文上下文。
pub fn pinned_context_block(pool: &DbPool, story_id: &str) -> Option<String> {
    let repo = repository::CorpusRepository::new(pool.clone());
    let items = repo.list_pinned(story_id, 6).ok()?;
    if items.is_empty() {
        return None;
    }
    let mut lines =
        vec!["【固定语料（作者固定用于本篇；只可引用/化用，不得改写其设定）】".to_string()];
    let mut total = lines[0].chars().count();
    for item in items.into_iter().take(4) {
        let mut body = String::new();
        if let Ok(Some(detail)) = repo.get(&item.id) {
            if let Some(d) = detail.distilled {
                let mut bits: Vec<String> = Vec::new();
                bits.extend(d.facts.into_iter().take(2));
                bits.extend(d.world_hooks.into_iter().take(2));
                bits.extend(d.adaptation_angles.into_iter().take(1));
                if !bits.is_empty() {
                    body = bits.join("；");
                }
            }
            if body.trim().is_empty() {
                body = detail.summary;
            }
            if body.trim().is_empty() {
                body = detail.content.chars().take(180).collect();
            }
        }
        if body.trim().is_empty() {
            body = item.summary;
        }
        let body = body.trim();
        if body.is_empty() {
            continue;
        }
        let line = format!(
            "- 《{}》[{}]：{}",
            item.title,
            item.kind,
            body.chars().take(220).collect::<String>()
        );
        let len = line.chars().count();
        if total + len > 1200 {
            break;
        }
        total += len;
        lines.push(line);
    }
    if lines.len() == 1 {
        None
    } else {
        Some(lines.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::models::CorpusUpdate;

    fn setup() -> (DbPool, String) {
        let pool = crate::db::create_test_pool().unwrap();
        let story = crate::db::repositories::StoryRepository::new(pool.clone())
            .create(crate::db::dto::CreateStoryRequest {
                title: "固定语料测试".into(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();
        (pool, story.id)
    }

    #[test]
    fn pinned_block_only_includes_pinned_items() {
        let (pool, story_id) = setup();
        let repo = repository::CorpusRepository::new(pool.clone());
        let pinned = repo
            .create(
                &story_id,
                "article",
                "潮汐禁令",
                "潮汐禁令规定每月朔日不得出海，违者船毁人亡。",
                "潮汐禁令：朔日禁海",
                None,
                None,
                vec![],
            )
            .unwrap();
        repo.update(
            &pinned,
            &CorpusUpdate {
                pinned: Some(true),
                ..Default::default()
            },
        )
        .unwrap();
        repo.create(
            &story_id,
            "article",
            "无关素材",
            "一段不固定的素材。",
            "无关素材",
            None,
            None,
            vec![],
        )
        .unwrap();
        let block = pinned_context_block(&pool, &story_id).expect("block");
        assert!(block.contains("固定语料"));
        assert!(block.contains("潮汐禁令"));
        assert!(!block.contains("无关素材"));
    }

    #[test]
    fn no_pinned_returns_none() {
        let (pool, story_id) = setup();
        assert!(pinned_context_block(&pool, &story_id).is_none());
    }
}
