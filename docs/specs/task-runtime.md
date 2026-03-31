# 任务运行时规范（Bee Runtime 版）

任务运行时描述某个任务在 `honeycomb-bee` 中一次执行过程的临时状态与证据。

## 1. 任务运行时解决什么问题

它负责描述：

- 当前任务状态与拓扑推进位置
- 当前 queen/worker 会话与 assignment 生命周期
- 当前上下文、短期记忆与中间产物
- 当前工具调用轨迹与 LLM 执行证据
- 当前局部失败、重试与回退信息

## 2. 任务运行时不负责什么

任务运行时不负责：

- 定义全局技能能力
- 定义长期默认实现
- 定义长期谱系与晋升策略
- 发布全局最佳实践

以上归属 `honeycomb`（能力中心）与 `honeycomb-evolution`（进化中心）。

## 3. 核心对象

### 3.1 任务规格（Task Spec）

描述任务输入、目标、拓扑、约束与能力引用。

### 3.2 任务运行时（Task Runtime）

描述任务生命周期、节点推进状态、执行证据与局部状态。

### 3.3 Bee 会话（Bee Session）

描述某个 queen/worker 在当前任务中的一次实际参与情况。

## 4. 任务运行时建议字段

- `task_id`
- `tenant_id`
- `namespace`
- `owner_user_id`
- `owner_team_id`
- `status`
- `topology`
- `queen_node_id`
- `sessions`
- `assignments`
- `artifacts`
- `llm_trace_refs`
- `tool_call_refs`
- `created_at`
- `updated_at`

## 5. Bee 会话字段建议

- `session_id`
- `role`（`queen` / `worker`）
- `hive_id`
- `capability`
- `worker_node_id`
- `assignment_ids`
- `status`
- `local_state`
- `context_snapshot_ref`
- `prompt_bundle_ref`
- `artifacts`

## 6. 记忆与上下文

- 任务上下文与短期记忆属于运行时数据，必须可重放可清理
- 上下文允许按节点和会话做局部覆盖
- 会话内记忆不可直接升格为长期系统默认

## 7. 关键原则

- 任务运行时只属于本次任务
- 任务结束可归档，但不能污染长期能力定义
- 运行时智能执行必须可追溯到 LLM 与工具调用证据
- 长期状态变更必须经由 evolution -> hive 正式流程
