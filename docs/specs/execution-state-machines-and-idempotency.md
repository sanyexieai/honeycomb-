# 执行面统一状态机与幂等规则

## 1. 目标

本文档补充执行面在任务、assignment、worker、trigger 四类对象上的状态机约束，以及事件重放、重复提交和恢复时的幂等规则。

## 2. 统一原则

- 状态迁移必须显式
- 非法迁移必须记录拒绝原因
- 恢复流程不得绕过原状态机
- 同一输入重放时结果应尽量幂等
- 事件追加与快照更新必须可重复执行

## 3. 任务状态机

建议任务状态：

- `queued`
- `running`
- `interrupted`
- `completed`
- `failed`
- `cancelled`

建议迁移：

| 当前状态 | 允许迁移到 | 说明 |
| --- | --- | --- |
| `queued` | `running` `cancelled` | 调度开始或显式取消 |
| `running` | `completed` `failed` `interrupted` `cancelled` | 正常结束、失败、中断或取消 |
| `interrupted` | `running` `failed` `cancelled` | 恢复、确认失败或取消 |
| `completed` | 无 | 终态 |
| `failed` | 无 | 终态 |
| `cancelled` | 无 | 终态 |

不允许：

- `completed -> running`
- `failed -> running`
- `cancelled -> running`

## 4. Assignment 状态机

建议 assignment 状态：

- `created`
- `assigned`
- `running`
- `retry_pending`
- `completed`
- `failed`
- `skipped`
- `cancelled`

建议迁移：

| 当前状态 | 允许迁移到 |
| --- | --- |
| `created` | `assigned` `skipped` `cancelled` |
| `assigned` | `running` `retry_pending` `failed` `cancelled` |
| `running` | `completed` `retry_pending` `failed` `cancelled` |
| `retry_pending` | `assigned` `failed` `skipped` `cancelled` |
| `completed` | 无 |
| `failed` | 无 |
| `skipped` | 无 |
| `cancelled` | 无 |

规则：

- 一个 assignment 在任一时刻只能有一个活跃执行尝试
- 新尝试必须有新 `attempt_id`
- 重试不复用旧结果文件路径

## 5. Worker 状态机

建议 worker 状态：

- `starting`
- `connecting`
- `idle`
- `busy`
- `degraded`
- `dead`
- `shutdown`

建议迁移：

| 当前状态 | 允许迁移到 |
| --- | --- |
| `starting` | `connecting` `dead` |
| `connecting` | `idle` `dead` |
| `idle` | `busy` `shutdown` `dead` |
| `busy` | `idle` `degraded` `dead` `shutdown` |
| `degraded` | `idle` `dead` `shutdown` |
| `dead` | 无 |
| `shutdown` | 无 |

规则：

- `dead` 与 `shutdown` 都视为不可再接收 assignment
- `degraded` 可被调度器暂时摘除

## 6. Trigger 状态机

建议 trigger 状态：

- `active`
- `paused`
- `disabled`
- `failed`

建议迁移：

| 当前状态 | 允许迁移到 |
| --- | --- |
| `active` | `paused` `disabled` `failed` |
| `paused` | `active` `disabled` |
| `failed` | `paused` `disabled` |
| `disabled` | 无 |

规则：

- `disabled` 视为永久停用
- 失败修复后建议先转为 `paused`，避免立即再次命中

## 7. 幂等键建议

建议以下对象支持幂等键：

- 任务创建：`task_request_id`
- trigger 命中：`dedupe_key`
- assignment 尝试：`attempt_id`
- 评估输入导出：`evaluation_export_id`

## 8. 事件写入幂等

建议规则：

- 每条事件带 `event_id`
- 同一 `event_id` 重放写入时应被识别为重复
- 快照更新允许重试，但不得回退状态

例如：

- 重复收到同一 `task_result`，若 `attempt_id` 已完成，则只追加一条去重审计或直接忽略
- 重试写 `task.json` 时，若目标状态低于当前快照状态，应拒绝覆盖

## 9. 恢复幂等

恢复过程建议遵守：

- 先基于 `events.jsonl` 重建事实
- 再基于最新快照补全索引
- 对已进入终态的 assignment 不重复调度
- 对未知中间态统一转为 `retry_pending` 或 `manual_intervention_required`

## 10. 协议幂等

### 10.1 `hello`

- 相同 worker 重复注册时，如果身份材料与任务上下文一致，可返回同一个接受结果
- 若上下文不一致，必须拒绝并审计

### 10.2 `task_result`

- 必须校验 `assignment_id` 和 `attempt_id`
- 已终态的旧尝试结果不得覆盖新尝试结果

### 10.3 `heartbeat`

- 重复心跳只更新最近存活时间，不应触发额外调度动作

## 11. 人工介入触点

以下情况建议直接进入人工介入：

- 状态迁移冲突无法自动裁决
- 同一 assignment 出现多个互斥结果
- worker 身份校验通过但上下文事实不一致
- 快照与事件流严重不一致

## 12. 总结

执行面真正稳定的关键，不只是“有哪些状态”，而是“哪些迁移合法、重复输入怎么处理、恢复后怎么继续”。这三件事必须一起定义，系统行为才可预测。
