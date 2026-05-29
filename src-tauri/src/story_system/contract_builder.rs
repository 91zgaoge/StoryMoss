//! Contract Builder - 合同构建器
//!
//! 辅助构建各类合同的 JSON 内容

/// 合同构建器
pub struct ContractBuilder;

impl ContractBuilder {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ContractBuilder {
    fn default() -> Self {
        Self::new()
    }
}
