use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        91
    }

    fn description(&self) -> &'static str {
        "model capability profile"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS model_capability_profile (
            model_id              TEXT PRIMARY KEY,
            short_ttfb_ms_p50     INTEGER,
            short_ttfb_ms_p95     INTEGER,
            long_ttfb_ms_p50      INTEGER,
            long_ttfb_ms_p95      INTEGER,
            sustained_tps         REAL,
            short_output_tps      REAL,
            success_rate_24h      REAL,
            last_full_benchmark_at INTEGER,
            last_health_probe_at  INTEGER,
            benchmark_sample_count INTEGER NOT NULL DEFAULT 0,
            status                TEXT NOT NULL DEFAULT 'unknown',
            status_reason         TEXT,
            capability_score      REAL,
            speed_score           REAL,
            quality_score         REAL,
            created_at            INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
            updated_at            INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        );

        CREATE INDEX IF NOT EXISTS idx_capability_status
            ON model_capability_profile(status);
        CREATE INDEX IF NOT EXISTS idx_capability_score
            ON model_capability_profile(capability_score);
        ",
        )?;
        Ok(())
    }
}
