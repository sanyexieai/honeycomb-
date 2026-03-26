# 实现体与基因规范

## 1. 目标

本规范定义蜂巢能力契约之下的可变实现层。

## 2. 实现体

实现体描述某个技能当前如何被执行。

建议包含：

- `implementation_id`
- 执行方式
- 组件路径
- 运行策略
- 兼容性信息
- 来源信息
- 约束信息

## 3. 实现体最小示例

```json
{
  "implementation_id": "impl_v2",
  "skill_id": "summarizer",
  "executor": "worker_process",
  "entry": {
    "kind": "script",
    "path": "scripts/run_summary.py"
  },
  "components": {
    "prompt": "prompts/system.md",
    "config": "config/runtime.json"
  },
  "strategy": {
    "mode": "extract_then_summarize",
    "temperature": 0.2
  },
  "compatibility": {
    "capability": "summarize_text",
    "input_schema_version": "1.0.0",
    "output_schema_version": "1.0.0"
  },
  "constraints": {
    "max_cost": 0.02,
    "max_latency_ms": 5000
  },
  "origin": {
    "source": "mutation",
    "parent_impl": "impl_v1"
  }
}
```

## 4. 基因

基因用于描述实现体允许变化的空间。

建议包含：

- 可变字段
- 不可变字段
- 变异策略
- 允许的候选值或范围
- 变异次数或强度限制

## 5. 基因最小示例

```json
{
  "implementation_id": "impl_v2",
  "mutable_fields": {
    "strategy.temperature": {
      "type": "float",
      "min": 0.0,
      "max": 0.8,
      "step": 0.1
    },
    "components.prompt": {
      "type": "enum",
      "values": [
        "prompts/system.md",
        "prompts/strict.md"
      ]
    }
  },
  "immutable_fields": [
    "compatibility.capability",
    "compatibility.input_schema_version",
    "compatibility.output_schema_version"
  ],
  "mutation_policy": {
    "max_mutations_per_generation": 2,
    "allow_component_swap": true,
    "allow_freeform_code_edit": false
  }
}
```

## 6. 字段表

| 字段 | 必填 | 类型 | 含义 | 说明 |
| --- | --- | --- | --- | --- |
| `implementation_id` | 是 | string | 实现体 ID | 同一技能下唯一 |
| `skill_id` | 是 | string | 所属技能 | 指向稳定能力契约 |
| `executor` | 是 | string | 执行方式 | 如 worker_process |
| `entry` | 是 | object | 启动入口 | 脚本、二进制或其他入口 |
| `components` | 否 | object | 资源组件集合 | prompt、config 等 |
| `strategy` | 否 | object | 运行策略 | 参数、模式、顺序等 |
| `compatibility` | 是 | object | 协议兼容信息 | 防止错配 |
| `constraints` | 否 | object | 成本和时延等约束 | 用于执行面控制 |
| `origin` | 否 | object | 来源信息 | 手工、变异、导入等 |

## 7. 关键原则

- 能力契约稳定，实现体可演化
- 基因限制进化空间，避免无边界漂移
- 长期修改由进化面负责，不由运行时直接改写

## 8. 与其他文档的关系

- 能力契约见 `hive-spec.md`
- 最佳实践见 `practice-profile.md`
- 进化系统见 `evolution-system.md`
- 数据落地样例见 `execution-evolution-data-examples.md`
