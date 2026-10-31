//! P3：白盒情绪电路适配端口（仅本地 HF 模型可选）。
//!
//! 远程 OpenAI 兼容 API 拿不到 hidden states，不能做激活级干预。本端口只
//! 为本地已加载权重的模型预留；默认实现明确返回不支持，避免误以为已接线。
//!
//! 论文方法：情绪方向向量 + 逐层局部组件 + 全局稀疏电路 + 差分向量调制。
//! 若未来接入，必须复用论文的消融/增强验证，不得跳过因果校验。

use super::vector::EmotionVector;

pub trait WhiteboxEmotionAdapter: Send + Sync {
    fn name(&self) -> &'static str;
    fn supports(&self, model_id: &str) -> bool;
    fn steer(&self, model_id: &str, emotion: &EmotionVector, scale: f32) -> Result<(), String>;
}

/// 默认端口：不声称支持任何白盒模型。
pub struct NoopWhiteboxAdapter;

impl WhiteboxEmotionAdapter for NoopWhiteboxAdapter {
    fn name(&self) -> &'static str {
        "noop"
    }

    fn supports(&self, _model_id: &str) -> bool {
        false
    }

    fn steer(&self, _model_id: &str, _emotion: &EmotionVector, _scale: f32) -> Result<(), String> {
        Err("白盒情绪调制未启用：当前模型不暴露 hidden states".into())
    }
}

pub fn whitebox_available() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noop_adapter_refuses_steering() {
        let adapter = NoopWhiteboxAdapter;
        assert!(!adapter.supports("any-model"));
        assert!(adapter
            .steer("any-model", &EmotionVector::default(), 0.8)
            .is_err());
        assert!(!whitebox_available());
    }
}
