# 执行面代码模块映射草案

## 1. 目标

本文档把执行面已有规格映射为可实现的 Rust 模块边界，避免在落地时重新把任务、协议、调度、恢复和治理耦合在一起。

## 2. 设计原则

- 数据边界先于代码边界
- 文件落盘与领域逻辑分离
- 协议层不直接做调度决策
- 调度层不直接改写长期定义
- 恢复、审计、追踪使用统一运行事实

## 3. 顶层模块建议

建议执行面至少拆成以下一级模块：

- `app`：进程入口、角色启动、配置装配
- `core`：共享核心类型与枚举
- `identity`：节点身份、任务级信任材料
- `protocol`：蜂后与工蜂消息模型、编解码、连接状态
- `runtime`：`TaskSpec`、`TaskRuntime`、`TaskHiveSession`
- `scheduler`：拓扑推进、ready 判断、assignment 派发
- `executor`：assignment 实际执行适配层
- `recovery`：失败分类、重试决策、恢复动作
- `storage`：文件系统读写、快照、追加日志
- `observability`：事件、追踪、审计写入与查询
- `control`：配额、策略、权限前置检查
- `registry`：只读注册表访问
- `trigger`：手动、定时、事件触发
- `resident`：常驻蜂巢生命周期与触发绑定

## 4. 建议目录布局

```text
src/
  main.rs
  app/
  core/
  identity/
  protocol/
  runtime/
  scheduler/
  executor/
  recovery/
  storage/
  observability/
  control/
  registry/
  trigger/
  resident/
```

## 5. 模块职责

### 5.1 `app`

负责：

- 解析 CLI 和配置
- 选择 `queen` 或 `worker` 角色
- 组装依赖
- 启动主循环

不负责：

- 直接定义任务对象细节
- 直接读写运行态文件

### 5.2 `core`

负责：

- 通用 ID 类型
- 通用错误枚举
- 时间与版本字段
- 共享状态枚举

建议放入：

- `TenantId`
- `Namespace`
- `TaskId`
- `NodeId`
- `AssignmentId`
- `ProtocolVersion`

### 5.3 `identity`

负责：

- 节点身份对象
- `queen_token` 等任务级信任材料
- worker 接入前校验上下文

输出给：

- `protocol`
- `scheduler`
- `recovery`

### 5.4 `protocol`

负责：

- `hello`
- `hello_ack`
- `heartbeat`
- `task_assign`
- `task_progress`
- `task_result`
- `shutdown`

建议再拆成：

- `protocol::message`
- `protocol::codec`
- `protocol::queen_session`
- `protocol::worker_session`

### 5.5 `runtime`

负责：

- `TaskSpec`
- `TaskRuntime`
- `TaskHiveSession`
- assignment 运行态视图

关键要求：

- 只表达当前任务事实
- 不承载长期技能定义

### 5.6 `scheduler`

负责：

- 接收任务
- 校验可调度前置条件
- 推进拓扑 ready 节点
- 选择 worker
- 创建和更新 assignment

不负责：

- 执行具体 assignment
- 直接决定长期默认实现

### 5.7 `executor`

负责：

- 将 assignment 转换为实际执行动作
- 屏蔽本地进程、脚本或未来其他执行器差异
- 生成结构化结果

第一版建议只实现本地进程执行器。

### 5.8 `recovery`

负责：

- 失败分类
- 重试预算检查
- `retry_same_worker` / `retry_new_worker`
- 中断任务恢复

依赖：

- `runtime`
- `storage`
- `observability`

### 5.9 `storage`

负责：

- `task.json`
- `events.jsonl`
- `trace.jsonl`
- `audit.jsonl`
- assignment 文件
- 快照和结果文件

原则：

- 领域层不直接拼文件路径
- 追加日志和快照更新分开

### 5.10 `observability`

负责：

- 事件写入
- trace span 写入
- audit 记录写入
- 按任务查询最近状态

输入来源：

- `protocol`
- `scheduler`
- `executor`
- `recovery`
- `trigger`

### 5.11 `control`

负责：

- 租户、团队、用户、任务级配额读取
- 权限前置检查
- 限流与拒绝原因结构化输出

注意：

- 动作授权与对象可见性判断应可单独复用

### 5.12 `registry`

负责：

- 读取技能定义
- 读取默认实现体
- 读取模板、最佳实践、正式状态

第一版建议只读，不写。

### 5.13 `trigger`

负责：

- 触发器对象
- 触发命中判断
- 去重、冷却、窗口限流
- 触发历史记录

### 5.14 `resident`

负责：

- 常驻蜂巢配置
- 常驻蜂巢状态机
- 订阅触发器并生成任务请求

## 6. 关键对象归属

建议对象与模块归属如下：

| 对象 | 主模块 | 说明 |
| --- | --- | --- |
| `TaskSpec` | `runtime` | 任务输入、目标、约束 |
| `TaskRuntime` | `runtime` | 任务当前状态与节点事实 |
| `Assignment` | `scheduler` | 派发对象与依赖信息 |
| `QueenHello` / `WorkerHello` | `protocol` | 通信消息 |
| `NodeIdentity` | `identity` | 节点级身份 |
| `QuotaDecision` | `control` | 配额与策略判断结果 |
| `RecoveryAction` | `recovery` | 恢复动作 |
| `EventRecord` | `observability` | 原始运行事件 |

## 7. 推荐依赖方向

建议依赖方向尽量保持单向：

```text
app
  -> control / registry / resident / trigger / scheduler / protocol
scheduler
  -> runtime / identity / control / executor / recovery / observability / storage
executor
  -> runtime / observability / storage
recovery
  -> runtime / observability / storage
protocol
  -> core / identity / observability
resident
  -> trigger / scheduler / observability
registry
  -> storage
control
  -> storage
observability
  -> storage
```

避免反向依赖：

- `storage` 不依赖 `scheduler`
- `runtime` 不依赖 `protocol`
- `protocol` 不依赖 `scheduler`

## 8. 分阶段实现映射

### 阶段 1

- `app`
- `core`
- `identity`
- `storage`
- `observability`

### 阶段 2

- `protocol`

### 阶段 3

- `runtime`
- `scheduler`
- `executor`

### 阶段 4

- `recovery`
- `control`

### 阶段 5

- `registry`
- `trigger`
- `resident`

## 9. 测试边界建议

建议至少分四层测试：

- `core`/`runtime`/`control` 的纯单元测试
- `protocol` 编解码与状态机测试
- `storage` 落盘与恢复测试
- 从 `queen` 到 `worker` 的端到端最小闭环测试

## 10. 总结

执行面代码边界应围绕：

- 任务事实
- 协议事实
- 调度决策
- 恢复决策
- 文件持久化

这五类责任展开。只要边界先稳住，后续替换执行器、增加触发器或引入多节点通信时，都不需要重写整体结构。
