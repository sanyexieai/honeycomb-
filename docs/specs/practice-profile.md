# 最佳实践档案规范

## 1. 目标

最佳实践档案用于沉淀跨任务经验，表达某种上下文下更适合的实现和使用方式。

## 2. 作用位置

它位于长期技能定义与单次任务运行之间，负责把“怎么用更好”沉淀下来。

## 3. 建议字段

建议至少包含：

- `practice_id`
- `hive_id`
- `capability`
- `context_selector`
- `recommended_impl`
- `recommended_strategy`
- `fitness_score`
- `based_on_runs`

## 4. 核心原则

- 最佳实践是面向上下文的局部最优建议
- 最佳实践不是全局真理
- 最佳实践需要有评分和历史证据支撑
- 最佳实践的晋升应经过显式流程

## 5. 与其他文档的关系

- 技能定义见 `hive-spec.md`
- 实现体与基因见 `implementation-spec.md`
- 晋升流程见 `fitness-and-promotion.md`
