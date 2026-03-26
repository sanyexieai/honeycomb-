# 运行时事件规范

## 1. 目标

运行时事件用于记录执行面中发生的关键行为，为调试、追踪、回放、恢复和进化输入提供统一来源。

## 2. 基本原则

- 事件只追加，不原地改写
- 事件优先记录事实，不记录推测
- 事件必须带上任务与节点上下文
- 事件应可被追踪层和审计层复用

## 3. 最小事件包结构

建议字段：

- `event_id`
- `event_type`
- `task_id`
- `node_id`
- `queen_node_id`
- `tenant_id`
- `namespace`
- `timestamp`
- `payload`

## 4. 推荐事件分类

### 4.1 任务事件

例如：

- `task_created`
- `task_started`
- `task_completed`
- `task_failed`
- `task_cancelled`

### 4.2 节点事件

例如：

- `queen_started`
- `worker_spawned`
- `worker_connected`
- `worker_dead`
- `worker_shutdown`

### 4.3 派发事件

例如：

- `assignment_created`
- `assignment_assigned`
- `assignment_started`
- `assignment_completed`
- `assignment_failed`
- `assignment_requeued`

### 4.4 协议事件

例如：

- `hello_received`
- `hello_accepted`
- `heartbeat_missed`
- `shutdown_sent`

### 4.5 评估输入事件

例如：

- `evaluation_input_exported`
- `result_recorded`

## 5. 存储建议

建议每个任务目录下至少有：

- `events.jsonl`

使用逐行 JSON，便于追加、检索和回放。

## 6. 与其他层的关系

- 追踪层基于事件构建因果链
- 审计层基于事件生成责任记录
- 恢复机制基于事件判断失败与重试
- 进化面基于事件和结果生成评估输入

## 7. 第一版建议

第一版先做到：

- 事件字段稳定
- 事件类型最小闭环
- 任务、节点、派发三类事件完整
- 事件能支撑基本排错和结果回溯
