# 审计与追踪

## 1. 目标

本规范定义 Honeycomb 如何记录责任、因果链和可追溯信息。

需要回答：

- 谁做了什么
- 发生在什么任务和节点上
- 影响了哪些运行态或长期态
- 后续如何排查、回放和治理

## 2. 两个概念

### 2.1 审计

审计关注责任与治理，例如：

- 谁创建了任务
- 谁派生了工蜂
- 谁晋升了某个实现体
- 谁废弃了某个最佳实践

### 2.2 追踪

追踪关注执行过程与因果链，例如：

- 一个派发单元何时开始
- 由哪个工蜂执行
- 为什么失败
- 是否发生了重试或替换

## 3. 与运行时事件的关系

建议三层关系：

- `events.jsonl`：原始运行事实
- `trace.jsonl`：因果链与执行跨度
- `audit.jsonl`：责任与治理记录

## 4. 最小审计字段

建议至少包含：

- `audit_id`
- `timestamp`
- `actor_type`
- `actor_id`
- `tenant_id`
- `namespace`
- `action`
- `target_type`
- `target_id`
- `task_id`
- `node_id`
- `result`
- `payload`

## 5. 最小追踪字段

建议至少包含：

- `trace_id`
- `span_id`
- `parent_span_id`
- `timestamp`
- `event_type`
- `task_id`
- `node_id`
- `assignment_id`
- `tenant_id`
- `namespace`
- `status`
- `payload`

## 6. 审计域

建议至少覆盖：

- bee 运行时审计
- 进化面审计
- 跨层（bee / evolution 等）交接审计

## 7. 追踪域

建议至少覆盖：

- 任务追踪
- 节点追踪
- 派发追踪
- 晋升追踪

## 8. 存储建议

每个任务目录下建议包含：

- `events.jsonl`
- `trace.jsonl`
- `audit.jsonl`

进化面建议在 `evolution/` 下记录长期审计与治理轨迹。

## 9. 原则

- 原始事件只追加
- 审计记录保留时间长于任务追踪
- 追踪优先服务调试与恢复
- 审计优先服务治理与责任归因
