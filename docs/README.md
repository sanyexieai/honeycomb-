# Honeycomb Design Docs

本文档集记录 Honeycomb 第一版核心设计。

Honeycomb 的目标不是单一 agent，而是一个由多个最小单元协作组成的系统：

- `Skill` 是静态定义载体，可以是单个 Markdown 或目录
- `Hive` 是 Skill 的运行时实例，是系统中的最小执行单元
- `System` 是多个 Hive 的协作网络

核心原则：

- 能力契约固定
- 具体实现可进化
- 最佳实践按上下文竞争
- 每个任务维护自己的运行时
- 进化过程可评估、可追踪、可回滚
- 多 Hive 协作通过统一协议完成

文档结构：

- `architecture.md`：总体架构与概念模型
- `specs/hive-spec.md`：固定能力契约规范
- `specs/implementation-spec.md`：实现体与基因规范
- `specs/practice-profile.md`：跨任务最佳实践规范
- `specs/process-executor.md`：主控与外部脚本/二进制执行协议
- `specs/task-scheduler.md`：任务模型与并行调度规范
- `specs/task-runtime.md`：任务运行时与会话模型
- `specs/runtime-and-traits.md`：Rust 核心结构体与 trait 草案

建议实现顺序：

1. `hive-core`
2. `hive-spec`
3. `hive-practice`
4. `hive-store`
5. `hive-runtime`
6. `hive-scheduler`
7. `hive-executors`
8. `hive-orchestrator`
9. `hive-evolution`
