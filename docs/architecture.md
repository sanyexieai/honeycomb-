# Honeycomb 总体架构（激进重构版）

## 1. 系统定位

Honeycomb 是一个 LLM 驱动的蜂群执行与进化系统，采用三层模型：

- 蜂运行时层（bee runtime）：面向任务内执行
- 蜂巢能力层（hive capability）：面向全局能力资产
- 生态进化层（ecology evolution）：面向全量治理优化

不再采用“单一执行二进制持续扩张”路线。

## 2. 三二进制模型

### 2.1 `honeycomb-bee`（运行时执行）

负责 queen/worker 角色运行与任务内协作：

- 以 LLM + 职责提示词文档 + 工具完成执行
- 处理任务上下文、会话状态、局部记忆与产物
- 回传审计与执行证据

硬约束：

- 执行主链路必须可追溯到 LLM 实现体
- 不允许直接写长期注册表和全局最佳实践

### 2.2 `honeycomb`（能力中心）

负责长期能力定义与发布：

- skill、tool、implementation、template、practice 的注册与查询
- 全局默认实现、工具白名单、能力约束配置
- 向 bee 提供能力引用与版本化能力视图

### 2.3 `honeycomb-evolution`（生态进化）

负责跨任务优化与治理：

- 汇总全量运行观测与执行证据
- 评分、谱系维护、晋升淘汰
- 最佳实践沉淀与治理决策执行

## 3. 角色与边界

### 3.1 queen / worker（仅属于 bee 运行时）

- `queen`：任务内规划、派发、汇总
- `worker`：执行单个工作单元并回传结果

### 3.2 hive（全局能力，不是运行时进程）

- 定义长期能力契约与可用实现
- 不负责直接运行任务循环

### 3.3 ecology（全局治理，不是任务调度器）

- 评估长期价值与风险
- 决定默认能力如何演化

## 4. 记忆与上下文

- 任务上下文与短期记忆：在 bee 运行时落地并可重放
- 长期记忆与最佳实现：在 hive/evolution 层治理与发布
- 运行时不得把会话临时结论直接升级为系统默认能力

## 5. 组织原则

- 单任务内允许 queen 主导
- 系统级无唯一总控
- 长期能力与任务执行强隔离
- 全局治理写入必须经过 evolution 正式流程

## 6. 当前架构结论

1. 主二进制固定为：`honeycomb-bee`、`honeycomb`、`honeycomb-evolution`
2. bee 负责运行时智能执行，必须 LLM 驱动
3. honeycomb 负责能力中心，不再承载蜂运行时主入口
4. evolution 负责全量监控视角下的能力进化与治理闭环

## 7. 相关文档

- `specs/bee-runtime-hive-ecology-architecture.md`
- `specs/binary-and-deployment-layout.md`
- `specs/memory-context-and-llm-roles.md`
