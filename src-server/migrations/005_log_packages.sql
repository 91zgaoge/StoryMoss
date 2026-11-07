-- v0.89.0: 用户日志包上传（显式授权、防重复、3 天保留）

CREATE TABLE IF NOT EXISTS log_packages (
    id UUID PRIMARY KEY,
    sha256 TEXT NOT NULL UNIQUE,
    filename TEXT NOT NULL,
    stored_path TEXT NOT NULL,
    size_bytes BIGINT NOT NULL,
    app_version TEXT,
    platform TEXT,
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    uploaded_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_log_packages_uploaded_at
    ON log_packages (uploaded_at DESC);
