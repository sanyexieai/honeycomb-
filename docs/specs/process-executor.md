# Process Executor Spec

`ProcessExecutor` 用于让主控二进制按需启动外部脚本或独立二进制，并通过统一协议完成执行。

## 1. 设计目标

目标是让以下能力保持解耦：

- 主控运行时尽量稳定
- Hive 实现体持续演化
- 脚本与二进制可以统一接入
- 并行调度和状态控制仍由主控负责

## 2. 执行模式

主控读取 `implementation.json` 后，根据实现配置决定：

- 启动脚本
- 启动目录中的独立二进制

推荐第一版使用短生命周期 worker：

- 主控拉起进程
- 通过 stdin 发送 JSON
- 从 stdout 读取 JSON 结果
- 进程退出

## 3. 输入协议

建议 stdin 输入为：

```json
{
  "task_id": "task_123",
  "session_id": "sess_001",
  "hive_id": "summarizer",
  "impl_id": "impl_v1",
  "input": {
    "source_text": "..."
  },
  "context": {
    "task_type": "long_document"
  },
  "overrides": {
    "temperature": 0.1
  }
}
```

## 4. 输出协议

建议 stdout 输出为：

```json
{
  "success": true,
  "payload": {
    "summary": "..."
  },
  "metrics": [
    { "name": "latency_ms", "value": 120.0 }
  ],
  "artifacts": []
}
```

## 5. 错误处理

建议统一处理以下错误：

- 进程启动失败
- 非零退出码
- stdout 非法 JSON
- 超时
- 输出缺少必填字段

这些错误都应被主控转换成标准 `HiveOutput` 失败记录或运行时错误。

## 6. 并发控制

建议由主控统一控制：

- 任务级并发上限
- capability 级并发上限
- 某个外部命令的并发上限

外部进程只负责执行，不负责抢占式调度。
