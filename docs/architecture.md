# Honeycomb Architecture

## 1. 定位

Honeycomb 是一个以 Markdown 或 Skill 目录为声明载体、以 `Hive` 为最小有状态执行单元、以 `Orchestrator` 连接多单元协作的 Rust 运行时框架。

它不是传统意义上的单 Agent 框架，而是一个可组合、可演化、可编排的蜂巢系统。

运行形态上，建议将 Honeycomb 设计为：

- 一个稳定的主控二进制
- 一组外部 Hive 目录
- 若干由主控按需拉起的脚本或子进程执行单元

## 2. 核心概念

### Skill

Skill 是静态定义，可由以下形式存在：

- 单个 `.md` 文件
- 一个包含多个文件的目录

Skill 用来描述：

- 能力目标
- 输入输出协议
- 约束规则
- 可用工具
- 协作依赖
- 评估指标
- 进化策略

### Hive

Hive 是 Skill 的运行时实例，是整个系统中的最小单位。每个 Hive 都应该具备：

- 唯一 ID
- 固定能力契约
- 当前实现池
- 生命周期状态
- 内部长期状态与记忆
- 输入输出接口
- 可观测日志与产物

### Capability

Capability 是 Hive 对外承诺的稳定能力。例如：

- `summarize_text`
- `retrieve_context`
- `rank_relevance`

Capability 应尽量稳定。它定义：

- 输入协议
- 输出协议
- 行为约束
- 评估标准

### Implementation

Implementation 是 Hive 的具体实现体。它可以由以下部件构成：

- Prompt
- 配置文件
- 脚本
- 二进制程序
- 执行策略参数

Implementation 允许演化，但必须保持与 Capability 的兼容性。

### Practice Profile

Practice Profile 是跨任务沉淀出来的“最佳实践”层。它描述：

- 在什么上下文下
- 推荐使用哪个实现
- 推荐使用哪些运行参数和协作方式
- 这些推荐基于多少历史任务得到

Practice Profile 的最佳性是上下文相关的，而不是全局唯一的。

### Task Runtime Session

Task Runtime Session 是某个具体任务中的临时运行时实例。它描述：

- 当前任务输入
- 当前上下文
- 本次选中的实现
- 本次匹配到的最佳实践
- 本次临时参数覆盖
- 本次局部状态和产物

Task Runtime Session 不应直接污染 Hive 的长期状态。

### Task Runtime

Task Runtime 是任务级运行态。它负责：

- 维护一个任务下的多个 Hive session
- 保存任务级共享上下文
- 记录任务当前状态、产物和依赖推进情况

### Process Worker

Process Worker 是主控运行时按需拉起的外部执行单元。它可以是：

- 脚本
- 目录内独立二进制

主控通过统一的 stdin/stdout JSON 协议与其交互。

### Scheduler

Scheduler 是主控二进制中的调度核心。它负责：

- 任务级并发
- 任务内 session 级并发
- 依赖满足后的 ready queue 推进
- capability 级别并发限制
- 执行失败、重试、超时和取消

### Genome

Genome 定义可变异空间，而不是实现本身。它明确说明：

- 哪些字段允许变化
- 每个字段的变异范围
- 是否允许组件替换
- 是否允许 Prompt 重写
- 是否允许自由代码修改

### Lineage

Lineage 用于追踪血缘关系，例如：

- 父实现是谁
- 何时分裂
- 由何种变异产生
- 哪些候选被淘汰

### Fitness

Fitness 是对 Implementation 或 Practice 效果的评估结果，通常综合以下维度：

- 正确率
- 成本
- 延迟
- 格式合规
- 被下游采纳率

### Orchestrator

Orchestrator 负责多 Hive 协作：

- 路由请求到合适 Hive
- 根据依赖关系调度执行
- 收集结果与状态
- 处理推荐与选择

### Evolution Manager

Evolution Manager 负责进化相关逻辑：

- 生成候选实现
- 分裂相似 Hive
- 评估候选效果
- 选择活跃实现
- 更新最佳实践

## 3. 核心原则

### 3.1 能力固定，实现可进化

Honeycomb 的根原则是：

- `Capability` 固定
- `Implementation` 可变

这意味着：

- 输入输出协议不应被任意变更
- 其他 Hive 可以稳定地调用该 Hive
- 实现体可以不断试错、替换、分裂

### 3.2 进化的是实现，不是协议

允许变更：

- Prompt 模板
- 工具顺序
- 超参数
- 脚本变体
- 二进制组件选择

不建议在第一版允许：

- 自动修改输入输出 schema
- 自动修改 Capability 定义
- 无约束自由改写脚本源码

### 3.3 最佳实践是上下文相关的

同一个 Hive 在不同场景下可能有不同最优实现。例如：

- 长文总结
- 短文本压缩
- 法律文本摘要
- 会议纪要整理

因此系统不应只有一个全局 `active implementation` 概念，还应维护：

- 面向任务类型的 Practice Profile
- 面向编排结构的 Practice Profile
- 面向预算和时延等级的 Practice Profile

### 3.4 任务运行时必须独立

同一个 Hive 可能同时参与多个任务，因此：

- Hive 长期状态必须和任务态隔离
- 每个任务都需要自己的本地运行时 session
- 当前任务中的局部调整不应直接覆盖长期默认值

### 3.5 协作优先于自治

第一版重点不是让 Hive 完全自治，而是确保：

- 每个 Hive 都有清晰接口
- 多 Hive 可以稳定协作
- 演化有明确评估机制

### 3.6 主控稳定，执行体可变

建议将稳定性要求放在主控二进制上，而不是放在每个 Hive 的实现体上：

- 主控二进制尽量少变
- Hive 目录可以持续演化
- 具体执行可以是脚本，也可以是独立二进制
- 调度、状态、评估、实践匹配仍由主控统一负责

## 4. Honeycomb 的层次

推荐按 6 层理解系统：

1. `Spec Layer`
   - Markdown、JSON、Schema
   - 定义 Hive 是什么
2. `Implementation Layer`
   - Prompt、脚本、二进制、参数
   - 定义 Hive 现在怎么做
3. `Practice Layer`
   - 面向上下文的最佳实践
   - 定义新任务默认应该怎么用
4. `Task Layer`
   - TaskSpec、TaskRuntime、TaskHiveSession
   - 定义某个具体任务怎么实例化和隔离
5. `Runtime Layer`
   - 状态机、执行器、持久化
   - 管理任务中某次 Hive 如何运行
6. `Coordination Layer`
   - 路由、依赖、推荐、演化
   - 管理多个 Hive 如何协作和优化

## 5. 推荐目录形态

```text
hives/
  summarizer/
    hive.md
    implementation.json
    genome.json
    practices/
      long_docs_v1.json
      short_text_fast_v1.json
    interface/
      input.schema.json
      output.schema.json
    prompts/
      system.md
    config/
      runtime.json
    scripts/
      summarize.py
    bin/
      ranker.exe
    evolution/
      lineage.json
      evaluations.jsonl
      candidates/
    state/
      runtime.json
      memory.json
```

说明：

- `hive.md`：固定能力契约
- `implementation.json`：当前默认实现体入口
- `genome.json`：允许如何变异
- `practices/`：不同上下文下的最佳实践
- `evolution/`：进化记录
- `state/`：长期运行状态与记忆

执行建议：

- `scripts/` 放轻量实现
- `bin/` 放独立可执行文件
- 由主控根据 `implementation.json` 统一拉起和调度

## 6. 生命周期建议

Hive 长期生命周期建议至少包含：

- `Loaded`
- `Ready`
- `Running`
- `WaitingInput`
- `WaitingDependency`
- `Suspended`
- `Completed`
- `Failed`

任务内 session 也可复用类似状态，但应与 Hive 长期状态隔离。

## 7. 协作模型建议

第一版建议采用：

- 同步调用为主
- 事件记录为辅
- 主控调度为主
- 外部 worker 按需拉起

即：

- Hive 对 Hive 的调用可以是明确的函数式调度
- 所有状态变化同时写入事件流或日志

这样调试成本更低，也更适合先把底座做稳。

## 8. 并行模型建议

建议明确区分 3 层并行：

1. `Task-Level Parallelism`
   - 多个任务同时执行
2. `Hive-Level Parallelism`
   - 同一个任务内部多个 Hive session 并行
3. `Implementation-Level Competition`
   - 同一能力的多个实现并行试跑，用于评估和选择

建议第一版默认支持前两层，并将第三层限制在评估/进化流程中使用。

## 9. 进程模型建议

建议采用混合模式：

- 一个稳定的主控二进制
- 每个 Hive 目录可包含脚本或独立二进制
- 主控读取实现配置后，按需启动外部进程执行

不建议第一版为每个 Hive 建立常驻服务进程，因为这会提前引入：

- 服务发现
- 端口管理
- 健康检查
- 长生命周期崩溃恢复

第一版更适合：

- 主控常驻
- worker 短生命周期
- 执行完毕即退出

## 10. 演化模型建议

第一版建议支持：

- 多实现并存
- 受约束 mutation
- split 分裂
- evaluation 评估
- active implementation 切换
- practice profile 更新

第一版不建议支持：

- 任意代码自修改
- 无约束自由生成新协议
- 完全自治的社会型协作网络

## 11. 第一版目标

V1 应重点实现：

1. 解析 Hive 目录与单文件 Skill
2. 创建 Hive 运行时实例
3. 维护 Hive 长期状态与任务运行时的隔离
4. 基于 Practice Profile 为新任务选择默认实现
5. 支持主控按需拉起脚本或二进制实现
6. 让一个 Hive 调另一个 Hive
7. 支持多任务和任务内 session 并行
8. 记录评估结果
9. 支持分裂、候选实现切换和最佳实践更新
