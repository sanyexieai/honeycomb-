# Practice Profile Spec

`PracticeProfile` 描述某个 Hive 在特定上下文下的推荐使用方式。

它的目标不是替代 `Implementation`，而是回答这个问题：

- 对于一个新任务，在当前上下文下，默认应该怎样使用这个 Hive？

## 1. 设计动机

同一个 Hive 可能被不同任务、不同结构复用，例如：

- 长文总结
- 短文本压缩
- 法律文本摘要
- 会议纪要提炼

这些场景可能对应不同的最优实现和策略。因此系统不能只依赖一个全局活跃实现，还需要沉淀“上下文相关的最佳实践”。

## 2. 角色定位

`PracticeProfile` 位于：

- `HiveSpec` 和 `ImplementationSpec` 之上
- `Task Runtime Session` 之前

它负责：

- 匹配任务上下文
- 推荐默认实现
- 推荐默认策略
- 作为新任务的起始标准

## 3. 最小示例

```json
{
  "practice_id": "summarizer_long_docs_v1",
  "hive_id": "summarizer",
  "capability": "summarize_text",
  "context_selector": {
    "task_type": "long_document",
    "domain": "general",
    "topology_type": "pipeline",
    "input_size": "large",
    "budget_class": "standard"
  },
  "recommended_impl": "impl_v2_b",
  "recommended_strategy": {
    "mode": "extract_then_compress",
    "temperature": 0.1,
    "tool_order": ["retrieve_context", "run_script", "rank_relevance"]
  },
  "fitness": {
    "score": 0.89,
    "based_on_runs": 124
  },
  "status": "active"
}
```

## 4. 字段说明

- `practice_id`
  - 最佳实践唯一标识
- `hive_id`
  - 所属 Hive
- `capability`
  - 对应能力
- `context_selector`
  - 场景匹配条件
- `recommended_impl`
  - 推荐实现
- `recommended_strategy`
  - 推荐策略覆盖
- `fitness`
  - 当前实践表现
- `status`
  - 建议值：
    - `active`
    - `candidate`
    - `deprecated`

## 5. 上下文匹配维度

建议支持但不强制所有字段：

- `task_type`
- `domain`
- `topology_type`
- `input_size`
- `budget_class`
- `latency_class`
- `caller_role`
- `downstream_requirement`

这些字段用于决定：

- 新任务优先采用哪一个 Practice Profile

## 6. 竞争机制

同一个 Hive 可同时存在多个 Practice Profile，它们之间是竞争关系。

例如：

- `summarizer_long_docs_v1`
- `summarizer_short_text_fast_v2`
- `summarizer_legal_docs_v1`

系统不应强制收敛成单一全局最优，而应保留：

- 面向不同上下文的局部最优实践

## 7. 生命周期

建议状态：

- `candidate`
- `active`
- `deprecated`

典型流程：

1. 从某次高质量任务运行中提炼新实践
2. 作为 `candidate` 参与后续任务
3. 当样本足够且得分稳定后晋升为 `active`
4. 长期表现退化后降级为 `deprecated`

## 8. 与任务运行时的关系

新任务启动时建议：

1. 根据任务上下文匹配 `PracticeProfile`
2. 选出默认 `recommended_impl`
3. 应用 `recommended_strategy`
4. 创建 `TaskHiveSession`
5. 允许本次运行时做局部覆盖

任务完成后：

- 运行结果回写评估系统
- 可能更新已有 Practice Profile
- 也可能生成新的候选 Practice Profile
