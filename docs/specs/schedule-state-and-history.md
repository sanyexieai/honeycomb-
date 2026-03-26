# 调度状态与触发历史

## 1. 目标

本规范定义触发器命中后的状态记录、历史保留和查看方式。

## 2. 为什么要单独设计

仅有触发器定义还不够，系统还必须回答：

- 这个触发器最近有没有命中
- 命中后是否真正执行了
- 为什么被跳过
- 最近一次失败是什么原因

这些信息会直接影响：

- 调试
- 巡检
- 提醒准确性
- 常驻蜂巢可用性

## 3. 建议对象

建议至少增加两类对象：

### 3.1 调度状态

用于描述某个触发器当前的运行状态。

### 3.2 触发历史记录

用于描述某次具体命中的结果。

## 4. 调度状态建议字段

建议至少包含：

- `trigger_id`
- `status`
- `last_triggered_at`
- `last_result`
- `last_error`
- `pending_count`
- `suppressed_count`
- `updated_at`

## 5. 触发历史建议字段

建议至少包含：

- `history_id`
- `trigger_id`
- `timestamp`
- `matched_by`
- `target_ref`
- `result`
- `reason`
- `task_id`
- `node_id`

## 6. 存储建议

建议增加：

```text
runtime/
  tenant/
    <tenant_id>/
      schedules/
        triggers/
        state/
        history/
```

## 7. 查看建议

建议未来支持：

- 查看某个触发器最近 10 次命中
- 查看某个触发器最近失败原因
- 查看某个常驻蜂巢的触发统计

## 8. 总结

调度状态和触发历史是让触发器系统真正可运维、可调试的关键补充层。
