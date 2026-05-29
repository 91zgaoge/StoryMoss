//! Command utilities — shared helpers for Tauri command handlers.

use tauri::{AppHandle, Runtime};

use crate::error::AppError;

/// Extension trait for `Result<T, AppError>` that emits a state-sync event
/// after a successful mutation.
///
/// This eliminates the repetitive pattern:
/// ```ignore
/// let result = do_something().map_err(AppError::from)?;
/// let _ = crate::state_sync::StateSync::emit_data_refresh(&app, story_id, "resourceType");
/// Ok(result)
/// ```
///
/// With the trait it becomes:
/// ```ignore
/// do_something()
///     .map_err(AppError::from)
///     .emit_sync(&app, story_id, "resourceType")
/// ```
pub trait EmitSync<R: Runtime> {
    /// Emit a `DataRefresh` sync event if `self` is `Ok`, then return `self`.
    fn emit_sync(self, app: &AppHandle<R>, story_id: Option<&str>, resource_type: &str) -> Self;
}

impl<R: Runtime, T> EmitSync<R> for Result<T, AppError> {
    fn emit_sync(self, app: &AppHandle<R>, story_id: Option<&str>, resource_type: &str) -> Self {
        if self.is_ok() {
            crate::state_sync::StateSync::emit_data_refresh(app, story_id, resource_type);
        }
        self
    }
}
