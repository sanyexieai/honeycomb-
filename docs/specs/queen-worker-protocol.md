# 蜂后与工蜂协议

## 1. 目标

本协议定义蜂后与工蜂之间的最小通信模型。

目标是先保证：

- 能注册
- 能派发
- 能回报
- 能心跳
- 能关闭

## 2. 协议原则

- 同一任务内通信
- 同一租户内通信
- 同一命名空间边界内通信
- 工蜂只与所属蜂后通信
- 消息使用统一包结构

## 3. 最小消息类型

第一版建议至少支持：

- `hello`
- `hello_ack`
- `heartbeat`
- `task_assign`
- `task_progress`
- `task_result`
- `shutdown`

## 4. 统一消息包结构

建议字段：

- `msg_id`
- `kind`
- `protocol_version`
- `from`
- `to`
- `task_id`
- `timestamp`
- `payload`

## 5. 消息语义

### 5.1 `hello`

工蜂向蜂后发起注册，携带自身身份、租户、命名空间、任务和校验信息。

### 5.2 `hello_ack`

蜂后确认是否接受该工蜂加入当前任务蜂群。

### 5.3 `heartbeat`

工蜂周期性回报存活状态和当前执行状态。

### 5.4 `task_assign`

蜂后向工蜂下发单个派发单元。

### 5.5 `task_progress`

工蜂回报当前派发单元的进度信息。

### 5.6 `task_result`

工蜂回报派发单元的执行结果，可以成功也可以失败。

### 5.7 `shutdown`

蜂后要求工蜂优雅退出。

## 6. 协议约束

必须校验：

- `protocol_version`
- `tenant_id`
- `namespace`
- `task_id`
- `queen_node_id`
- `queen_token` 或同类信任材料

## 7. 状态机建议

### 7.1 蜂后侧

- 监听中
- 已接受工蜂
- 已派发任务
- 等待结果
- 回收工蜂

### 7.2 工蜂侧

- 启动中
- 已连接蜂后
- 空闲
- 执行中
- 已回报结果
- 已退出

## 8. 第一版明确不支持

- 工蜂直接连接工蜂
- 工蜂跨任务迁移
- 跨租户通信
- 未授权的自由派生
- 广播式消息总线

## 9. 与其他文档的关系

- 身份字段见 `node-identity.md`
- 拓扑与派发见 `task-topology.md`
- 事件记录见 `runtime-event-schema.md`
- 恢复语义见 `failure-recovery.md`
