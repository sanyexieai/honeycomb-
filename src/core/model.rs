use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveSpec {
    pub id: String,
    pub name: String,
    pub version: String,
    pub kind: HiveKind,
    pub capability: String,
    pub description: Option<String>,
    pub interface: InterfaceSpec,
    pub execution: ExecutionSpec,
    #[serde(default)]
    pub state: StateSpec,
    #[serde(default)]
    pub dependencies: DependencySpec,
    #[serde(default)]
    pub tools: ToolPolicy,
    pub evaluation: EvaluationSpec,
    pub evolution: EvolutionPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HiveKind {
    Hive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceSpec {
    pub input_schema: PathBuf,
    pub output_schema: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSpec {
    pub executor: ExecutorKind,
    pub entrypoint: String,
    pub timeout_ms: u64,
    pub max_steps: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutorKind {
    Llm,
    Deterministic,
    Composite,
    Process,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct StateSpec {
    pub persistent: bool,
    pub slots: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DependencySpec {
    pub required_capabilities: Vec<String>,
    pub optional_capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ToolPolicy {
    pub allowed: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationSpec {
    pub metrics: Vec<String>,
    pub fitness_formula: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionPolicy {
    pub implementation_mutable: bool,
    pub interface_mutable: bool,
    pub split_allowed: bool,
    pub recommendable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImplementationSpec {
    pub impl_id: String,
    pub hive_id: String,
    pub version: String,
    pub status: ImplStatus,
    pub executor: ExecutorKind,
    pub entrypoint: String,
    #[serde(default)]
    pub components: ComponentSpec,
    #[serde(default)]
    pub strategy: StrategySpec,
    pub compatibility: CompatibilitySpec,
    #[serde(default)]
    pub constraints: RuntimeConstraints,
    pub origin: OriginSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImplStatus {
    Active,
    Candidate,
    Deprecated,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ComponentSpec {
    pub prompt: Option<PathBuf>,
    pub config: Option<PathBuf>,
    pub script: Option<PathBuf>,
    pub binary: Option<PathBuf>,
    pub assets: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct StrategySpec {
    pub mode: Option<String>,
    pub llm_model: Option<String>,
    pub temperature: Option<f32>,
    pub tool_order: Vec<String>,
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilitySpec {
    pub capability: String,
    pub input_schema_version: String,
    pub output_schema_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct RuntimeConstraints {
    pub max_cost: Option<f64>,
    pub max_latency_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OriginSpec {
    pub source: OriginKind,
    pub parent_impl: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OriginKind {
    Manual,
    Mutation,
    Recombination,
    Imported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenomeSpec {
    pub hive_id: String,
    pub impl_id: String,
    pub mutable_genes: HashMap<String, GeneSpec>,
    pub immutable_fields: Vec<String>,
    pub mutation_policy: MutationPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneSpec {
    pub gene_type: GeneType,
    pub options: Option<Vec<Value>>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GeneType {
    Enum,
    Float,
    Integer,
    Bool,
    Sequence,
    String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationPolicy {
    pub max_mutations_per_generation: u32,
    pub allow_component_swap: bool,
    pub allow_prompt_rewrite: bool,
    pub allow_freeform_code_edit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveInstance {
    pub hive_id: String,
    pub impl_id: String,
    pub lifecycle: LifecycleState,
    pub state: Value,
    pub memory: Value,
    pub workspace: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PracticeProfile {
    pub practice_id: String,
    pub hive_id: String,
    pub capability: String,
    pub context_selector: Value,
    pub recommended_impl: String,
    pub recommended_strategy: Value,
    pub fitness_score: f64,
    pub based_on_runs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskHiveSession {
    pub session_id: String,
    pub task_id: String,
    pub hive_id: String,
    pub selected_impl: String,
    pub selected_practice: Option<String>,
    pub lifecycle: LifecycleState,
    pub input: Value,
    pub context: Value,
    pub overrides: Value,
    pub local_state: Value,
    pub artifacts: Vec<Artifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSpec {
    pub task_id: String,
    pub task_type: String,
    pub input: Value,
    pub context: Value,
    pub topology: TaskTopology,
    #[serde(default)]
    pub constraints: TaskConstraints,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRuntime {
    pub task_id: String,
    pub status: TaskStatus,
    pub shared_context: Value,
    pub sessions: Vec<TaskHiveSession>,
    pub artifacts: Vec<Artifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskTopology {
    Singleton,
    Pipeline,
    Graph,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TaskConstraints {
    pub max_concurrency: Option<usize>,
    pub timeout_ms: Option<u64>,
    pub budget: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Created,
    Queued,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleState {
    Loaded,
    Created,
    Ready,
    Running,
    WaitingInput,
    WaitingDependency,
    Suspended,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveInput {
    pub task_id: String,
    pub capability: String,
    pub payload: Value,
    pub context: Value,
    pub caller: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveOutput {
    pub task_id: String,
    pub hive_id: String,
    pub impl_id: String,
    pub success: bool,
    pub payload: Value,
    pub artifacts: Vec<Artifact>,
    pub metrics: Vec<MetricValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerRequest {
    pub task_id: String,
    pub session_id: String,
    pub hive_id: String,
    pub impl_id: String,
    pub input: Value,
    pub context: Value,
    pub overrides: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerResponse {
    pub success: bool,
    pub payload: Value,
    pub metrics: Option<Vec<MetricValue>>,
    pub artifacts: Option<Vec<Artifact>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub name: String,
    pub kind: String,
    pub path: Option<PathBuf>,
    pub value: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricValue {
    pub name: String,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FitnessReport {
    pub hive_id: String,
    pub impl_id: String,
    pub score: f64,
    pub metric_values: Vec<MetricValue>,
    pub accepted: bool,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    pub capability: String,
    pub hive_id: String,
    pub impl_id: String,
    pub score: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExecutionContext {
    pub run_id: Option<String>,
    pub metadata: HashMap<String, Value>,
}
