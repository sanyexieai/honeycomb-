# Honeycomb 设计文档

这组文档用于定义 Honeycomb 当前阶段的系统边界、运行模型和后续实现方向。

当前总原则：

- 可以推翻旧方案，不被已有原型绑定
- 单任务内允许蜂后主导
- 系统级不设唯一全局中心
- 三层分治：蜂运行时、蜂巢能力中心、生态进化中心
- 角色、任务、技能、进化必须边界清晰
- 逻辑隔离和物理隔离要同步考虑

三个二进制：

- `honeycomb-bee`：蜂运行时，负责蜂后/工蜂角色执行、LLM 推理、会话上下文与工具调用
- `honeycomb`：蜂巢能力中心，负责全局能力资产、工具治理、最佳实现发布入口
- `honeycomb-evolution`：生态进化中心，负责全量观测汇总、评分、谱系与治理决策

**CLI 最小入口（类 Claude Code 会话）**：在仓库根目录执行 `cargo run --bin honeycomb-bee`（或安装后的 `honeycomb-bee`），无参数即进入交互式 Code REPL；等价地也可用 `cargo run --bin honeycomb -- code`。默认技能为 `code-assistant`，实现 `impl_code_default` 使用 **`minimax_chat`**（默认模型 `MiniMax-M2.5`，见 `registry/implementations/impl_code_default.json`）。推荐使用通用环境变量：`HONEYCOMB_LLM_PROVIDER=minimax`、`HONEYCOMB_LLM_API_KEY=...`、`HONEYCOMB_LLM_BASE_URL=https://api.minimaxi.com/v1`（写入仓库根目录 `.env` 或 `.env.local`，运行时自动加载）。系统提示词文件：`prompts/code-assistant.md`。可用 `HONEYCOMB_CODE_SKILL` 或 `code --skill-id …` 换用其它已注册技能。

建议优先阅读顺序：

1. `architecture.md`
2. `specs/design-status.md`
3. `specs/domain-boundaries.md`
4. `specs/execution-vs-evolution-plane.md`
5. `specs/node-identity.md`
6. `specs/queen-worker-model.md`
7. `specs/queen-worker-protocol.md`
8. `specs/task-topology.md`
9. `specs/runtime-event-schema.md`
10. `specs/runtime-storage-layout.md`

当前重点文档：

- `specs/task-runtime.md`
- `specs/task-scheduler.md`
- `specs/multi-user-model.md`
- `specs/multi-tenant-model.md`
- `specs/permission-and-visibility.md`
- `specs/resource-control-and-quotas.md`
- `specs/failure-recovery.md`
- `specs/audit-and-trace.md`
- `specs/execution-evolution-data-examples.md`

文档目标不是一次写死所有实现细节，而是先把结构位置、数据边界和后续扩展空间定稳，避免后续因为边界不清而整体返工。

## 扩展文档索引（按主题）

安全与可观测：

- `specs/security-and-trust-model.md`
- `specs/observability-and-replay.md`
- `specs/architecture-review-and-guardrails.md`
- `specs/architecture-review-record.md`
- `specs/architecture-reflection-record.md`

技能与生态：

- `specs/skill-registry-and-marketplace.md`
- `specs/evolution-governance.md`
- `specs/skill-package-format.md`
- `specs/import-export-workflow.md`
- `specs/remote-skill-registry-and-distribution.md`
- `specs/template-practice-integration.md`

运行时与模板：

- `specs/worker-materialization-and-lifecycle.md`
- `specs/debug-and-inspection-tools.md`
- `specs/evolution-registry-integration.md`
- `specs/binary-and-deployment-layout.md`
- `specs/execution-code-module-mapping.md`
- `specs/evolution-code-module-mapping.md`
- `specs/execution-state-machines-and-idempotency.md`
- `specs/storage-schema-versioning-and-migration.md`
- `specs/manual-intervention-and-approval-workflow.md`
- `specs/task-templates-and-composition.md`
- `specs/ecosystem-composite-hives.md`
- `specs/resident-hive-model.md`

调度与路线图：

- `specs/trigger-and-schedule-model.md`
- `specs/schedule-state-and-history.md`
- `specs/execution-plane-implementation-roadmap.md`
- `specs/evolution-plane-implementation-roadmap.md`
- `specs/convergence-review-and-roadmap.md`
- `specs/current-capability-audit-and-aggressive-convergence.md`

三层激进重构：

- `specs/bee-runtime-hive-ecology-architecture.md`
- `specs/binary-and-deployment-layout.md`
- `specs/migration-cutover-plan.md`
- `specs/execution-state-machines-and-idempotency.md`（Bee 运行时状态机）

记忆、上下文与 LLM 角色（目标与现状对齐）：

- `specs/memory-context-and-llm-roles.md`

未列入上文优先顺序但已存在的规格：`specs/runtime-and-traits.md`、`specs/process-executor.md`（与运行时/执行器实现细节相关）。

## 文档口径（统一术语）

- `honeycomb-bee`：bee 运行时（queen/worker、任务内 LLM 与证据）
- `honeycomb`：蜂巢能力中心（长期能力资产、注册与工具治理）
- `honeycomb-evolution`：生态进化中心（评分、谱系、治理与晋升）
- 旧文档中的「执行面」一般指**运行时侧**；激进重构后拆为 **bee 运行时** 与 **能力中心**，以 `binary-and-deployment-layout.md` 为准。
