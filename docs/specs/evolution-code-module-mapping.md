# 进化面代码模块映射草案

## 1. 目标

本文档把进化面的评分、治理、谱系和注册表联动映射成可实现的模块边界。

## 2. 顶层模块建议

- `app`：进程入口、批处理任务启动
- `core`：共享对象与枚举
- `ingest`：读取 bee 运行时导出的评估输入
- `fitness`：指标归一化、评分计算、报告生成
- `governance`：`promote`、`hold`、`deprecate`、`observe`
- `lineage`：实现体谱系与来源关系
- `practice`：最佳实践沉淀与作用域发布
- `registry`：正式注册表写入适配
- `storage`：`evolution/` 目录读写
- `audit`：长期治理审计
- `policy`：晋升阈值、审批策略、证据门槛

## 3. 核心职责边界

### 3.1 `ingest`

负责：

- 读取 bee 运行时导出的评估输入
- 校验最小上下文
- 去重和批次归档

### 3.2 `fitness`

负责：

- 生成 `FitnessReport`
- 汇总成本、延迟、正确性等指标
- 产出可审计评分依据

### 3.3 `governance`

负责：

- 根据策略生成治理建议
- 判断是否需要人工审批
- 输出正式治理动作

### 3.4 `lineage`

负责：

- 维护 parent-child 关系
- 记录实现体来源
- 支撑回滚与比较

### 3.5 `practice`

负责：

- 沉淀最佳实践
- 区分用户、团队、租户内发布范围
- 给模板层提供引用

### 3.6 `registry`

负责：

- 更新默认实现体
- 更新最佳实践正式状态
- 保留变更历史

### 3.7 `policy`

负责：

- 阈值配置
- 审批门槛
- 风险分级
- 自动化允许范围

## 4. 建议依赖方向

```text
app
  -> ingest / fitness / governance / registry
governance
  -> fitness / lineage / practice / policy / audit / storage
fitness
  -> storage
registry
  -> storage / audit
practice
  -> storage
lineage
  -> storage
audit
  -> storage
```

## 5. 最小对象归属

| 对象 | 主模块 | 说明 |
| --- | --- | --- |
| `EvaluationInput` | `ingest` | bee 运行时导入的评估输入 |
| `FitnessReport` | `fitness` | 评分结果 |
| `GovernanceDecision` | `governance` | 晋升或观察决定 |
| `LineageRecord` | `lineage` | 谱系记录 |
| `PracticeProfile` | `practice` | 最佳实践档案 |
| `RegistryUpdate` | `registry` | 注册表变更 |

## 6. 分阶段实现映射

### 阶段 1

- `ingest`
- `storage`
- `audit`

### 阶段 2

- `fitness`

### 阶段 3

- `policy`
- `governance`

### 阶段 4

- `registry`

### 阶段 5

- `lineage`
- `practice`

## 7. 关键原则

- bee 运行时只提交证据，不直接下长期结论
- 评分与治理分层，便于替换公式
- 注册表更新必须经过审计
- 自动化策略必须受 `policy` 限制

## 8. 总结

进化面的实现边界应围绕“读证据、算分数、下决策、写长期状态”这条链展开，避免把评分、治理和注册表写入混成一个不可审计的批处理脚本。
