# 架构审查与护栏机制

## 1. 目标

本文档定义 Honeycomb 为避免实现持续偏离最初设计边界而引入的审查与护栏机制。

目标不是增加繁琐审批，而是把高风险偏移尽早暴露，并在进入代码、存储和长期状态之前进行边界检查。

## 2. 为什么需要这一层

Honeycomb 当前设计强调：

- 执行面与进化面分离
- 运行态与长期态分离
- 任务事实与长期结论分离
- 数据边界先于代码边界

如果缺少专门的审查机制，系统在持续扩展时容易出现：

- 执行面逐步接管长期写操作
- 运行态被后验治理结果回写
- 新命令跨越多个领域边界
- 单个入口文件持续吸收不属于自己的职责
- 文档边界与代码边界逐步脱节

因此需要把“边界是否被突破”变成一个正式检查项，而不是依赖开发者临时记忆。

当前最近一轮代码回顾结论是：

- 未发现执行面重新接管长期状态写入
- 未发现新的历史运行事实回写路径
- 新增的 global governance defaults、review/reflection 建议链路仍位于进化面

这说明当前护栏机制已经开始发挥作用，但仍需继续跟踪运行态对象化与 `control` 上提这两项遗留收敛任务。

## 3. 核心原则

### 3.1 审查重点是边界，不是所有代码

该机制主要审查高风险结构变化，不对所有小改动施加同等流程。

### 3.2 写操作比读操作更需要审查

特别是以下写入：

- 长期状态写入
- 历史运行事实修改
- 跨模块写入
- 权限与审批状态写入

### 3.3 先判断对象属性，再判断实现方式

每个新增能力应先回答：

1. 它属于执行面还是进化面
2. 它读写的是运行态还是长期态
3. 它记录的是事实还是结论
4. 它是否跨越租户、命名空间或治理边界

### 3.4 审查结果必须可追踪

高风险变更的审查结论应形成结构化记录，便于回溯和后续治理。

## 4. 审查分层

建议分成三层：

### 4.1 轻量架构检查

适用于日常开发阶段，在设计和实现前快速判断边界是否合理。

最小检查项：

- 所属平面
- 写入对象
- 状态类型
- 事实/结论属性
- 是否需要文档更新

### 4.2 正式架构审查

适用于会影响边界稳定性的变更，需要形成审查记录。

典型场景：

- 新增长期状态写操作
- 修改注册表或默认实现体
- 修改历史运行记录
- 引入新审批流
- 引入跨模块协调写逻辑
- 新增高权限命令

### 4.3 定期架构反思

适用于持续演化阶段，不针对单个改动，而是定期回看系统整体是否已经偏离原始边界。

这层主要解决“每次单看都合理，但累计下来整体跑偏”的问题。

建议至少覆盖：

- 执行面是否正在吸收长期治理职责
- 运行态是否出现越来越多后验回写
- 超大入口文件是否持续膨胀
- 文档声明的模块边界是否仍与代码一致
- 暂时性兼容逻辑是否已经变成长期默认逻辑

## 5. 必须触发审查的场景

建议以下场景默认强制触发审查：

### 5.1 平面边界风险

- 执行面新增长期状态写操作
- 进化面新增对运行中任务的直接干预
- 一个命令同时承担执行与治理职责

### 5.2 状态边界风险

- 修改 `runtime/` 中既有事实记录
- 修改 `skills/`、`evolution/`、`control/` 中正式对象
- 新增从运行态回写长期态的隐式路径

### 5.3 事实与结论混淆风险

- 用推荐值覆盖历史事实
- 用治理结论改写任务执行记录
- 将运行结果直接写成默认实现或正式状态

### 5.4 权限与控制风险

- 新增审批通过/拒绝写路径
- 新增可放宽边界的控制命令
- 新增跨租户、跨命名空间或跨作用域发布路径

### 5.5 模块耦合风险

- 单个模块开始依赖多个本应独立的领域
- CLI 入口继续吸收非入口职责
- 协议层、调度层、治理层开始互相直接改写状态

## 6. 审查问题清单

每个高风险变更建议至少回答以下问题：

1. 这个能力属于执行面还是进化面？
2. 它读写的是运行态还是长期态？
3. 它记录的是事实、建议，还是正式结论？
4. 是否会修改已有历史记录？
5. 是否引入新的长期默认值或正式状态写入？
6. 是否绕过了现有审批、治理或审计路径？
7. 是否需要同时更新文档、测试和审计模型？

如果前三问无法明确回答，应默认暂停实现。

## 7. 审查结果类型

建议至少支持：

- `pass`：边界清晰，可直接实现
- `pass_with_followup`：可实现，但需补文档或后续拆分
- `needs_redesign`：方向可行，但当前实现方式越界
- `blocked`：明显违反当前架构边界

## 8. 审查记录对象

建议引入 `ArchitectureReviewRecord`。

最小字段建议：

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

## 9. 审查记录最小示例

```json
{
  "review_id": "arch-review-2026-03-28-001",
  "title": "task backfill implementation behavior",
  "change_scope": "execution_cli_command",
  "requested_by": "local-dev",
  "target_plane": "execution",
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

## 10. 与现有治理和审批的关系

该机制不是替代人工审批，而是比审批更前置的一层架构过滤。

建议关系如下：

- 架构审查：判断“这个能力是否越界”
- 人工审批：判断“这个动作是否允许执行”
- 进化治理：判断“长期状态应如何更新”

一句话：

- 架构审查管边界
- 审批流程管权限
- 治理流程管长期结论
- 定期反思管长期漂移

## 10.1 护栏策略优先级

当前 guardrail 热点 refresh 与严重性判定已经支持分层策略来源。

优先级如下：

1. implementation `constraints`
2. skill `governance_policy`
3. global governance defaults
4. 系统默认策略

这样做的目标是：

- 让同一 skill 下的实现体默认遵循一致的治理节奏
- 让整个系统能定义一份跨 skill 的基础治理底线
- 让少数高风险或高价值实现体可以单独覆盖策略
- 避免 refresh review 判定完全写死在代码里

当前这层全局默认策略已经可通过 `honeycomb-evolution governance-defaults inspect` 直接查看。
同时也已可通过 `honeycomb-evolution governance-defaults set` 做最小增量治理调整。
此外，`honeycomb-evolution registry overview --with-details` 也已会带出当前全局治理默认策略摘要，便于在总览里观察系统底线。
同一总览中的 implementation hotspot 现在也会标明每个 refresh / 严重性参数来自哪一层策略来源，便于排查“为什么这次命中了这套护栏”。

## 11. 定期反思机制

### 11.1 目标

定期反思用于回答：

- 过去一段时间内系统是否在持续偏离最初设计
- 哪些偏移已经从“例外”演变成“常态”
- 哪些临时方案应该被删除、迁移或正式化

### 11.2 建议周期

建议至少两种节奏：

- 小周期：每周或每两周一次，快速检查新增命令、写路径和模块膨胀情况
- 大周期：每个阶段或里程碑结束后一次，回看是否需要调整路线图和边界文档

### 11.3 反思输入

建议定期汇总以下输入：

- 新增的 `ArchitectureReviewRecord`
- 新增 CLI 命令和持久化写路径
- `convergence-review-and-roadmap.md` 中未关闭的收敛项
- `evolution/audit/` 中与治理、审批、注册表更新相关的变更
- 超大文件、跨模块依赖和临时兼容逻辑清单
- implementation hotspot 的 guardrail 趋势、refresh 候选与 skill 级治理策略命中情况

### 11.4 反思问题清单

每次定期反思建议至少回答：

1. 本周期新增了哪些高风险写操作？
2. 执行面是否承担了更多长期治理职责？
3. 是否出现了新的历史事实回写路径？
4. 哪些模块正在持续膨胀？
5. 哪些“临时兼容”已经存在过久？
6. 哪些文档已经落后于代码？
7. 当前路线图是否需要调整优先级？

### 11.5 反思输出

建议每次定期反思至少输出：

- `drift_detected` 或 `no_major_drift`
- 本周期主要偏移项
- 需要冻结的高风险行为
- 下一个周期必须完成的收敛动作
- 是否需要新增正式架构审查

### 11.6 记录对象

当前实现已引入最小 `ArchitectureReflectionRecord`。

第一版也可以先使用文档化方式，把结果写入：

- `docs/specs/convergence-review-and-roadmap.md`
- 或单独的阶段性回顾文档

同时也支持结构化记录：

- `honeycomb-evolution reflection record`
- `honeycomb-evolution reflection inspect`
- `honeycomb-evolution reflection list`
- reflection 读取侧应优先聚合近期 `guardrail_blocked` audit，作为周期反思的默认输入
- 新生成的 reflection record 应固化一份 guardrail snapshot，避免后续读取时因时间推进而改变观察结果
- guardrail snapshot 应至少提供 `action / target_type / target_id / skill / reason` 五个分面，支持定位高频漂移来源
- review record 也应固化一份 guardrail snapshot，作为近期风险热点的自动 evidence
- implementation 读取视图也应能直接暴露近期 guardrail 命中、推荐状态与活跃任务上下文，避免风险只存在于审查记录而不进入日常观察面
- review/reflection 记录入口应默认把近期 implementation hotspot 转换成自动 follow-up、drift、freeze action 和 evidence refs，减少人工遗漏
- 系统还应提供只读的 `review suggest` 入口，把 implementation hotspot 直接转成结构化审查候选，便于人工确认后再正式落盘
- 系统还应提供 `review materialize`，允许将当前建议候选批量落成正式 `ArchitectureReviewRecord`，形成半自动审查流
- `review suggest/materialize` 应避免对同一 hotspot 重复生成同一 `review_id`，已存在的 review 应在建议视图中显式标记，并在 materialize 时自动跳过
- 当同一 implementation hotspot 相比既有 review 明显恶化时，`review suggest/materialize` 应生成新的 refresh review，而不是永远停留在“已存在”状态
- “明显恶化”应至少由阈值控制，例如 guardrail block 次数翻倍或绝对增量达到约定门槛，避免轻微波动频繁触发 refresh review
- 恶化判定不应只依赖 block 次数，也应结合 `recommended_by`、`active_tasks`、高风险 flags 等严重性信号，避免遗漏低频但高压的热点
- 上述 refresh 阈值与严重性权重应允许从实现体或治理配置读取，而不是永久硬编码在审查逻辑里

### 11.7 与实时审查的关系

实时审查解决：

- 这次改动会不会越界

定期反思解决：

- 系统整体是不是已经开始偏移

二者必须同时存在，才能避免：

- 单次变更看起来合理
- 多次累积后整体偏离初衷

## 12. 落地建议

### 12.1 第一版先文档化执行

第一版可以先不做复杂系统内引擎，先把以下动作固定下来：

- 新增高风险命令前必须补一条审查记录
- 新增长期写入前必须更新相关 spec
- 代码 review 时必须回答第 6 节问题清单
- 每个迭代或里程碑结束后必须做一次定期反思

### 12.2 第二版引入结构化记录

后续可将架构审查记录落到：

- `control/architecture-reviews/`
- 或 `evolution/reviews/`

用于沉淀边界决策历史。

同时可增加定期反思记录，落到：

- `evolution/reflections/`
- 或 `control/architecture-reflections/`

### 12.3 第三版引入自动护栏

后续可增加自动检查，例如：

- 检测执行面是否新增长期写入函数
- 检测是否新增对 `runtime/` 历史记录的覆盖性修改
- 检测 CLI 新命令是否跨越多个高风险领域
- 检测新增持久化路径是否未在文档中声明
- 检测长时间未关闭的收敛项
- 检测高风险审查记录是否持续重复出现

## 13. 建议优先接入的对象

最应该优先纳入审查机制的对象：

- CLI 新命令
- 注册表写操作
- 审批状态写操作
- 任务回填/迁移/修复类命令
- 进化面发布动作
- control 层策略变更

最应该优先纳入定期反思的对象：

- 超大入口文件
- 执行面对长期状态的写操作集合
- 历史事实修复、回填、迁移类命令
- 注册表同步与审批流
- 长期未关闭的兼容层和过渡逻辑

## 14. 第一版建议

第一版最少做到：

- 有正式审查文档
- 有统一问题清单
- 高风险改动必须留审查记录
- 审查结果进入后续收敛文档或治理记录
- 定期反思结果进入收敛路线文档

第一版不必承诺：

- 完整自动化审查引擎
- 所有改动强制审批
- 复杂图形化审查面板
- 全自动长期漂移诊断

## 15. 总结

Honeycomb 需要的不只是“能审批动作”的机制，还需要“能在动作落地前检查架构边界”的机制。

这层护栏的价值在于：

- 让偏移更早被发现
- 让边界决策可追踪
- 让系统在持续扩展时仍然保持最初设计初衷
- 让系统能周期性回看自己是否已经整体漂移
