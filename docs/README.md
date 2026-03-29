# Honeycomb 设计文档

这组文档用于定义 Honeycomb 当前阶段的系统边界、运行模型和后续实现方向。

当前总原则：

- 可以推翻旧方案，不被已有原型绑定
- 单任务内允许蜂后主导
- 系统级不设唯一全局中心
- 执行面与进化面拆成两个二进制
- 角色、任务、技能、进化必须边界清晰
- 逻辑隔离和物理隔离要同步考虑

两个二进制：

- `honeycomb`：执行面，负责蜂后、工蜂、任务运行、协议通信、任务状态与产物
- `honeycomb-evolution`：进化面，负责评分、谱系、晋升、淘汰、最佳实践沉淀

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

未列入上文优先顺序但已存在的规格：`specs/runtime-and-traits.md`、`specs/process-executor.md`（与运行时/执行器实现细节相关）。
