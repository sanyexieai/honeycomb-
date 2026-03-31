# 运行时存储布局（三层模型）

## 1. 目标

本规范定义 Honeycomb 在物理层面如何区分：

- 能力中心长期定义（hive）
- Bee 短期运行态与执行证据
- 进化态
- 产物
- 控制配置

## 2. 顶层目录建议

```text
skills/          # 能力中心：长期稳定定义与正式发布（hive 写入路径）
runtime/         # bee：短期任务状态与 LLM/工具证据
evolution/       # 进化面：长期优化与治理结果
artifacts/       # 重型产物
control/         # 配额与策略配置
```

## 3. 命名空间优先

无论是单用户、多用户还是多租户，建议都先按命名空间归档。

建议层次：

- `global`
- `team/<team_id>`
- `user/<user_id>`

在多租户下，再把租户放在更外层。

## 4. 能力中心布局（`skills/`）

`skills/`（及并列的 `registry/` 等，以实现为准）放长期稳定定义：

- 技能说明与契约
- 实现体定义
- 工具白名单与约束引用
- 被正式晋升的长期资源

**Bee 运行时不得直接写此处**；仅通过只读引用加载。

## 5. Bee 运行态布局（`runtime/`）

`runtime/` 放短期任务状态与**可重放、可审计**的执行证据：

- `task.json`
- `events.jsonl`
- `trace.jsonl`
- `audit.jsonl`
- `llm/traces.jsonl`：LLM 调用证据（摘要或结构化记录，与供应商日志策略对齐）
- `llm/prompt_bundles/`：职责提示词文档版本快照
- `tool_calls.jsonl`：工具调用轨迹（可与 trace 合并，但语义须可查询）
- `queen/`
- `workers/`
- `assignments/`
- `outputs/`

原则：凡进入治理与晋升链路的结论，必须能在 `runtime/` 找到对应证据引用。

## 6. 进化态布局（`evolution/`）

`evolution/` 放长期优化结果：

- `evaluations/`
- `fitness/`
- `promotions/`
- `lineages/`
- `practices/`

## 7. 产物布局（`artifacts/`）

`artifacts/` 放大文件或需要单独归档的结果，例如：

- 大文本输出
- 截图
- 中间生成文件
- 调试快照

## 8. 控制配置布局（`control/`）

`control/` 用于配额、治理和策略配置，例如：

- 租户配额
- 团队配额
- 用户配额
- 策略开关

## 9. 关键写入边界

- `runtime/` 只写短期运行态与 bee 执行证据（含 LLM/工具）
- `evolution/` 只写长期优化与治理结果
- `skills/`（能力中心）只放稳定定义与正式晋升内容，**不由 bee 运行时直写**
- `artifacts/` 存放重型产物
- `control/` 存放治理与配额策略

长期默认能力变更应走 **evolution → hive 发布**，而非写入 `runtime/` 冒充长期状态。

## 10. 相关文档

- `execution-evolution-data-examples.md`
- `bee-runtime-hive-ecology-architecture.md`
