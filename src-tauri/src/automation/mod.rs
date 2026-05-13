pub mod triggers;
pub mod handlers;
pub mod service;
pub mod commands;

pub use service::AutomationService;
pub use triggers::{AutomationTrigger, TriggerEvent, TriggerCondition};
pub use handlers::{AutomationHandler, HandlerAction};