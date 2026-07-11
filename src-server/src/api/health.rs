//! Health Check API

use actix_web::{get, web, HttpResponse, Responder};
use serde_json::json;

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(health_check);
}

#[get("/health")]
async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(json!({
        "status": "ok",
        "service": "storymoss-server",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}
