#!/usr/bin/env python3
"""Extract inline Rust migrations from connection.rs into numbered files."""

import re
from pathlib import Path

ROOT = Path("/Users/yuzaimu/projects/StoryForge")
SRC = ROOT / "src-tauri/src/db/connection.rs"
OUT_DIR = ROOT / "src-tauri/src/db/migrations"

# Manual clean descriptions keyed by record_migration version.
DESCRIPTIONS = {
    28: "scene_structure_fields",
    29: "chat_sessions_and_messages",
    30: "story_runtime_states",
    31: "story_style_configs",
    32: "scene_style_blend_override",
    33: "user_auth_system",
    34: "subscription_real_user_id",
    35: "story_outlines",
    36: "character_relationships",
    37: "scene_foreshadowing_ids",
    38: "chapter_scene_mapping",
    39: "workflow_instances",
    40: "pending_vector_indexes",
    41: "story_metadata",
    42: "scene_characters",
    43: "scene_character_actions",
    44: "plan_templates",
    45: "story_contracts",
    46: "scene_commits",
    47: "memory_items",
    48: "chapter_reading_power",
    49: "chase_debt",
    50: "override_contracts",
    51: "review_issues",
    52: "genre_profiles",
    53: "chapter_writing_phase",
    54: "ingest_jobs",
    55: "feature_usage_logs",
    56: "blueprints",
    57: "drafts",
    58: "revisions",
    59: "reviews",
    60: "post_process_runs",
    61: "character_dynamic_state_fields",
    62: "llm_calls",
    63: "ai_usage_quota_offline_grace",
    64: "style_snapshots",
    65: "narrative_tables_status",
    66: "genesis_runs",
    67: "scene_commits_chapter_id",
    68: "reference_to_narrative_migration",
    69: "rename_chapter_commits",
    70: "drop_chapters_scene_id",
    71: "scene_divider_nodes",
    72: "entity_mentions",
    73: "narrative_events",
    74: "narrative_threads",
    75: "narrative_structure_positions",
    76: "narrative_structure",
    77: "narrative_chunks",
    78: "scene_litseg_fields",
    79: "foreshadowing_tracker_events",
    80: "character_states_arc",
    81: "story_outlines_analyzed_structure",
    82: "conflict_escalations",
    83: "drop_redundant_litseg_tables",
    85: "reference_scenes_litseg_fields",
    86: "reference_books_analyzed_structure",
    87: "genre_profiles_typical_structure",
    88: "stories_genre_profile_id",
    89: "llm_calls_model_health",
    90: "text_annotations_metadata_severity",
    91: "model_capability_profile",
    92: "beat_cards_story_engines_pressure_relationships",
    93: "prompt_overrides",
    94: "drop_dead_tables",
    96: "genre_profiles_recommended_assets",
    97: "stories_reference_book_id",
    98: "narrative_tables_status_v2",
    99: "source_and_auto_generated_columns",
}


def extract_blocks(body: str):
    """Extract each `if current_version < X { ... }` block."""
    lines = body.splitlines(keepends=True)
    blocks = []
    current = []
    version = None
    started = False

    for line in lines:
        m = re.search(r"if current_version < (\d+) \{", line)
        if m:
            if current:
                blocks.append((version, "".join(current)))
                current = []
            started = True
            version = int(m.group(1)) + 1

        if not started:
            continue

        current.append(line)

        rm = re.search(r"record_migration\(conn, (\d+)\)\?;", line)
        if rm:
            version = int(rm.group(1))

    if current:
        text = "".join(current).strip()
        if text:
            blocks.append((version, text + "\n"))

    return blocks


def extract_inner_body(block: str) -> str:
    """Remove outer `if current_version < X { ... }` wrapper and unindent one level."""
    lines = block.splitlines(keepends=True)
    depth = 0
    inner = []
    in_if = False
    for line in lines:
        stripped = line.strip()
        if re.match(r"if current_version < \d+ \{", stripped):
            depth = 1
            in_if = True
            continue
        if not in_if:
            continue
        # Count braces on this line, but ignore braces inside string literals is not needed
        # because migration blocks only use braces for control flow and SQL strings don't contain them.
        opens = line.count("{")
        closes = line.count("}")
        new_depth = depth + opens - closes
        if new_depth == 0 and depth > 0:
            depth = new_depth
            continue
        depth = new_depth
        if depth > 0:
            # The if-body is indented 4 spaces relative to function body.
            # The generated `apply` function body is indented 4 spaces, so we
            # want these lines to keep their relative 4/8-space indentation.
            # Strip exactly the 4 spaces that the `if` block added.
            if line.startswith("        "):
                inner.append(line[4:])
            else:
                inner.append(line)
    return "".join(inner)


def main():
    content = SRC.read_text(encoding="utf-8")
    start_marker = "fn run_migrations(conn: &mut rusqlite::Connection) -> Result<(), rusqlite::Error> {"
    start = content.find(start_marker)
    assert start != -1, "run_migrations not found"
    body_start = content.find("\n", start) + 1
    end = content.find("\n    Ok(())\n}\n", body_start)
    assert end != -1, "run_migrations end not found"
    body = content[body_start:end]

    blocks = extract_blocks(body)
    print(f"Found {len(blocks)} migration blocks")

    files_created = []
    for version, block in blocks:
        assert version is not None, "block without version"
        desc = DESCRIPTIONS.get(version, f"migration_{version}")
        filename = f"V{version:03}__{desc}.rs"

        inner = extract_inner_body(block)
        # The runner records the migration after apply() returns, so drop the
        # inline record_migration calls from the original function.
        inner = re.sub(
            r"\s*record_migration\(conn, \d+\)\?;\n",
            "\n",
            inner,
        )
        # Ensure trailing newline and final Ok(())
        if inner and not inner.endswith("\n"):
            inner += "\n"
        inner = inner.rstrip() + "\n        Ok(())\n"

        file_content = f"""use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {{
    fn version(&self) -> i32 {{
        {version}
    }}

    fn description(&self) -> &'static str {{
        "{desc.replace('_', ' ')}"
    }}

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {{
{inner}    }}
}}
"""
        out_path = OUT_DIR / filename
        out_path.write_text(file_content, encoding="utf-8")
        files_created.append(filename)

    print(f"Created {len(files_created)} files")
    for f in files_created:
        print(f"  {f}")


if __name__ == "__main__":
    main()
