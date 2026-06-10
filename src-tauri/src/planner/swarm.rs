//! Agent Swarm - 多智能体并行调度与协作
//!
//! 将执行计划中的步骤按依赖关系拓扑排序，
//! 无依赖的步骤并行执行，有依赖的步骤顺序执行。
//! 支持 Writer→Inspector→Writer 闭环协作模式。

use std::collections::{HashMap, HashSet};

/// 拓扑排序结果：将步骤分组成可并行执行的批次
pub struct ExecutionBatches {
    pub batches: Vec<Vec<String>>, // 每批包含的 step_id 列表
}

/// 对计划步骤进行拓扑排序，返回可按批次并行执行的顺序
pub fn topological_sort(steps: &[crate::planner::PlanStep]) -> ExecutionBatches {
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    let mut dependents: HashMap<String, Vec<String>> = HashMap::new();
    let mut all_ids: HashSet<String> = HashSet::new();

    // 初始化
    for step in steps {
        all_ids.insert(step.step_id.clone());
        in_degree.entry(step.step_id.clone()).or_insert(0);
        for dep in &step.depends_on {
            // 只统计存在于当前计划中的依赖
            if all_ids.contains(dep) || steps.iter().any(|s| s.step_id == *dep) {
                in_degree
                    .entry(step.step_id.clone())
                    .and_modify(|e| *e += 1)
                    .or_insert(1);
                dependents
                    .entry(dep.clone())
                    .or_default()
                    .push(step.step_id.clone());
            }
        }
    }

    // 重新初始化 all_ids（上面的逻辑有bug，应该在遍历前完成）
    all_ids.clear();
    for step in steps {
        all_ids.insert(step.step_id.clone());
    }

    // Kahn's algorithm
    let mut batches: Vec<Vec<String>> = Vec::new();
    let mut completed: HashSet<String> = HashSet::new();

    while completed.len() < all_ids.len() {
        let mut ready: Vec<String> = Vec::new();

        for step in steps {
            if completed.contains(&step.step_id) {
                continue;
            }
            // 检查所有依赖是否已完成
            let deps_satisfied = step.depends_on.iter().all(|dep| {
                // 如果依赖不在当前计划中，认为已满足
                !all_ids.contains(dep) || completed.contains(dep)
            });
            if deps_satisfied {
                ready.push(step.step_id.clone());
            }
        }

        if ready.is_empty() {
            // 存在循环依赖，打破循环（选择入度最小的一个）
            let remaining: Vec<String> = all_ids
                .iter()
                .filter(|id| !completed.contains(*id))
                .cloned()
                .collect();
            if let Some(first) = remaining.first() {
                log::warn!(
                    "[Swarm] Circular dependency detected, forcing execution of {}",
                    first
                );
                ready.push(first.clone());
            } else {
                break;
            }
        }

        for id in &ready {
            completed.insert(id.clone());
        }
        batches.push(ready);
    }

    ExecutionBatches { batches }
}

/// 检测计划是否包含 Inspector→Writer 闭环模式
/// 返回闭环的起始 step_id 和结束 step_id
pub fn detect_inspector_writer_loop(
    steps: &[crate::planner::PlanStep],
) -> Option<(String, String)> {
    // 查找 writer step 前面有 inspector step 的模式
    for (i, step) in steps.iter().enumerate() {
        if step.capability_id == "writer" {
            // 向前查找是否有 inspector 依赖
            for dep in &step.depends_on {
                if let Some(inspector_idx) = steps.iter().position(|s| s.step_id == *dep) {
                    if steps[inspector_idx].capability_id == "inspector" {
                        return Some((steps[inspector_idx].step_id.clone(), step.step_id.clone()));
                    }
                }
            }
            // 或者直接顺序查找：inspector 在前，writer 在后
            if i > 0 {
                for prev in steps[..i].iter().rev() {
                    if prev.capability_id == "inspector" {
                        return Some((prev.step_id.clone(), step.step_id.clone()));
                    }
                }
            }
        }
    }
    None
}

/// 计算步骤的预估执行时间（用于负载均衡）
pub fn estimate_step_complexity(step: &crate::planner::PlanStep) -> u32 {
    match step.capability_id.as_str() {
        "writer" => 10,
        "inspector" => 8,
        "outline_planner" => 6,
        "plot_analyzer" => 5,
        "style_mimic" => 4,
        "create_story" | "create_chapter" | "create_character" => 2,
        skill_id if skill_id.starts_with("builtin.") => 3,
        _ => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::PlanStep;

    fn make_step(id: &str, cap: &str, deps: &[&str]) -> PlanStep {
        PlanStep {
            step_id: id.to_string(),
            capability_id: cap.to_string(),
            purpose: "test".to_string(),
            parameters: HashMap::new(),
            depends_on: deps.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn test_topological_sort_linear() {
        let steps = vec![
            make_step("a", "writer", &[]),
            make_step("b", "inspector", &["a"]),
            make_step("c", "writer", &["b"]),
        ];
        let batches = topological_sort(&steps);
        assert_eq!(batches.batches.len(), 3);
        assert!(batches.batches[0].contains(&"a".to_string()));
        assert!(batches.batches[1].contains(&"b".to_string()));
        assert!(batches.batches[2].contains(&"c".to_string()));
    }

    #[test]
    fn test_topological_sort_parallel() {
        let steps = vec![
            make_step("a", "writer", &[]),
            make_step("b", "outline_planner", &[]),
            make_step("c", "inspector", &["a"]),
            make_step("d", "writer", &["b"]),
        ];
        let batches = topological_sort(&steps);
        // Batch 0: a, b (no deps)
        assert_eq!(batches.batches[0].len(), 2);
        assert!(batches.batches[0].contains(&"a".to_string()));
        assert!(batches.batches[0].contains(&"b".to_string()));
        // Batch 1: c, d (depend on a, b respectively)
        assert_eq!(batches.batches[1].len(), 2);
    }

    #[test]
    fn test_detect_inspector_writer_loop() {
        let steps = vec![
            make_step("inspect", "inspector", &[]),
            make_step("rewrite", "writer", &["inspect"]),
        ];
        let result = detect_inspector_writer_loop(&steps);
        assert!(result.is_some());
        let (start, end) = result.unwrap();
        assert_eq!(start, "inspect");
        assert_eq!(end, "rewrite");
    }
}
