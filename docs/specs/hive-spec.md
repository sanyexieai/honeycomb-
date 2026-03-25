# Hive Spec

`hive.md` 是固定能力契约。它描述一个 Hive 是什么，而不是描述某次具体如何执行。

## 1. 设计目标

`hive.md` 用来保证：

- Hive 的能力边界稳定
- 其他 Hive 能稳定依赖它
- 实现体的替换不会破坏系统协议

## 2. 建议格式

推荐采用：

- YAML frontmatter 放结构化字段
- 正文放规则、成功标准、协作说明、示例

## 3. 最小示例

```md
---
id: summarizer
name: Summarizer Hive
version: 0.1.0
kind: hive
capability: summarize_text
description: 将输入文本压缩为结构化摘要

interface:
  input_schema: interface/input.schema.json
  output_schema: interface/output.schema.json

execution:
  executor: composite
  entrypoint: summarize
  timeout_ms: 30000
  max_steps: 6

state:
  persistent: true
  slots:
    - current_task
    - last_result
    - retry_count

dependencies:
  required_capabilities:
    - retrieve_context
  optional_capabilities:
    - rank_relevance

tools:
  allowed:
    - read_file
    - write_file
    - run_script
    - run_binary

evaluation:
  metrics:
    - accuracy
    - brevity
    - format_compliance
    - latency
    - downstream_acceptance
  fitness_formula: "0.35*accuracy + 0.2*brevity + 0.2*format_compliance + 0.15*downstream_acceptance - 0.1*latency"

evolution:
  implementation_mutable: true
  interface_mutable: false
  split_allowed: true
  recommendable: true
---

# Purpose
负责把原始文本整理为短摘要或结构化摘要。

# Rules
- 不得编造输入中不存在的信息
- 输出必须符合 output schema
- 缺少必要上下文时返回 clarification_needed

# Collaboration
- 当输入过长时，可以调用具备 retrieve_context 能力的 hive
- 当存在多个候选摘要时，可以调用具备 rank_relevance 能力的 hive

# Success Criteria
- 事实保真
- 长度可控
- 结构稳定
- 可被下游 hive 直接消费
```

## 4. 字段定义

### 顶层字段

- `id`
  - Hive 的稳定标识
- `name`
  - 人类可读名称
- `version`
  - 契约版本
- `kind`
  - 当前建议固定为 `hive`
- `capability`
  - 该 Hive 对外暴露的稳定能力名
- `description`
  - 简要描述

### `interface`

- `input_schema`
  - 输入 JSON Schema 路径
- `output_schema`
  - 输出 JSON Schema 路径

### `execution`

- `executor`
  - 推荐值：
    - `llm`
    - `deterministic`
    - `composite`
    - `process`
- `entrypoint`
  - 执行入口名称
- `timeout_ms`
  - 超时限制
- `max_steps`
  - 最大内部执行步数

### `state`

- `persistent`
  - 是否持久化运行状态
- `slots`
  - 预定义状态槽位名称

### `dependencies`

- `required_capabilities`
  - 运行此 Hive 必须能调用的能力
- `optional_capabilities`
  - 可选依赖能力

### `tools`

- `allowed`
  - 允许使用的工具标识

### `evaluation`

- `metrics`
  - 评估指标列表
- `fitness_formula`
  - 综合评分公式

### `evolution`

- `implementation_mutable`
  - 是否允许变更实现体
- `interface_mutable`
  - 是否允许变更输入输出协议
- `split_allowed`
  - 是否允许分裂出同能力子实现
- `recommendable`
  - 是否允许被推荐给其他 Hive

## 5. 正文 Section 约定

建议正文支持以下 section：

- `# Purpose`
- `# Rules`
- `# Collaboration`
- `# Success Criteria`
- `# Failure Modes`
- `# Examples`

解析器可以先做宽松支持，不强制要求所有 section 都存在。

## 6. 约束规则

### 固定项

以下内容原则上不允许自动演化修改：

- `capability`
- 输入输出 schema 的语义
- 对外行为契约

### 可演化项

以下内容可以由实现体承载并演化：

- Prompt
- 工具调用顺序
- 执行策略参数
- 脚本或二进制版本

## 7. 兼容性要求

任意新的 Implementation 必须满足：

- 仍属于同一个 `capability`
- 输入输出 schema 版本兼容
- 行为约束不与 `Rules` 冲突
- 可被既有下游 Hive 消费
