use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use crate::core::{
    GenomeSpec, HiveInstance, HiveRepository, HiveSpec, ImplementationSpec, PracticeProfile,
    PracticeRepository, TaskRuntime,
};

#[derive(Debug, Clone)]
pub struct FsRepository {
    root: PathBuf,
}

impl FsRepository {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn load_implementation_from_dir(&self, hive_dir: &Path) -> Result<ImplementationSpec> {
        self.read_json_file(&hive_dir.join("implementation.json"))
    }

    pub fn save_task_runtime(&self, runtime: &TaskRuntime) -> Result<PathBuf> {
        let dir = self.root.join(".honeycomb").join("tasks");
        fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create task runtime dir: {}", dir.display()))?;

        let path = dir.join(format!("{}.json", runtime.task_id));
        self.write_json_file(&path, runtime)?;
        Ok(path)
    }

    pub fn load_task_runtime(&self, task_id: &str) -> Result<TaskRuntime> {
        let path = self.task_runtime_path(task_id);
        self.read_json_file(&path)
    }

    pub fn task_runtime_path(&self, task_id: &str) -> PathBuf {
        self.root
            .join(".honeycomb")
            .join("tasks")
            .join(format!("{}.json", task_id))
    }

    fn read_json_file<T: DeserializeOwned>(&self, path: &Path) -> Result<T> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let content = content.trim_start_matches('\u{feff}');
        serde_json::from_str(content)
            .with_context(|| format!("failed to parse json from {}", path.display()))
    }

    fn write_json_file<T: Serialize>(&self, path: &Path, value: &T) -> Result<()> {
        let data = serde_json::to_string_pretty(value)?;
        fs::write(path, data).with_context(|| format!("failed to write {}", path.display()))
    }
}

#[async_trait]
impl HiveRepository for FsRepository {
    async fn load_hive_spec(&self, _hive_id: &str) -> Result<HiveSpec> {
        bail!("hive spec loading is not implemented yet")
    }

    async fn load_active_impl(&self, _hive_id: &str) -> Result<ImplementationSpec> {
        bail!("implementation loading is not implemented yet")
    }

    async fn load_genome(&self, _hive_id: &str, _impl_id: &str) -> Result<GenomeSpec> {
        bail!("genome loading is not implemented yet")
    }

    async fn save_impl(&self, _implementation: &ImplementationSpec) -> Result<()> {
        bail!("implementation persistence is not implemented yet")
    }

    async fn save_state(&self, _hive_id: &str, _state: &HiveInstance) -> Result<()> {
        bail!("state persistence is not implemented yet")
    }
}

#[async_trait]
impl PracticeRepository for FsRepository {
    async fn match_practices(&self, _hive_id: &str, _context: &Value) -> Result<Vec<PracticeProfile>> {
        bail!("practice matching is not implemented yet")
    }

    async fn save_practice(&self, _practice: &PracticeProfile) -> Result<()> {
        bail!("practice persistence is not implemented yet")
    }
}
