# 执行面与进化面数据样例

## 1. 目标

本规范通过一个完整例子说明：

- 执行面写什么
- 进化面读什么
- 两者如何通过文件和记录衔接

## 2. 示例目录

```text
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

## 3. 执行面关键文件

建议至少有：

- `task.json`
- `queen/node.json`
- `workers/<worker_id>/node.json`
- `assignments/<assignment_id>.json`
- `outputs/result.json`
- `events.jsonl`
- `trace.jsonl`
- `audit.jsonl`

## 4. 进化面关键文件

建议至少有：

- `evaluations/<evaluation_id>.json`
- `fitness/<hive_impl>.json`
- `promotions/<promotion_id>.json`
- `lineages/<hive_impl>.json`

## 5. 关键边界

通过样例应体现：

- 执行面只写 `runtime/`
- 进化面只写 `evolution/`
- 晋升记录要能回指运行证据
- 租户、命名空间、任务和实现体标识要贯通两边
