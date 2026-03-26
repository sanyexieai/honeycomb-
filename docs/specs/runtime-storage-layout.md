# 运行时存储布局

## 1. 目标

本规范定义 Honeycomb 在物理层面如何区分：

- 技能定义
- 运行态
- 进化态
- 产物
- 控制配置

## 2. 顶层目录建议

```text
skills/
runtime/
evolution/
artifacts/
control/
```

## 3. 命名空间优先

无论是单用户、多用户还是多租户，建议都先按命名空间归档。

建议层次：

- `global`
- `team/<team_id>`
- `user/<user_id>`

在多租户下，再把租户放在更外层。

## 4. 技能布局

`skills/` 放长期稳定定义：

- 技能说明
- 实现体定义
- 基因定义
- 最佳实践
- 被正式晋升的长期资源

运行时不应直接写这里。

## 5. 运行态布局

`runtime/` 放短期任务状态：

- `task.json`
- `events.jsonl`
- `trace.jsonl`
- `audit.jsonl`
- `queen/`
- `workers/`
- `assignments/`
- `outputs/`

## 6. 进化态布局

`evolution/` 放长期优化结果：

- `evaluations/`
- `fitness/`
- `promotions/`
- `lineages/`
- `practices/`

## 7. 产物布局

`artifacts/` 放大文件或需要单独归档的结果，例如：

- 大文本输出
- 截图
- 中间生成文件
- 调试快照

## 8. 控制配置布局

`control/` 用于配额、治理和策略配置，例如：

- 租户配额
- 团队配额
- 用户配额
- 策略开关

## 9. 关键写入边界

- `runtime/` 只写短期运行态
- `evolution/` 只写长期优化结果
- `skills/` 只放稳定定义与正式晋升内容
- `artifacts/` 存放重型产物
- `control/` 存放治理与配额策略

这条边界必须长期保持稳定。
