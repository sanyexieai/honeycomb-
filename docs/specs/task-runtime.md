# Task Runtime Spec

`Task Runtime` 用于描述某个 Hive 在一个具体任务中的临时运行态。

它的存在是为了将：

- Hive 的长期状态
- 某次任务的短期运行时

明确隔离。

## 1. 设计目标

一个 Hive 可能同时被多个任务复用，因此：

- 不能把每次任务的局部状态直接写回 Hive 长期状态
- 不能让任务内临时参数覆盖全局默认行为
- 必须为每次任务创建独立的运行时 session

## 2. 核心原则

- 长期能力归 Hive
- 长期经验归 Practice Profile
- 当前任务状态归 Task Runtime Session

## 3. 最小示例

```json
{
  "session_id": "sess_001",
  "task_id": "task_123",
  "hive_id": "summarizer",
  "selected_impl": "impl_v2_b",
  "selected_practice": "summarizer_long_docs_v1",
  "lifecycle": "Running",
  "input": {
    "source_text": "..."
  },
  "context": {
    "task_type": "long_document",
    "domain": "general"
  },
  "overrides": {
    "temperature": 0.0
  },
  "local_state": {
    "step_count": 2,
    "intermediate_summary": "..."
  }
}
```

## 4. 字段说明

- `session_id`
  - 任务内该 Hive 实例的唯一标识
- `task_id`
  - 所属任务
- `hive_id`
  - 使用的 Hive
- `selected_impl`
  - 本次选中的实现
- `selected_practice`
  - 本次匹配到的 Practice Profile
- `lifecycle`
  - 当前任务中的生命周期状态
- `input`
  - 当前任务输入
- `context`
  - 当前任务上下文
- `overrides`
  - 本次任务覆盖参数
- `local_state`
  - 本次任务内部临时状态

## 5. 任务内覆盖

`Task Runtime Session` 允许做局部覆盖，例如：

- 模型参数覆盖
- 工具顺序覆盖
- 超时限制覆盖
- 预算限制覆盖

这些覆盖只在当前任务内有效，不应直接写回：

- `ImplementationSpec`
- `PracticeProfile`
- `HiveSpec`

## 6. 生命周期

建议状态：

- `Created`
- `Ready`
- `Running`
- `WaitingInput`
- `WaitingDependency`
- `Completed`
- `Failed`

## 7. 与长期状态的边界

建议：

- `Hive` 保存长期能力、实现池、进化历史
- `PracticeProfile` 保存跨任务经验
- `Task Runtime Session` 保存本次任务上下文和临时状态

任务结束后：

- 可将执行结果、评估指标、产物摘要回写长期记录
- 不应直接把临时局部状态原样提升为长期默认值

## 8. 推荐流程

建议的任务启动流程：

1. 创建 `TaskSpec`
2. 根据任务上下文匹配 `PracticeProfile`
3. 选出 `selected_impl`
4. 创建 `TaskHiveSession`
5. 执行并记录本次评估
6. 将评估结果用于更新实现排名和最佳实践库
