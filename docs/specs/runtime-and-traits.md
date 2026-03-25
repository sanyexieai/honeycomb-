# Runtime And Traits Draft

本文档给出 Honeycomb 第一版的 Rust 核心结构体与 trait 草案。

## 1. 总体原则

运行时需要同时满足：

- 单 Hive 可独立执行
- 多 Hive 可通过统一协议协作
- 实现体可替换
- 最佳实践可按上下文竞争
- 每个任务拥有自己的运行时实例
- 主控二进制尽量稳定
- 外部脚本或子进程可按需拉起
- 演化过程可插拔

## 2. 核心结构体草案

### HiveSpec

```rust
pub struct HiveSpec {
    pub id: String,
    pub name: String,
    pub version: String,
    pub kind: HiveKind,
    pub capability: String,
    pub description: Option<String>,
    pub interface: InterfaceSpec,
    pub execution: ExecutionSpec,
    pub state: StateSpec,
    pub dependencies: DependencySpec,
    pub tools: ToolPolicy,
    pub evaluation: EvaluationSpec,
    pub evolution: EvolutionPolicy,
}
```

### ImplementationSpec

```rust
pub struct ImplementationSpec {
    pub impl_id: String,
    pub hive_id: String,
    pub version: String,
    pub status: ImplStatus,
    pub executor: ExecutorKind,
    pub entrypoint: String,
    pub components: ComponentSpec,
    pub strategy: StrategySpec,
    pub compatibility: CompatibilitySpec,
    pub constraints: RuntimeConstraints,
    pub origin: OriginSpec,
}
```

### GenomeSpec

```rust
pub struct GenomeSpec {
    pub hive_id: String,
    pub impl_id: String,
    pub mutable_genes: HashMap<String, GeneSpec>,
    pub immutable_fields: Vec<String>,
    pub mutation_policy: MutationPolicy,
}
```

### HiveInstance

```rust
pub struct HiveInstance {
    pub hive_id: String,
    pub impl_id: String,
    pub lifecycle: LifecycleState,
    pub state: serde_json::Value,
    pub memory: serde_json::Value,
    pub workspace: PathBuf,
}
```

### PracticeProfile

```rust
pub struct PracticeProfile {
    pub practice_id: String,
    pub hive_id: String,
    pub capability: String,
    pub context_selector: serde_json::Value,
    pub recommended_impl: String,
    pub recommended_strategy: serde_json::Value,
    pub fitness_score: f64,
    pub based_on_runs: u64,
}
```

### TaskHiveSession

```rust
pub struct TaskHiveSession {
    pub session_id: String,
    pub task_id: String,
    pub hive_id: String,
    pub selected_impl: String,
    pub selected_practice: Option<String>,
    pub lifecycle: LifecycleState,
    pub input: serde_json::Value,
    pub context: serde_json::Value,
    pub overrides: serde_json::Value,
    pub local_state: serde_json::Value,
    pub artifacts: Vec<Artifact>,
}
```

### TaskSpec

```rust
pub struct TaskSpec {
    pub task_id: String,
    pub task_type: String,
    pub input: serde_json::Value,
    pub context: serde_json::Value,
    pub topology: TaskTopology,
    pub constraints: TaskConstraints,
}
```

### TaskRuntime

```rust
pub struct TaskRuntime {
    pub task_id: String,
    pub status: TaskStatus,
    pub shared_context: serde_json::Value,
    pub sessions: Vec<TaskHiveSession>,
    pub artifacts: Vec<Artifact>,
}
```

## 3. 生命周期

建议状态：

- `Loaded`
- `Ready`
- `Running`
- `WaitingInput`
- `WaitingDependency`
- `Suspended`
- `Completed`
- `Failed`

状态机的目标是：

- 便于恢复
- 便于持久化
- 便于多 Hive 编排

## 4. 输入输出协议

### HiveInput

```rust
pub struct HiveInput {
    pub task_id: String,
    pub capability: String,
    pub payload: serde_json::Value,
    pub context: serde_json::Value,
    pub caller: Option<String>,
}
```

### HiveOutput

```rust
pub struct HiveOutput {
    pub task_id: String,
    pub hive_id: String,
    pub impl_id: String,
    pub success: bool,
    pub payload: serde_json::Value,
    pub artifacts: Vec<Artifact>,
    pub metrics: Vec<MetricValue>,
}
```

## 5. 核心 Trait

### Hive

最小执行单元。

```rust
#[async_trait::async_trait]
pub trait Hive: Send + Sync {
    fn spec(&self) -> &HiveSpec;
    fn implementation(&self) -> &ImplementationSpec;
    fn state(&self) -> &HiveInstance;

    async fn handle(&mut self, input: HiveInput) -> anyhow::Result<HiveOutput>;
}
```

### Executor

真正执行实现体逻辑。

```rust
#[async_trait::async_trait]
pub trait Executor: Send + Sync {
    fn kind(&self) -> ExecutorKind;

    async fn execute(
        &self,
        spec: &HiveSpec,
        implementation: &ImplementationSpec,
        instance: &mut HiveInstance,
        input: HiveInput,
        ctx: &ExecutionContext,
    ) -> anyhow::Result<HiveOutput>;
}
```

建议第一版至少支持：

- `ProcessExecutor`
- `CompositeExecutor`

其中：

- `ProcessExecutor` 用于调用脚本或二进制
- `CompositeExecutor` 用于组合 LLM、规则、工具和外部进程

### Scheduler

负责多任务和任务内并行推进。

```rust
#[async_trait::async_trait]
pub trait Scheduler: Send + Sync {
    async fn submit(&self, task: TaskSpec) -> anyhow::Result<String>;
    async fn poll(&self, task_id: &str) -> anyhow::Result<TaskRuntime>;
}
```

### HiveRepository

负责加载与持久化。

```rust
#[async_trait::async_trait]
pub trait HiveRepository: Send + Sync {
    async fn load_hive_spec(&self, hive_id: &str) -> anyhow::Result<HiveSpec>;
    async fn load_active_impl(&self, hive_id: &str) -> anyhow::Result<ImplementationSpec>;
    async fn load_genome(&self, hive_id: &str, impl_id: &str) -> anyhow::Result<GenomeSpec>;
    async fn save_impl(&self, implementation: &ImplementationSpec) -> anyhow::Result<()>;
    async fn save_state(&self, hive_id: &str, state: &HiveInstance) -> anyhow::Result<()>;
}
```

### PracticeRepository

负责最佳实践的读取、匹配、写回。

```rust
#[async_trait::async_trait]
pub trait PracticeRepository: Send + Sync {
    async fn match_practices(
        &self,
        hive_id: &str,
        context: &serde_json::Value,
    ) -> anyhow::Result<Vec<PracticeProfile>>;

    async fn save_practice(&self, practice: &PracticeProfile) -> anyhow::Result<()>;
}
```

### Orchestrator

负责 Hive 间调度与推荐。

```rust
#[async_trait::async_trait]
pub trait Orchestrator: Send + Sync {
    async fn dispatch(&self, target_capability: &str, input: HiveInput) -> anyhow::Result<HiveOutput>;
    async fn dispatch_to_hive(&self, hive_id: &str, input: HiveInput) -> anyhow::Result<HiveOutput>;
    async fn recommend(&self, capability: &str, context: &serde_json::Value) -> anyhow::Result<Vec<Recommendation>>;
}
```

### EvolutionManager

负责 mutation、split、评估、选择。

```rust
#[async_trait::async_trait]
pub trait EvolutionManager: Send + Sync {
    async fn mutate(
        &self,
        spec: &HiveSpec,
        implementation: &ImplementationSpec,
        genome: &GenomeSpec,
    ) -> anyhow::Result<ImplementationSpec>;

    async fn split(
        &self,
        spec: &HiveSpec,
        implementation: &ImplementationSpec,
        genome: &GenomeSpec,
    ) -> anyhow::Result<Vec<ImplementationSpec>>;

    async fn evaluate(
        &self,
        spec: &HiveSpec,
        implementation: &ImplementationSpec,
        output: &HiveOutput,
    ) -> anyhow::Result<FitnessReport>;

    async fn select_active(
        &self,
        hive_id: &str,
        candidates: Vec<FitnessReport>,
    ) -> anyhow::Result<Option<String>>;
}
```

## 6. 模块划分建议

推荐模块或 crate：

- `hive-core`
- `hive-spec`
- `hive-practice`
- `hive-store`
- `hive-runtime`
- `hive-scheduler`
- `hive-executors`
- `hive-orchestrator`
- `hive-evolution`
- `hive-cli`

## 7. 第一版实现顺序

建议按以下顺序推进：

1. 定义数据结构和 trait
2. 实现文件系统版本的 `HiveRepository`
3. 实现 `hive.md` 和 JSON 规范解析
4. 实现 `PracticeRepository` 和上下文匹配
5. 实现 `TaskSpec`、`TaskRuntime` 和基础 `Scheduler`
6. 实现 `ProcessExecutor`
7. 实现单 Hive 执行和 `TaskHiveSession`
8. 实现基础 `Orchestrator`
9. 实现最小 `EvolutionManager`

## 8. 第一版边界

建议第一版明确不做：

- 无约束自修改代码
- 任意协议自动进化
- 完全自治的去中心化群体行为

第一版应优先做稳：

- 契约稳定性
- 多实现切换
- 最佳实践匹配
- 任务运行时隔离
- 主控统一调度
- 外部 worker 执行协议
- 可追踪评估
- 多 Hive 协作
