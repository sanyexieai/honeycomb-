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

## 9. 当前落地状态

当前代码已经落下最小正式实现体对象：

- `ImplementationRecord`
- `ImplementationEntry`
- `ImplementationCompatibility`
- `ImplementationOrigin`

当前代码入口：

- `src/registry/mod.rs`
- `src/storage/registry_store.rs`
- `src/app/evolution.rs`

当前已具备：

- 实现体对象结构化定义
- `registry/implementations/*.json` 落盘
- `persist/load/update/list` 存储接口
- `honeycomb-evolution implementation inspect`
- `honeycomb-evolution implementation list`
- `implementation inspect/list` 已可显示近期 30 天 guardrail 命中次数、最高频原因、当前推荐数、运行任务数、活跃任务数
- `FitnessReport` 与 `EvolutionPlan` 已内嵌最小实现体快照
- `governance plan/apply` 已开始直接消费 `strategy.mode`、`components.prompt`、`constraints.max_cost/max_latency_ms`
- 技能默认实现与推荐实现的对象存在性校验
- `task submit/demo-flow` 对实现体引用的入口校验
- `task submit/demo-flow` 已开始把最小 `implementation_snapshot` 写入任务运行态记录
- `task assign` 已开始把最小 `implementation_snapshot` 写入 assignment 运行态记录
- `skill inspect/list/execute` 对实现体绑定关系的校验
- `registry sync` / `registry overview` 对实现体对象的基础联动校验
- `registry overview --with-details` 已可显示“近期常触发 guardrail 且仍被推荐或活跃使用”的 implementation hotspot 视图
- `registry sync` 已对极端高风险实现体启用跳过与降权排序
- `governance plan/apply` 候选选择已接入同一套风险护栏，显式指定实现体时允许绕过默认保守策略
- 护栏命中结果已写入 evolution audit，可作为 review/reflection 的正式输入
- 实现体 `constraints` 已可承载 review refresh 策略，例如绝对增量阈值、倍数阈值、严重性增量阈值和各因子权重
- 技能对象已支持 `governance_policy`，可承载同一技能下实现体共享的 review refresh 默认策略
- 全局对象 `GovernanceDefaultsRecord` 已落地，可承载系统级治理默认策略
- review refresh 与严重性权重已支持四层优先级：implementation `constraints` 覆盖 skill `governance_policy`，再覆盖 global governance defaults，最后回退系统默认值
- 进化面已提供 `governance-defaults inspect`，可直接查看当前全局治理默认策略对象
- 进化面已提供 `governance-defaults set`，可增量写入或清理全局治理默认策略键
- `registry overview --with-details` 已可直接显示当前 global governance defaults 摘要与生效键列表
- `implementation hotspot` 视图已可直接显示 refresh/严重性参数的命中来源：`implementation`、`skill`、`global`、`built_in`

当前仍未完成：

- 运行态 `implementation_ref` 全面切换到正式实现体引用
- `governance` 更完整地直接消费 `ImplementationRecord.components/strategy/constraints` 全量字段
- 技能默认实现与推荐实现的统一对象化

## 10. 治理策略优先级

当前实现体相关治理策略已经支持分层覆盖。

建议优先级如下：

1. implementation `constraints`
2. skill `governance_policy`
3. global governance defaults
4. 系统内建默认值

当前已接入的 refresh / 严重性策略键包括：

- `review_refresh_min_absolute_increase`
- `review_refresh_min_multiplier`
- `review_refresh_min_severity_delta`
- `review_severity_weight_recommended_by`
- `review_severity_weight_active_tasks`
- `review_severity_weight_severe_flags`

这意味着同一技能下的多个实现体可以共享一组默认治理节奏，而少数特殊实现体再用自身 `constraints` 做更细粒度覆盖。

当前全局默认策略对象已使用 `GovernanceDefaultsRecord` 落到 `registry/governance-defaults.json`。
当前最小治理入口包括：

- `honeycomb-evolution governance-defaults inspect`
- `honeycomb-evolution governance-defaults set --policy KEY=VALUE`
- `honeycomb-evolution governance-defaults set --clear-policy KEY`
