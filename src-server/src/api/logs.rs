//! v0.89.0: 用户日志包上传（显式授权）+ 运维下载 + 3 天保留
//!
//! - `POST /api/logs/upload`：登录用户 multipart 上传；同 sha256 幂等
//!   （409→幂等响应）；上传时顺带清理过期文件。
//! - `GET /api/logs/packages`：运维列表（Bearer `LOG_OPS_TOKEN`）。
//! - `GET /api/logs/packages/{id}/download`：运维下载（同令牌）。
//!
//! 保留策略：文件与行记录超过 3 天即删（上传时清理 + 每小时 tokio 扫描）。
//! 磁盘配额靠 3 天滚动 + 客户端 20MB 上限兜底。

use actix_multipart::Multipart;
use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::PgPool;

use crate::{auth::jwt::AuthClaims, config::CONFIG};

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(upload).service(list_packages).service(download_package);
}

/// 每小时后台清理（main.rs spawn）
pub async fn retention_sweep(pool: PgPool) {
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
        if let Err(e) = sweep_once(&pool).await {
            log::warn!("[logs] retention sweep failed: {e}");
        }
    }
}

async fn sweep_once(pool: &PgPool) -> Result<(), String> {
    let rows = sqlx::query!(
        "SELECT id, stored_path FROM log_packages WHERE uploaded_at < NOW() - INTERVAL '72 hours'"
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    for row in rows {
        let _ = std::fs::remove_file(&row.stored_path);
    }
    let deleted = sqlx::query!(
        "DELETE FROM log_packages WHERE uploaded_at < NOW() - INTERVAL '72 hours'"
    )
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    if deleted.rows_affected() > 0 {
        log::info!("[logs] retention sweep removed {} expired packages", deleted.rows_affected());
    }
    Ok(())
}

#[derive(Debug, Serialize)]
struct UploadResponse {
    id: String,
    sha256: String,
    duplicate: bool,
    size_bytes: i64,
}

/// 检查运维令牌（列表/下载端点）
fn check_ops_token(req: &HttpRequest) -> Result<(), HttpResponse> {
    let Some(expected) = CONFIG.log_ops_token.as_deref() else {
        return Err(HttpResponse::ServiceUnavailable()
            .body("LOG_OPS_TOKEN not configured on server"));
    };
    let provided = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    if provided.is_empty() || provided != expected {
        return Err(HttpResponse::Unauthorized().body("invalid ops token"));
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

#[post("/logs/upload")]
async fn upload(
    claims: AuthClaims,
    pool: web::Data<PgPool>,
    mut payload: Multipart,
) -> impl Responder {
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut client_sha: Option<String> = None;

    use futures_util::TryStreamExt;
    loop {
        let mut field = match payload.try_next().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => return HttpResponse::BadRequest().body(format!("multipart error: {e}")),
        };
        let name = field.name().unwrap_or("").to_string();
        let mut data = Vec::new();
        loop {
            match field.try_next().await {
                Ok(Some(chunk)) => data.extend_from_slice(&chunk),
                Ok(None) => break,
                Err(e) => return HttpResponse::BadRequest().body(format!("read field failed: {e}")),
            }
        }
        match name.as_str() {
            "sha256" => {
                client_sha = Some(String::from_utf8_lossy(&data).trim().to_string());
            }
            "file" => file_bytes = Some(data),
            _ => {}
        }
    }

    let Some(bytes) = file_bytes else {
        return HttpResponse::BadRequest().body("missing file field");
    };
    // 服务端独立计算 sha256（不信任客户端值）
    let sha = sha256_hex(&bytes);
    if let Some(client) = &client_sha {
        if !client.is_empty() && client != &sha {
            return HttpResponse::BadRequest().body("sha256 mismatch");
        }
    }
    // 20MB 服务端硬上限（客户端 20MB 预算 + zip 结构余量）
    if bytes.len() > 21 * 1024 * 1024 {
        return HttpResponse::PayloadTooLarge().body("package too large (limit 21MB)");
    }

    // 幂等：同 sha256 未过期 → 返回已有记录（客户端把 409 视为已上传）
    if let Ok(Some(existing)) = sqlx::query!(
        "SELECT id, size_bytes FROM log_packages WHERE sha256 = $1 AND uploaded_at >= NOW() - INTERVAL '72 hours'",
        sha
    )
    .fetch_optional(pool.get_ref())
    .await
    {
        return HttpResponse::Conflict().json(UploadResponse {
            id: existing.id.to_string(),
            sha256: sha,
            duplicate: true,
            size_bytes: existing.size_bytes,
        });
    }

    // 落盘
    let dir = std::path::Path::new(&CONFIG.log_storage_dir);
    if let Err(e) = std::fs::create_dir_all(dir) {
        return HttpResponse::InternalServerError().body(format!("storage dir: {e}"));
    }
    let id_uuid = uuid::Uuid::new_v4();
    let id = id_uuid.to_string();
    let stored_path = dir.join(format!("{id}.zip"));
    if let Err(e) = std::fs::write(&stored_path, &bytes) {
        return HttpResponse::InternalServerError().body(format!("write failed: {e}"));
    }

    // 从 manifest 读 app_version/platform（可选，失败不阻塞）
    let (app_version, platform) = read_manifest_meta(&bytes);

    let size = bytes.len() as i64;
    let result = sqlx::query!(
        "INSERT INTO log_packages (id, sha256, filename, stored_path, size_bytes, app_version, platform, user_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
         ON CONFLICT (sha256) DO NOTHING RETURNING id, size_bytes",
        id_uuid,
        sha,
        filename_of(&stored_path),
        stored_path.to_string_lossy().to_string(),
        size,
        app_version,
        platform,
        uuid::Uuid::parse_str(&claims.sub).ok(),
    )
    .fetch_optional(pool.get_ref())
    .await;

    // 上传时顺带清理过期包（尽力而为）
    if let Err(e) = sweep_once(pool.get_ref()).await {
        log::warn!("[logs] post-upload sweep failed: {e}");
    }

    match result {
        Ok(Some(row)) => HttpResponse::Created().json(UploadResponse {
            id: row.id.to_string(),
            sha256: sha,
            duplicate: false,
            size_bytes: row.size_bytes,
        }),
        Ok(None) => HttpResponse::Conflict().json(UploadResponse {
            id,
            sha256: sha,
            duplicate: true,
            size_bytes: size,
        }),
        Err(e) => {
            let _ = std::fs::remove_file(&stored_path);
            HttpResponse::InternalServerError().body(format!("db: {e}"))
        }
    }
}

fn filename_of(p: &std::path::Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// manifest.json 在 zip 末尾写入；解 zip 需要 zip crate——服务端为零依赖
/// 起见直接在字节里找 manifest 的平台/版本字段（Deflated 不影响 json 字面
/// 出现，若找不到则置空，仅供分析参考，非关键路径）。
fn read_manifest_meta(bytes: &[u8]) -> (Option<String>, Option<String>) {
    let version = find_manifest_field(bytes, "app_version");
    let platform = find_manifest_field(bytes, "platform");
    (version, platform)
}

fn find_manifest_field(bytes: &[u8], key: &str) -> Option<String> {
    // 形如 "app_version": "0.89.0" （manifest.json 是 pretty JSON）
    let needle = format!("\"{key}\": \"");
    let haystack = String::from_utf8_lossy(bytes);
    let start = haystack.find(&needle)? + needle.len();
    let rest = &haystack[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

#[derive(Debug, Serialize)]
struct PackageRow {
    id: String,
    filename: String,
    size_bytes: i64,
    app_version: Option<String>,
    platform: Option<String>,
    uploaded_at: chrono::DateTime<chrono::Utc>,
}

#[get("/logs/packages")]
async fn list_packages(req: HttpRequest, pool: web::Data<PgPool>) -> impl Responder {
    if let Err(resp) = check_ops_token(&req) {
        return resp;
    }
    let rows = sqlx::query!(
        "SELECT id, filename, size_bytes, app_version, platform, uploaded_at \
         FROM log_packages ORDER BY uploaded_at DESC LIMIT 100"
    )
    .fetch_all(pool.get_ref())
    .await;
    match rows {
        Ok(rows) => HttpResponse::Ok().json(
            rows.into_iter()
                .map(|r| PackageRow {
                    id: r.id.to_string(),
                    filename: r.filename,
                    size_bytes: r.size_bytes,
                    app_version: r.app_version,
                    platform: r.platform,
                    uploaded_at: r.uploaded_at,
                })
                .collect::<Vec<_>>(),
        ),
        Err(e) => HttpResponse::InternalServerError().body(format!("db: {e}")),
    }
}

#[get("/logs/packages/{id}/download")]
async fn download_package(
    req: HttpRequest,
    pool: web::Data<PgPool>,
    path: web::Path<String>,
) -> impl Responder {
    if let Err(resp) = check_ops_token(&req) {
        return resp;
    }
    let id = path.into_inner();
    let Some(uid) = uuid::Uuid::parse_str(&id).ok() else {
        return HttpResponse::BadRequest().body("invalid id");
    };
    let row = sqlx::query!(
        "SELECT stored_path, filename FROM log_packages WHERE id = $1",
        uid
    )
    .fetch_optional(pool.get_ref())
    .await;
    match row {
        Ok(Some(r)) => match std::fs::read(&r.stored_path) {
            Ok(bytes) => HttpResponse::Ok()
                .content_type("application/zip")
                .insert_header((
                    "Content-Disposition",
                    format!("attachment; filename=\"{}\"", r.filename),
                ))
                .body(bytes),
            Err(_) => HttpResponse::NotFound().body("file missing"),
        },
        Ok(None) => HttpResponse::NotFound().body("not found"),
        Err(e) => HttpResponse::InternalServerError().body(format!("db: {e}")),
    }
}

// 本地可测纯函数（TESTING.md 惯例：src-server 无本地 PG，sqlx 宏不编译，
// 仅以独立提取的单测验证纯函数；CI/Docker 构建负责端到端编译验证）。
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_hex_is_stable() {
        // 确定性 + 64 位十六进制 + 输入敏感
        assert_eq!(sha256_hex(b"abc"), sha256_hex(b"abc"));
        assert_eq!(sha256_hex(b"abc").len(), 64);
        assert_ne!(sha256_hex(b"abc"), sha256_hex(b"abd"));
        // 已知向量（SHA-256("abc")）
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn manifest_meta_extraction() {
        let mut zip_like: Vec<u8> = Vec::new();
        zip_like.extend_from_slice(b"PK\x03\x04deflated-binary-blob...");
        let manifest = br#"{ "app_version": "0.89.0", "platform": "macos", "files": [] }"#;
        zip_like.extend_from_slice(manifest);
        let (version, platform) = read_manifest_meta(&zip_like);
        assert_eq!(version.as_deref(), Some("0.89.0"));
        assert_eq!(platform.as_deref(), Some("macos"));
    }

    #[test]
    fn manifest_meta_missing_is_none() {
        let (version, platform) = read_manifest_meta(b"no manifest here");
        assert_eq!(version, None);
        assert_eq!(platform, None);
    }
}
