-- RUST_LOGIC: This migration contained conditional Rust logic for idempotency checks.
-- The SQL statements below are the unconditional equivalents.

CREATE TABLE reference_books (
                    id TEXT PRIMARY KEY,
                    title TEXT NOT NULL,
                    author TEXT,
                    genre TEXT,
                    word_count INTEGER,
                    file_format TEXT,
                    file_hash TEXT UNIQUE,
                    file_path TEXT,
                    world_setting TEXT,
                    plot_summary TEXT,
                    story_arc TEXT,
                    analysis_status TEXT NOT NULL DEFAULT 'pending',
                    analysis_progress INTEGER DEFAULT 0,
                    analysis_error TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );
CREATE INDEX idx_ref_books_hash ON reference_books(file_hash);
CREATE INDEX idx_ref_books_status ON reference_books(analysis_status);
CREATE TABLE reference_characters (
                    id TEXT PRIMARY KEY,
                    book_id TEXT NOT NULL,
                    name TEXT NOT NULL,
                    role_type TEXT,
                    personality TEXT,
                    appearance TEXT,
                    relationships TEXT,
                    key_scenes TEXT,
                    importance_score REAL,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY (book_id) REFERENCES reference_books(id) ON DELETE CASCADE
                );
CREATE INDEX idx_ref_characters_book ON reference_characters(book_id);
CREATE TABLE reference_scenes (
                    id TEXT PRIMARY KEY,
                    book_id TEXT NOT NULL,
                    sequence_number INTEGER NOT NULL,
                    title TEXT,
                    summary TEXT,
                    characters_present TEXT,
                    key_events TEXT,
                    conflict_type TEXT,
                    emotional_tone TEXT,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY (book_id) REFERENCES reference_books(id) ON DELETE CASCADE
                );
CREATE INDEX idx_ref_scenes_book ON reference_scenes(book_id);
