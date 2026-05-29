//! Workflow DSL Loader - 从 JSON/YAML 文件加载工作流定义
//!
//! 监视应用数据目录下的 workflows/ 文件夹，自动加载、热重载工作流模板。

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};

use super::{Workflow, WorkflowEngine};
use crate::error::AppError;

/// 已加载工作流的元数据
#[derive(Debug, Clone, serde::Serialize)]
pub struct LoadedWorkflow {
    #[serde(flatten)]
    pub workflow: Workflow,
    #[serde(skip)]
    pub source_path: PathBuf,
    pub is_builtin: bool,
}

/// 工作流加载器，负责文件系统扫描和监视
pub struct WorkflowLoader {
    engine: Arc<WorkflowEngine>,
    loaded: Arc<Mutex<HashMap<String, LoadedWorkflow>>>,
    watch_paths: Arc<Mutex<Vec<PathBuf>>>,
}

impl WorkflowLoader {
    pub fn new(engine: Arc<WorkflowEngine>) -> Self {
        Self {
            engine,
            loaded: Arc::new(Mutex::new(HashMap::new())),
            watch_paths: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// 初始化加载：扫描内置目录 + 用户目录
    pub fn initialize(
        &self,
        builtin_dir: Option<PathBuf>,
        user_dir: PathBuf,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // 1. 内置工作流目录
        if let Some(dir) = builtin_dir {
            if dir.exists() {
                let _ = self.load_from_directory(&dir, true);
                self.watch_paths.lock().unwrap().push(dir);
            }
        }

        // 2. 用户工作流目录
        std::fs::create_dir_all(&user_dir)?;
        let _ = self.load_from_directory(&user_dir, false);
        self.watch_paths.lock().unwrap().push(user_dir);

        // 3. 启动文件系统监视器
        self.start_watcher()?;

        log::info!("[WorkflowLoader] Initialized");
        Ok(())
    }

    /// 从目录加载所有工作流文件
    fn load_from_directory(
        &self,
        dir: &Path,
        is_builtin: bool,
    ) -> Result<usize, Box<dyn std::error::Error>> {
        let mut count = 0;
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if matches!(ext, "json" | "yaml" | "yml") {
                    match self.load_file(&path, is_builtin) {
                        Ok(_) => count += 1,
                        Err(e) => {
                            log::warn!("[WorkflowLoader] Failed to load {}: {}", path.display(), e)
                        }
                    }
                }
            }
        }
        log::info!(
            "[WorkflowLoader] Loaded {} workflows from {}",
            count,
            dir.display()
        );
        Ok(count)
    }

    /// 加载单个文件
    fn load_file(&self, path: &Path, is_builtin: bool) -> Result<(), Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let workflow: Workflow =
            match ext {
                "json" => serde_json::from_str(&content)
                    .map_err(|e| format!("JSON parse error: {}", e))?,
                "yaml" | "yml" => serde_yaml::from_str(&content)
                    .map_err(|e| format!("YAML parse error: {}", e))?,
                _ => return Err("Unsupported file format".into()),
            };

        // 验证工作流无环
        self.engine.register_workflow(workflow.clone())?;

        let loaded = LoadedWorkflow {
            workflow: workflow.clone(),
            source_path: path.to_path_buf(),
            is_builtin,
        };

        let mut loaded_map = self.loaded.lock().unwrap();
        loaded_map.insert(workflow.id.clone(), loaded);

        log::info!(
            "[WorkflowLoader] Registered workflow '{}' from {}",
            workflow.id,
            path.display()
        );
        Ok(())
    }

    /// 启动文件系统监视器
    fn start_watcher(&self) -> Result<(), Box<dyn std::error::Error>> {
        let loaded = self.loaded.clone();
        let engine = self.engine.clone();
        let watch_paths = self.watch_paths.clone();

        std::thread::spawn(move || {
            let (tx, rx) = std::sync::mpsc::channel();

            let mut watcher: RecommendedWatcher = match RecommendedWatcher::new(
                move |res: Result<Event, notify::Error>| {
                    if let Ok(event) = res {
                        let _ = tx.send(event);
                    }
                },
                Config::default(),
            ) {
                Ok(w) => w,
                Err(e) => {
                    log::error!("[WorkflowLoader] Failed to create watcher: {}", e);
                    return;
                }
            };

            let paths = watch_paths.lock().unwrap().clone();
            for path in &paths {
                if let Err(e) = watcher.watch(path, RecursiveMode::NonRecursive) {
                    log::warn!("[WorkflowLoader] Failed to watch {}: {}", path.display(), e);
                }
            }

            log::info!("[WorkflowLoader] Watcher started");

            while let Ok(event) = rx.recv() {
                match event.kind {
                    notify::EventKind::Create(_) | notify::EventKind::Modify(_) => {
                        for path in &event.paths {
                            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                            if !matches!(ext, "json" | "yaml" | "yml") {
                                continue;
                            }

                            let is_builtin =
                                paths.first().map(|p| path.starts_with(p)).unwrap_or(false);
                            log::info!(
                                "[WorkflowLoader] Reloading workflow from {}",
                                path.display()
                            );

                            // 先尝试移除旧的工作流
                            let old_id = {
                                let loaded_map = loaded.lock().unwrap();
                                loaded_map
                                    .values()
                                    .find(|l| l.source_path == *path)
                                    .map(|l| l.workflow.id.clone())
                            };

                            // 重新加载
                            let content = match std::fs::read_to_string(path) {
                                Ok(c) => c,
                                Err(e) => {
                                    log::warn!(
                                        "[WorkflowLoader] Failed to read {}: {}",
                                        path.display(),
                                        e
                                    );
                                    continue;
                                }
                            };

                            let workflow: Result<Workflow, _> = match ext {
                                "json" => serde_json::from_str(&content).map_err(AppError::from),
                                "yaml" | "yml" => {
                                    serde_yaml::from_str(&content).map_err(AppError::from)
                                }
                                _ => continue,
                            };

                            match workflow {
                                Ok(w) => {
                                    if let Err(e) = engine.register_workflow(w.clone()) {
                                        log::warn!(
                                            "[WorkflowLoader] Failed to register {}: {}",
                                            path.display(),
                                            e
                                        );
                                        continue;
                                    }
                                    let mut loaded_map = loaded.lock().unwrap();
                                    if let Some(old) = old_id {
                                        loaded_map.remove(&old);
                                    }
                                    loaded_map.insert(
                                        w.id.clone(),
                                        LoadedWorkflow {
                                            workflow: w,
                                            source_path: path.clone(),
                                            is_builtin,
                                        },
                                    );
                                    log::info!(
                                        "[WorkflowLoader] Hot-reloaded '{}'",
                                        path.display()
                                    );
                                }
                                Err(e) => {
                                    log::warn!(
                                        "[WorkflowLoader] Parse error in {}: {}",
                                        path.display(),
                                        e
                                    );
                                }
                            }
                        }
                    }
                    notify::EventKind::Remove(_) => {
                        for path in &event.paths {
                            let mut loaded_map = loaded.lock().unwrap();
                            let to_remove = loaded_map
                                .values()
                                .find(|l| l.source_path == *path)
                                .map(|l| l.workflow.id.clone());
                            if let Some(id) = to_remove {
                                loaded_map.remove(&id);
                                log::info!("[WorkflowLoader] Removed workflow '{}'", id);
                            }
                        }
                    }
                    _ => {}
                }
            }
        });

        Ok(())
    }

    /// 手动重新加载所有路径
    pub fn reload_all(&self) -> Result<usize, Box<dyn std::error::Error>> {
        let mut total = 0;
        let paths = self.watch_paths.lock().unwrap().clone();
        for (idx, path) in paths.iter().enumerate() {
            let is_builtin = idx == 0;
            total += self.load_from_directory(path, is_builtin)?;
        }
        Ok(total)
    }

    /// 列出所有已加载的工作流
    pub fn list_workflows(&self) -> Vec<LoadedWorkflow> {
        let loaded = self.loaded.lock().unwrap();
        loaded.values().cloned().collect()
    }
}
