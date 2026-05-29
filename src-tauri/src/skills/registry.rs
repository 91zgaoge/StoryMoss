#![allow(dead_code)]
use std::collections::HashMap;

use super::*;

pub struct SkillRegistry {
    skills: HashMap<String, Skill>,
    hooks: HashMap<HookEvent, Vec<String>>, // event -> skill_ids
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
            hooks: HashMap::new(),
        }
    }

    /// Register a skill
    pub fn register(&mut self, skill: Skill) {
        let skill_id = skill.manifest.id.clone();

        // Register hooks
        for hook in &skill.manifest.hooks {
            let event = hook.event.clone();
            let entry = self.hooks.entry(event).or_default();
            entry.push(skill_id.clone());
            // Sort by priority
            entry.sort_by_key(|id| {
                self.skills
                    .get(id)
                    .and_then(|s| {
                        s.manifest
                            .hooks
                            .iter()
                            .find(|h| h.event == hook.event)
                            .map(|h| h.priority)
                    })
                    .unwrap_or(0)
            });
        }

        self.skills.insert(skill_id, skill);
    }

    /// Unregister a skill
    pub fn unregister(&mut self, skill_id: &str) -> Result<(), String> {
        let skill = self.skills.remove(skill_id).ok_or("Skill not found")?;

        // Remove from hooks
        for hook in &skill.manifest.hooks {
            if let Some(skills) = self.hooks.get_mut(&hook.event) {
                skills.retain(|id| id != skill_id);
            }
        }

        Ok(())
    }

    /// Get skill by ID
    pub fn get(&self, skill_id: &str) -> Option<Skill> {
        self.skills.get(skill_id).cloned()
    }

    /// Get all skills
    pub fn get_all(&self) -> Vec<Skill> {
        self.skills.values().cloned().collect()
    }

    /// Get skills by category
    pub fn get_by_category(&self, category: SkillCategory) -> Vec<Skill> {
        self.skills
            .values()
            .filter(|s| s.manifest.category == category)
            .cloned()
            .collect()
    }

    /// Get enabled skills
    pub fn get_enabled(&self) -> Vec<Skill> {
        self.skills
            .values()
            .filter(|s| s.is_enabled)
            .cloned()
            .collect()
    }

    /// Get skills for hook event
    pub fn get_hook_handlers(&self, event: &HookEvent) -> Vec<Skill> {
        self.hooks
            .get(event)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.skills.get(id))
                    .filter(|s| s.is_enabled)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Enable skill
    pub fn enable(&mut self, skill_id: &str) -> Result<(), String> {
        if let Some(skill) = self.skills.get_mut(skill_id) {
            skill.is_enabled = true;
            Ok(())
        } else {
            Err("Skill not found".to_string())
        }
    }

    /// Disable skill
    pub fn disable(&mut self, skill_id: &str) -> Result<(), String> {
        if let Some(skill) = self.skills.get_mut(skill_id) {
            skill.is_enabled = false;
            Ok(())
        } else {
            Err("Skill not found".to_string())
        }
    }

    /// Clear all skills
    pub fn clear(&mut self) {
        self.skills.clear();
        self.hooks.clear();
    }

    /// Update skill manifest
    pub fn update_manifest(
        &mut self,
        skill_id: &str,
        manifest: SkillManifest,
    ) -> Result<(), String> {
        if let Some(skill) = self.skills.get_mut(skill_id) {
            skill.manifest = manifest;
            Ok(())
        } else {
            Err("Skill not found".to_string())
        }
    }

    /// Check if skill exists
    pub fn contains(&self, skill_id: &str) -> bool {
        self.skills.contains_key(skill_id)
    }

    /// Get skill count
    pub fn len(&self) -> usize {
        self.skills.len()
    }

    /// Is empty
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}
