//! API Routes

use actix_web::web;

pub(crate) mod admin;
mod health;
mod logs;
pub(crate) mod subscription;
mod user;

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api")
            .configure(health::init_routes)
            .configure(subscription::init_routes)
            .configure(logs::init_routes)
            .configure(user::init_routes)
            .configure(super::auth::handlers::init_routes)
            .service(web::scope("/admin").configure(admin::init_routes)),
    );
}
