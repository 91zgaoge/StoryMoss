//! Operational Transformation (OT) Algorithm

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OperationType {
    Insert,
    Delete,
    Retain,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextOperation {
    pub op_type: OperationType,
    pub position: usize,
    pub content: Option<String>,
    pub length: usize,
    pub client_id: String,
    pub timestamp: u64,
}

impl TextOperation {
    pub fn insert(position: usize, content: String, client_id: String) -> Self {
        let length = content.len();
        Self {
            op_type: OperationType::Insert,
            position,
            content: Some(content),
            length,
            client_id,
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
        }
    }

    pub fn delete(position: usize, length: usize, client_id: String) -> Self {
        Self {
            op_type: OperationType::Delete,
            position,
            content: None,
            length,
            client_id,
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
        }
    }
}

pub struct OTTransformer;

impl OTTransformer {
    pub fn transform(op1: &TextOperation, op2: &TextOperation) -> (TextOperation, TextOperation) {
        match (&op1.op_type, &op2.op_type) {
            (OperationType::Insert, OperationType::Insert) => {
                if op1.position <= op2.position {
                    let mut t2 = op2.clone();
                    t2.position += op1.length;
                    (op1.clone(), t2)
                } else {
                    let mut t1 = op1.clone();
                    t1.position += op2.length;
                    (t1, op2.clone())
                }
            }
            (OperationType::Insert, OperationType::Delete) => {
                if op1.position <= op2.position {
                    let mut t2 = op2.clone();
                    t2.position += op1.length;
                    (op1.clone(), t2)
                } else {
                    let mut t1 = op1.clone();
                    t1.position -= op2.length.min(op1.position - op2.position);
                    (t1, op2.clone())
                }
            }
            (OperationType::Delete, OperationType::Insert) => {
                let (t2, t1) = Self::transform(op2, op1);
                (t1, t2)
            }
            _ => (op1.clone(), op2.clone()),
        }
    }

    pub fn apply(text: &str, op: &TextOperation) -> Result<String, String> {
        match op.op_type {
            OperationType::Insert => {
                if op.position > text.len() {
                    return Err("Position out of bounds".into());
                }
                let mut result = String::new();
                result.push_str(&text[..op.position]);
                result.push_str(op.content.as_ref().ok_or("Missing content")?);
                result.push_str(&text[op.position..]);
                Ok(result)
            }
            OperationType::Delete => {
                if op.position + op.length > text.len() {
                    return Err("Delete range out of bounds".into());
                }
                let mut result = String::new();
                result.push_str(&text[..op.position]);
                result.push_str(&text[op.position + op.length..]);
                Ok(result)
            }
            _ => Ok(text.to_string()),
        }
    }
}
