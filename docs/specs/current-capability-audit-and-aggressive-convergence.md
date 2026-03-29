# 当前能力盘点与激进收敛方案

## 1. 目标

本文档用于整理 Honeycomb 当前阶段已经暴露出来的功能与能力，并基于已有冲突点给出一份激进收敛方案。

本方案明确采用以下策略：

- 有错必改
- 不为已确认错误的设计保留兼容
- 优先清理边界冲突，再考虑后续重建

## 2. 当前能力分组

### 2.1 执行面

当前执行面已经具备：

- 蜂后与工蜂最小运行入口
- 任务提交、派发、结果回写、重跑、回放、查看
- assignment、trigger、resident 生命周期操作
- task、trace、audit、execution 查询
- scheduler 单次与循环推进
- runtime/system 总览与告警
- skill 读取与执行
- tool 读取与执行
- 部分技能、工具、审批、告警相关控制命令

### 2.2 进化面

当前进化面已经具备：

- fitness 记录与解释
- governance 计划与应用
- registry sync 与 registry overview
- lineage show
- evolution audit 查询
- architecture review 的记录、查看、列出

### 2.3 护栏与收敛

当前已经具备：

- 架构审查文档
- 审查记录对象
- 定期反思机制文档
- 收敛路线图文档

## 3. 已确认冲突

### 3.1 执行面长期写入冲突

以下能力与“执行面不直接改长期状态”的原则冲突：

- `skill register`
- `tool register`
- `tool request-shell`
- `tool authorize-shell`
- `tool revoke-shell`
- `tool approval-alert-ack`
- `tool approval-alert-unack`

这些能力会让执行面承担长期注册表、审批与策略状态写入职责，应立即收回。

### 3.2 语义失真命令冲突

以下命令名称与当前真实行为已经不一致：

- `task backfill-implementation`

该命令已不再执行回填，而是只输出建议，继续保留原命令名会误导用户，应直接移除。

### 3.3 对外暴露但未闭环的占位命令冲突

以下命令已经出现在 CLI 中，但实际仍为 scaffold：

- `practice publish`

这种能力会制造“看起来已经支持，实际上没有闭环”的假象，应直接移除。

## 4. 激进收敛原则

- 删除错误能力，优先于重命名或兼容提示
- 删除占位入口，优先于保留 scaffold
- 删除错误写路径，优先于继续扩展读取能力
- 不保留旧命令别名
- 不为已确认越界能力提供临时兼容层

## 5. 本轮执行动作

### 5.1 立即删除的执行面命令

- `task backfill-implementation`
- `skill register`
- `tool register`
- `tool request-shell`
- `tool authorize-shell`
- `tool revoke-shell`
- `tool approval-alert-ack`
- `tool approval-alert-unack`

### 5.2 立即删除的进化面占位命令

- `practice publish`

### 5.3 暂时保留的只读能力

以下能力虽然与长期治理有关，但当前只读，可暂时保留：

- `skill inspect`
- `skill list`
- `tool inspect`
- `tool list`
- `tool approval-inspect`
- `tool approval-list`
- `tool approval-queue`
- `tool approval-overdue`
- `tool approval-alerts`
- `tool approval-inbox`
- `registry overview`
- `review inspect`
- `review list`

## 6. 删除后的系统边界

本轮完成后，系统边界应收敛为：

- 执行面负责运行、查询、读注册表、执行工具与技能
- 进化面负责评分、治理、同步长期状态、记录架构审查
- 错误的长期写路径不再通过执行面暴露
- 纯占位命令不再对外暴露

## 7. 后续重建方向

被删除的能力如需回归，必须满足：

- 明确所属平面
- 明确写入对象是运行态还是长期态
- 有正式审查记录
- 有对应 spec 和测试

## 8. 状态

- 文档状态：已建立
- 本轮目标：已完成首轮执行
- 已删除：执行面的长期写入命令、语义失真命令、进化面的占位命令
