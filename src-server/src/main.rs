//! StoryMoss Server — Linux 服务端主站
//!
//! v4.5.0: Actix-web + PostgreSQL + OAuth2

use actix_cors::Cors;
use actix_web::{middleware, web, App, HttpServer};
use sqlx::postgres::PgPoolOptions;
use std::env;

mod api;
mod auth;
mod config;

use config::CONFIG;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Load .env file if present
    dotenvy::dotenv().ok();

    // Initialize logger
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    log::info!("🚀 StoryMoss Server v{}", env!("CARGO_PKG_VERSION"));
    log::info!("📡 Starting HTTP server on {}:{}", CONFIG.server_host, CONFIG.server_port);

    // Initialize database pool
    let db_pool = PgPoolOptions::new()
        .max_connections(20)
        .connect(&CONFIG.database_url)
        .await
        .expect("Failed to connect to PostgreSQL database");

    log::info!("✅ Database connected");

    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("Failed to run database migrations");

    log::info!("✅ Database migrations applied");

    let db_pool = web::Data::new(db_pool);

    HttpServer::new(move || {
        let cors = Cors::default()
            .allowed_origin(&CONFIG.frontend_url)
            .allow_any_method()
            .allow_any_header()
            .supports_credentials();

        App::new()
            .app_data(db_pool.clone())
            .wrap(middleware::Logger::default())
            .wrap(cors)
            .configure(api::init_routes)
    })
    .bind(format!("{}:{}", CONFIG.server_host, CONFIG.server_port))?
    .run()
    .await
}
