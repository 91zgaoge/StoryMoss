//! Subscription domain types.
//!
//! Shared value objects for subscription tiers. Business logic remains in
//! `crate::subscription`.

use serde::{Deserialize, Serialize};

/// 订阅层级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubscriptionTier {
    Free,
    Pro,
    Enterprise,
}

impl std::fmt::Display for SubscriptionTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            SubscriptionTier::Free => "free",
            SubscriptionTier::Pro => "pro",
            SubscriptionTier::Enterprise => "enterprise",
        };
        write!(f, "{}", s)
    }
}

impl std::str::FromStr for SubscriptionTier {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "free" => Ok(SubscriptionTier::Free),
            "pro" => Ok(SubscriptionTier::Pro),
            "enterprise" => Ok(SubscriptionTier::Enterprise),
            _ => Err(format!("Unknown subscription tier: {}", s)),
        }
    }
}
