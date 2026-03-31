# 激进切换施工单（无兼容承诺）

## 1. 目标

本文件描述从「旧执行面单入口」切换到 **三二进制、三层职责** 时的推荐施工顺序。

- 不保留 CLI 别名、不保留旧语义并行期
- 以文档 `bee-runtime-hive-ecology-architecture.md`、`binary-and-deployment-layout.md` 为准
- 实现可一次性破坏性重构

## 2. 目标终态（固定）

| 二进制 | 职责 |
| --- | --- |
| `honeycomb-bee` | queen/worker、LLM、提示词 bundle、工具调用、任务内证据 |
| `honeycomb` | 能力中心：注册、查询、全局工具与默认实现发布入口 |
| `honeycomb-evolution` | 全量观测汇总、评分、谱系、治理与晋升 |

## 3. 切换阶段

### 阶段 A：二进制与入口冻结

1. 新增 crate/目标：`honeycomb-bee`（或等价 bin 名），与现有 `honeycomb` 分离。
2. 将 `queen run`、`worker run`、任务内派发与 assignment 执行主链路**迁移**到 `honeycomb-bee`。
3. 删除 `honeycomb` 内与 queen/worker 实时执行循环绑定的入口（或整段移除，不保留兼容开关）。

### 阶段 B：能力中心收口

1. 在 `honeycomb` 上仅保留：技能/工具/实现体/模板/实践 **注册与查询**、发布与治理对接入口。
2. 将原先混在执行 CLI 里的「长期写入」能力**全部**迁出 bee 路径，或删除（见 `current-capability-audit-and-aggressive-convergence.md` 精神）。
3. Bee 通过只读客户端读取能力中心物料（见 `execution-code-module-mapping.md` 中 `registry_client`）。

### 阶段 C：进化面消费证据

1. 在 `runtime/` 下落地 `llm/`、`tool_calls` 等证据路径（见 `runtime-storage-layout.md`）。
2. `honeycomb-evolution` 的评估输入**必须**能解析上述证据引用。
3. 治理输出只写 `evolution/` 与经 hive 发布的 `skills/`，不直接回写 bee 运行时。

### 阶段 D：验证与删旧

1. 端到端：提交任务 → bee 执行 → 落盘证据 → evolution 可消费 → hive 可发布新默认。
2. 删除旧文档段落、旧命令、旧目录约定中与新模型冲突的部分（本仓库已分批更新文档）。
3. 更新 CI/发布说明：三二进制产物与安装顺序。

## 4. 明确禁止

- 在 `honeycomb-bee` 内写长期注册表或全局最佳实践
- 在 `honeycomb` 内跑 queen/worker 主循环
- 用「临时兼容层」长期保留两套 CLI 语义

## 5. 相关文档

- `bee-runtime-hive-ecology-architecture.md`
- `binary-and-deployment-layout.md`
- `execution-plane-implementation-roadmap.md`
- `migration-cutover-plan.md`（本文）
