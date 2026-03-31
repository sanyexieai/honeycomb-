# 架构审查记录对象

## 1. 目标

本文档定义 Honeycomb 中 `ArchitectureReviewRecord` 的最小数据模型，用于将高风险架构边界审查正式落盘。

## 2. 定位

`ArchitectureReviewRecord` 不负责审批动作，也不直接替代进化治理决策。

它的职责是记录：

- 某项改动是否越界
- 越界风险位于哪里
- 审查结论是什么
- 后续需要补哪些动作

一句话：

它回答“这项改动在架构边界上是否成立”。

## 3. 最小字段

建议至少包含：

- `schema_version`
- `review_id`
- `title`
- `change_scope`
- `requested_by`
- `target_plane`
- `target_modules`
- `writes_runtime`
- `writes_long_term`
- `mutates_historical_facts`
- `touches_registry`
- `touches_approval_or_policy`
- `status`
- `decision`
- `rationale`
- `required_followups`
- `evidence_refs`
- `created_at`
- `updated_at`

## 4. 枚举建议

### 4.1 `target_plane`

- `bee_runtime`
- `hive_capability`
- `evolution`
- `cross_layer`

说明：旧数据或示例中可能出现 `execution`，语义等同于 `bee_runtime`，新记录应使用上表枚举。

### 4.2 `status`

- `open`
- `completed`

### 4.3 `decision`

- `pass`
- `pass_with_followup`
- `needs_redesign`
- `blocked`

## 5. 落盘建议

第一版建议按单文件对象落盘：

- `evolution/reviews/<review_id>.json`

后续可扩展：

- 汇总索引
- 统计视图
- 关联治理记录

## 6. 最小示例

```json
{
  "schema_version": "1.0.0",
  "review_id": "arch-review-2026-03-28-001",
  "title": "task backfill implementation behavior",
  "change_scope": "execution_cli_command",
  "requested_by": "local-dev",
  "target_plane": "bee_runtime",
  "target_modules": [
    "app",
    "runtime",
    "storage"
  ],
  "writes_runtime": true,
  "writes_long_term": false,
  "mutates_historical_facts": true,
  "touches_registry": false,
  "touches_approval_or_policy": false,
  "status": "completed",
  "decision": "needs_redesign",
  "rationale": "would overwrite historical task facts using later governance recommendations",
  "required_followups": [
    "convert to suggestion-only output",
    "preserve original task and assignment facts"
  ],
  "evidence_refs": [
    "docs/specs/execution-vs-evolution-plane.md",
    "docs/specs/domain-boundaries.md"
  ],
  "created_at": "unix_ms:1760000000000",
  "updated_at": "unix_ms:1760000001000"
}
```

## 7. 第一版建议

第一版最少做到：

- 能记录高风险改动的审查结论
- 能按 `review_id` 读取
- 能列出已有记录
- 能和审计记录形成最小关联

## 8. 总结

`ArchitectureReviewRecord` 是 Honeycomb 防止架构边界持续漂移的基础对象之一。

它的价值不在于取代设计文档，而在于把“边界判断”变成可以检索、回放、对比的系统记录。
