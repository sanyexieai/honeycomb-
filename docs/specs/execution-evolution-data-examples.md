# Bee 运行时、能力中心与进化面数据样例

## 1. 目标

本规范通过一个完整例子说明：

- `honeycomb-bee`（运行时）写什么
- `honeycomb`（能力中心）提供什么只读引用
- `honeycomb-evolution`（进化面）读什么、写什么
- 三者如何通过目录与记录衔接

不承诺与旧目录名或旧字段一一兼容；以本规范为后续实现与迁移的准绳。

## 2. 示例目录

```text
skills/                              # 能力中心长期定义（由 hive 流程发布，bee 只读）
  registry/
  implementations/
  ...

runtime/
  tenant/
    tenant_default/
      namespaces/
        user/
          user_123/
            tasks/
              task_xxx/
                task.json
                events.jsonl
                trace.jsonl
                audit.jsonl
                llm/                         # bee：LLM 与提示词执行证据（可追加）
                  traces.jsonl
                  prompt_bundles/
                    <bundle_id>.json
                tool_calls.jsonl               # 工具调用轨迹（可选独立文件）
                queen/
                workers/
                assignments/
                outputs/

evolution/
  tenant/
    tenant_default/
      user/
        user_123/
          evaluations/
          fitness/
          promotions/
          lineages/
          practices/
```

## 3. Bee 运行时关键文件（`runtime/`）

建议至少有：

- `task.json`（含任务级上下文与对实现体、提示词 bundle 的引用）
- `queen/node.json`
- `workers/<worker_id>/node.json`
- `assignments/<assignment_id>.json`
- `outputs/result.json`
- `events.jsonl`
- `trace.jsonl`
- `audit.jsonl`
- `llm/traces.jsonl`（或等价路径：模型调用、请求摘要、证据 ID，**不可**替代审计主链）
- `llm/prompt_bundles/<bundle_id>.json`（职责提示词文档快照，版本可追溯）
- `tool_calls.jsonl`（若未并入 trace，则单独记录工具调用）

## 4. 能力中心关键数据（`skills/` 等，hive）

Bee **只读**引用，不负责写入长期注册表。样例中应出现：

- 正式 `skill` 定义与 `implementation_id`
- 工具白名单与约束（由能力中心物料表达）

进化面消费运行证据时，应能解析上述 ID 与 hive 内定义对齐。

## 5. 进化面关键文件（`evolution/`）

建议至少有：

- `evaluations/<evaluation_id>.json`
- `fitness/<hive_impl>.json`
- `promotions/<promotion_id>.json`
- `lineages/<hive_impl>.json`

## 6. `task.json` 字段示例（示意）

以下字段名可按实现调整，但语义应保留：

```json
{
  "task_id": "task_xxx",
  "tenant_id": "tenant_default",
  "namespace": "user/user_123",
  "status": "running",
  "topology": { "kind": "singleton", "nodes": [] },
  "queen_node_id": "queen-1",
  "implementation_ref": "impl_abc",
  "prompt_bundle_ref": "pbundle_001",
  "llm_trace_refs": ["llm/traces.jsonl#123"],
  "tool_call_refs": ["tool_calls.jsonl#45"]
}
```

## 7. 关键边界

通过样例应体现：

- Bee 只写 `runtime/` 下的短期运行态与执行证据
- 能力中心长期定义落在 `skills/`（及注册表布局），由 hive 发布流程写入，**不由 bee 直接改**
- 进化面只写 `evolution/`
- 晋升记录必须能回指 `runtime/` 中的事件、trace、audit、**LLM/工具证据**
- `tenant_id`、命名空间、`task_id`、实现体标识在三层间贯通

## 8. 相关文档

- `runtime-storage-layout.md`
- `bee-runtime-hive-ecology-architecture.md`
- `task-runtime.md`
