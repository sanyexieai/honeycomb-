# Task And Scheduler Spec

本文档定义任务模型和并行调度的基本方向。

## 1. 核心原则

- 二进制尽量稳定
- 任务是运行时对象，不是编译期对象
- 并行由主控统一调度
- 任务局部状态隔离，长期经验共享

## 2. TaskSpec

`TaskSpec` 表示一个提交给 Honeycomb 的任务。

建议字段：

- `task_id`
- `task_type`
- `input`
- `context`
- `topology`
- `constraints`

## 3. TaskRuntime

`TaskRuntime` 表示任务当前运行态。

建议字段：

- `task_id`
- `status`
- `shared_context`
- `sessions`
- `artifacts`

## 4. TaskHiveSession

每个任务中的每次 Hive 调用都应创建独立 session：

- 不与其他任务共享局部状态
- 可以引用同一 Hive 的长期实践和实现池
- 可以在当前任务内进行参数覆盖

## 5. 并行层级

### Task-Level Parallelism

多个任务同时执行。

### Hive-Level Parallelism

单任务内部多个 Hive session 并行。

### Implementation-Level Competition

同一能力的多个实现并行试跑，主要用于评估和进化。

## 6. 调度建议

推荐模型：

- `TaskSpec -> TaskGraph -> Ready Queue -> Executor -> Event -> State Update`

含义：

- 任务被展开为拓扑图
- 依赖满足的节点进入 ready queue
- executor 执行 session
- 完成后写回事件和状态
- scheduler 继续推进后续节点

## 7. 推荐的第一版范围

第一版建议重点支持：

- `Singleton`
- `Pipeline`
- `Graph`

暂不重点支持：

- 完全自治 swarm
- 去中心化协商式调度

## 8. 隔离规则

建议至少保证：

1. 同一个 `TaskHiveSession` 只能被一个执行器占用
2. 同一个任务的局部状态不能被别的任务直接写
3. 长期实践更新不能在任务执行中直接无锁覆盖

## 9. 共享与隔离

共享：

- Hive 能力定义
- Implementation 池
- Practice Profile
- Evaluation 历史

隔离：

- 任务输入
- 当前任务上下文
- 本次 session 的临时状态
- 本次中间产物
