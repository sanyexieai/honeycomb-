use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use super::model::{
    ExecutionContext, ExecutorKind, FitnessReport, GenomeSpec, HiveInput, HiveInstance, HiveOutput,
    HiveSpec, ImplementationSpec, LifecycleState, PracticeProfile, Recommendation, TaskHiveSession,
    TaskRuntime, TaskSpec, TaskStatus,
};

#[async_trait]
pub trait Hive: Send + Sync {
    fn spec(&self) -> &HiveSpec;
    fn implementation(&self) -> &ImplementationSpec;
    fn state(&self) -> &HiveInstance;

    async fn handle(&mut self, input: HiveInput) -> Result<HiveOutput>;
}

#[async_trait]
pub trait Executor: Send + Sync {
    fn kind(&self) -> ExecutorKind;

    async fn execute(
        &self,
        spec: &HiveSpec,
        implementation: &ImplementationSpec,
        instance: &mut HiveInstance,
        input: HiveInput,
        ctx: &ExecutionContext,
    ) -> Result<HiveOutput>;
}

#[async_trait]
pub trait Scheduler: Send + Sync {
    async fn submit(&self, task: TaskSpec) -> Result<String>;
    async fn poll(&self, task_id: &str) -> Result<TaskRuntime>;
    async fn update_task_status(&self, task_id: &str, status: TaskStatus) -> Result<()>;
    async fn add_session(&self, task_id: &str, session: TaskHiveSession) -> Result<()>;
    async fn update_session_lifecycle(
        &self,
        task_id: &str,
        session_id: &str,
        lifecycle: LifecycleState,
    ) -> Result<()>;
    async fn attach_session_output(
        &self,
        task_id: &str,
        session_id: &str,
        output: &HiveOutput,
    ) -> Result<()>;
}

#[async_trait]
pub trait HiveRepository: Send + Sync {
    async fn load_hive_spec(&self, hive_id: &str) -> Result<HiveSpec>;
    async fn load_active_impl(&self, hive_id: &str) -> Result<ImplementationSpec>;
    async fn load_genome(&self, hive_id: &str, impl_id: &str) -> Result<GenomeSpec>;
    async fn save_impl(&self, implementation: &ImplementationSpec) -> Result<()>;
    async fn save_state(&self, hive_id: &str, state: &HiveInstance) -> Result<()>;
}

#[async_trait]
pub trait PracticeRepository: Send + Sync {
    async fn match_practices(&self, hive_id: &str, context: &Value) -> Result<Vec<PracticeProfile>>;
    async fn save_practice(&self, practice: &PracticeProfile) -> Result<()>;
}

#[async_trait]
pub trait Orchestrator: Send + Sync {
    async fn dispatch(&self, target_capability: &str, input: HiveInput) -> Result<HiveOutput>;
    async fn dispatch_to_hive(&self, hive_id: &str, input: HiveInput) -> Result<HiveOutput>;
    async fn recommend(&self, capability: &str, context: &Value) -> Result<Vec<Recommendation>>;
}

#[async_trait]
pub trait EvolutionManager: Send + Sync {
    async fn mutate(
        &self,
        spec: &HiveSpec,
        implementation: &ImplementationSpec,
        genome: &GenomeSpec,
    ) -> Result<ImplementationSpec>;

    async fn split(
        &self,
        spec: &HiveSpec,
        implementation: &ImplementationSpec,
        genome: &GenomeSpec,
    ) -> Result<Vec<ImplementationSpec>>;

    async fn evaluate(
        &self,
        spec: &HiveSpec,
        implementation: &ImplementationSpec,
        output: &HiveOutput,
    ) -> Result<FitnessReport>;

    async fn select_active(&self, hive_id: &str, candidates: Vec<FitnessReport>) -> Result<Option<String>>;
}
