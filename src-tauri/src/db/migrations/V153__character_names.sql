-- v0.80.1: character name ledger + address rules.
CREATE TABLE IF NOT EXISTS character_names (
    character_id TEXT PRIMARY KEY,
    story_id TEXT NOT NULL,
    formal TEXT NOT NULL,
    surname TEXT,
    generation TEXT,
    given TEXT,
    courtesy TEXT,
    art TEXT,
    childhood TEXT,
    nickname TEXT,
    aliases_json TEXT NOT NULL DEFAULT '[]',
    titles_json TEXT NOT NULL DEFAULT '[]',
    taboo_json TEXT NOT NULL DEFAULT '[]',
    address_rules_json TEXT NOT NULL DEFAULT '[]',
    name_story TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_character_names_story
    ON character_names (story_id);
