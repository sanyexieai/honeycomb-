# 故障恢复

## 1. 目标

本规范定义 Honeycomb 在执行面如何检测、分类、控制和恢复失败。

## 2. 失败域

建议至少区分：

- 工蜂失败
- 蜂后失败
- 派发单元失败
- 任务失败

## 3. 失败分类

建议第一版使用：

- `transient`：暂时性错误，可重试
- `deterministic`：确定性错误，重试意义不大
- `timeout`：超时
- `resource_limit`：资源或配额限制
- `protocol_error`：协议错误
- `cancellation`：主动取消
- `unknown`：未知错误

## 4. 恢复动作

建议至少支持：

- `retry_same_worker`
- `retry_new_worker`
- `skip_assignment`
- `fail_task`
- `mark_worker_dead`
- `manual_intervention_required`

## 5. 第一版恢复原则

第一版建议支持：

- 工蜂死亡检测
- 派发超时检测
- 有界重试
- 为可重试派发更换工蜂
- 持久化中断任务状态

第一版不应假装支持：

- 蜂后自动接管
- 分布式选主
- 崩溃后工蜂自动重连恢复整任务

## 6. 持久化要求

恢复依赖以下持久化数据：

- `task.json`
- 节点身份文件
- 派发记录
- 运行事件
- 追踪记录
- 审计记录
- 结果快照

## 7. 与追踪和审计的关系

恢复动作必须进入：

- `trace.jsonl`
- `audit.jsonl`

因为恢复本身会改变系统行为。

## 8. 当前建议

先把失败显式化、证据保留下来，再逐步增强自动恢复能力。
