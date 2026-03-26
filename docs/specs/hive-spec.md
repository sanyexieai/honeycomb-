# 蜂巢规范

## 1. 目标

本规范定义一个技能或蜂巢的稳定能力契约。

## 2. 建议结构

建议一个蜂巢定义至少包含：

- 基本身份信息
- 能力名称
- 输入输出协议
- 执行约束
- 状态槽位
- 依赖能力
- 可用工具
- 评估方式
- 演化策略

## 3. 推荐写法

建议使用两层结构：

- Frontmatter：放结构化字段
- 正文：放角色说明、规则、协作方式、成功标准、失败模式

## 4. 最小示例

```md
---
id: summarizer
name: 文本摘要蜂巢
version: 0.1.0
capability: summarize_text
interface:
  input_schema: interface/input.schema.json
  output_schema: interface/output.schema.json
execution:
  mode: assignment
  timeout_ms: 30000
state:
  persistent: true
  slots:
    - current_task
    - last_result
    - retry_count
dependencies:
  required_capabilities: []
tools:
  allowed:
    - read_file
    - write_file
evaluation:
  metrics:
    - accuracy
    - brevity
    - format_compliance
evolution:
  implementation_mutable: true
  interface_mutable: false
---

# 目的
负责把输入文本整理为可消费的摘要结果。

# 规则
- 不得编造事实
- 输出必须符合 output schema

# 协作
- 必要时可调用检索类能力补充上下文

# 成功标准
- 摘要准确
- 格式稳定
- 可被下游蜂巢直接消费
```

## 5. 字段表

| 字段 | 必填 | 类型 | 含义 | 是否长期稳定 |
| --- | --- | --- | --- | --- |
| `id` | 是 | string | 蜂巢唯一标识 | 是 |
| `name` | 是 | string | 人类可读名称 | 否 |
| `version` | 是 | string | 契约版本 | 是 |
| `capability` | 是 | string | 能力名称 | 是 |
| `interface` | 是 | object | 输入输出协议定义 | 是 |
| `execution` | 是 | object | 执行约束与模式 | 否 |
| `state` | 否 | object | 状态槽位与持久化定义 | 否 |
| `dependencies` | 否 | object | 依赖能力声明 | 否 |
| `tools` | 否 | object | 可用工具范围 | 否 |
| `evaluation` | 是 | object | 评估指标 | 是 |
| `evolution` | 是 | object | 演化策略边界 | 是 |

## 6. 关键原则

- 能力契约长期稳定
- 任务运行时不直接改动蜂巢定义
- 长期变更由进化面或治理流程推动
- 能力与具体实现体分离

## 7. 正文建议章节

正文建议统一约定：

- `目的`
- `规则`
- `协作`
- `成功标准`
- `失败模式`
- `示例`

## 8. 与其他文档的关系

- 实现体与基因见 `implementation-spec.md`
- 最佳实践见 `practice-profile.md`
- 评分与晋升见 `fitness-and-promotion.md`
- 数据落地样例见 `execution-evolution-data-examples.md`
