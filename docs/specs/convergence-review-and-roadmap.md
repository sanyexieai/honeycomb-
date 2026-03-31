# 偏离回顾与收敛路线

## 1. 目标

本文档用于记录当前实现相对最初设计初衷的偏离点，并给出按阶段执行的收敛路线。

目标不是否定当前开发成果，而是在保留已有可运行骨架的前提下，把系统重新拉回最初设计强调的边界原则。

## 2. 本次回顾结论

当前实现没有偏离总体方向，但已经出现明显的实现层边界漂移。

仍然保持一致的部分：

- 三二进制分层目标已写入文档（见 `architecture.md`）；旧实现曾以双二进制为主
- bee 运行时、蜂巢能力中心与进化面的概念仍在推进落地
- 任务、assignment、事件、追踪、审计落盘骨架已经形成
- 触发器、常驻蜂巢、评估与治理已经有最小原型

已经开始偏离的部分：

- 旧 `honeycomb` 混入口曾直接写长期注册表和治理状态（目标终态应由能力中心 + 进化治理路径承担）
- 运行态开始被后验治理结果反向改写
- 超大 CLI 入口把多类职责耦合到同一个文件
- 实现体仍以字符串引用为主，尚未恢复为正式长期对象

## 3. 关键偏离点

### 3.1 混入口直接写长期状态

当前 `honeycomb` 曾承担部分技能、工具、审批与策略写入职责（与三层拆分目标冲突）。

这与以下原则冲突：

- bee 运行时不直接改长期技能定义
- 长期状态写入由进化面或经治理的能力中心路径负责
- 运行时不得直接改写稳定技能定义

### 3.2 历史运行事实被后验推荐回写

当前存在根据技能 `recommended_implementation_id` 回填旧任务和旧 assignment 的能力。

这会模糊以下边界：

- 任务记录应该描述当时实际运行事实
- 长期推荐不应回写为历史运行事实
- 审计记录应追加新结论，而不是篡改旧上下文

### 3.3 模块边界在实现层持续变松

当前执行 CLI 已同时承担：

- 任务提交与运行
- 调度与触发
- resident 生命周期
- 注册表读写
- 审批与告警
- 运行总览与系统总览

这会让后续继续偏离模块映射文档中的 `scheduler`、`control`、`observability`、`registry`、`trigger`、`resident` 等独立边界。

### 3.4 实现体抽象被压缩过度

当前治理对象主要还是 `implementation_ref` / `implementation_id` 字符串。

这与最初“能力契约稳定、实现体可演化、基因限制演化空间”的设计不完全一致，后续应恢复为正式实现体对象。

## 4. 收敛原则

- 先冻结高风险偏离，再做结构性拆分
- 先保护运行事实，再调整长期治理入口
- 先把写边界拉直，再补正式对象模型
- 可以保留现有 CLI 兼容层，但不能继续扩大职责范围
- 实时审查与定期反思同时存在

## 5. 分阶段路线

### 阶段 1：冻结历史运行事实回写

本阶段目标：

- 停止改写历史任务与 assignment 的 `implementation_ref`
- 将相关命令降级为只读建议模式
- 用追加记录或控制台建议代替回写

交付标准：

- 历史任务运行记录不再被后验推荐结果修改
- 用户仍可看到建议的推荐实现，但不会污染原始运行事实

### 阶段 2：收回混入口对长期状态的直接写入

本阶段目标：

- 将技能、工具、审批等长期写操作从 bee 运行链与混入口迁出
- bee 运行时保留运行期消费；注册表只读查询由能力中心（`honeycomb`）提供
- 长期状态写入收敛到进化面或经治理的能力中心入口

交付标准：

- `honeycomb` 作为能力中心不再承担未经治理的长期默认值、推荐值、审批状态写入

### 阶段 3：拆分混入口并迁移至 `honeycomb-bee`

本阶段目标：

- 将原执行 CLI 按职责拆成多个子模块，并把 queen/worker 主链迁移至 `honeycomb-bee`
- 为 `scheduler`、`control`、`observability`、`trigger`、`resident` 建立明确边界

交付标准：

- `src/app/execution.rs` 明显缩小
- 主要领域逻辑不再继续堆叠到单文件
- 领域目录至少覆盖 `task`、`overview`、`scheduler`、`protocol`、`trigger`、`resident`、`capability`

### 阶段 4：恢复正式实现体对象

本阶段目标：

- 引入最小 `ImplementationRecord`
- 让注册、评分、治理围绕正式实现体对象而不是纯字符串工作

交付标准：

- 注册表、进化面、运行时引用同一套实现体对象入口

### 阶段 5：建立定期反思节奏

本阶段目标：

- 将架构审查机制从“只拦单次高风险改动”扩展到“定期回看整体漂移”
- 固定周期性回顾输入、问题清单和输出格式
- 让收敛路线图持续根据反思结果更新

交付标准：

- 每个迭代或阶段结束后至少有一次正式反思结论
- 反思结果能明确指出新增偏移、遗留偏移和下阶段收敛动作

## 6. 当前执行顺序

当前按以下顺序推进：

1. 冻结 `task backfill-implementation` 的回写行为
2. 迁出混入口对长期状态的写操作
3. 拆分混入口并迁移 queen/worker 至 `honeycomb-bee`
4. 引入正式实现体对象
5. 建立定期反思记录与节奏

## 7. 状态

- 文档状态：已建立
- 当前回顾结论：未发现新的架构偏离，当前开发仍在沿既定收敛路线推进
  - 已确认：bee 运行链未重新接管长期状态写入
  - 已确认：进化面新增的治理默认策略、review/reflection 建议与 hotspot 汇总仍停留在长期治理侧
  - 已确认：未出现新的历史运行事实回写路径
  - 遗留：运行态仍主要使用 `implementation_ref` 字符串；`control` 仍是 bee 运行时代码布局内子模块
- 阶段 1：已完成
  - 已完成：`task backfill-implementation` 已冻结为只读建议模式，不再改写历史任务与 assignment 运行事实
- 阶段 2：已完成
  - 已完成：混入口长期写命令已从 CLI 移除
- 阶段 3：进行中
  - 已完成：`src/app/execution.rs` 已收敛为薄分发层
  - 已完成：bee 运行时代码布局内已形成 `task`、`overview`、`scheduler`、`protocol`、`trigger`、`resident`、`capability`、`control` 领域目录
  - 已完成：`capability` 已进一步拆分为 `skill`、`tool`、`approval`、`execution_record`
  - 已完成：shell policy、审批状态过滤、执行前授权检查已迁入 `control`
  - 遗留：`control` 仍是 bee 运行时代码布局内子模块，尚未提升为更独立的上层领域模块
- 阶段 4：进行中
  - 已完成：最小 `ImplementationRecord`、`ImplementationEntry`、`ImplementationCompatibility`、`ImplementationOrigin` 已落地
  - 已完成：`registry/implementations` 的 `persist/load/update/list` 存储接口已落地
  - 已完成：进化面已提供 `implementation inspect/list` 只读入口
  - 已完成：技能默认实现与推荐实现已接入对象存在性与技能归属校验
  - 已完成：`task submit/demo-flow`、`skill inspect/list/execute`、`registry sync/overview` 已接入实现体对象校验
  - 已完成：`task submit/demo-flow` 已开始把最小 `implementation_snapshot` 写入任务运行态记录
  - 已完成：`task assign` 已开始把最小 `implementation_snapshot` 写入 assignment 运行态记录
  - 已完成：`skill execute` / `tool execute` 已开始把最小 `implementation_snapshot` 写入 execution record 运行态记录
  - 已完成：`runtime overview` / `system overview` 的实现体聚合口径已开始优先消费运行态 `implementation_snapshot`
  - 已完成：进化面的 `implementation usage` / `implementation hotspot` / `print_runtime_usage` 已开始优先消费运行态 `implementation_snapshot`
  - 已完成：进化面的实现体使用统计已扩展到 `task / assignment / execution record` 三层运行态
  - 已完成：`registry overview --with-details` 的 `implementation_usage` 已从单一 `task_count` 升级为多维实现体使用摘要
  - 已完成：能力中心侧 `runtime overview` / `system overview` 已与进化面对齐到同一套多维实现体使用摘要
  - 已完成：`FitnessReport` 与 `EvolutionPlan` 已内嵌最小实现体快照，不再只围绕裸 `implementation_id`
  - 已完成：治理决策已开始直接消费 `strategy.mode`、`components.prompt`、`constraints.max_cost/max_latency_ms`
  - 已完成：`registry sync` 已对极端高风险实现体启用跳过与降权排序
  - 已完成：`governance plan/apply` 候选选择已与 `registry sync` 对齐到同一套风险护栏
  - 已完成：护栏命中结果已写入 evolution audit，可直接作为后续 review/reflection 输入
  - 已完成：`implementation inspect/list` 已可直接显示近期 guardrail 命中、推荐状态与活跃任务上下文
  - 已完成：`registry overview --with-details` 已新增 implementation hotspot 视图，用于定位“最近常触发 guardrail 但仍被推荐或活跃使用”的实现体
  - 遗留：更多运行态对象和更深层治理链路仍未全面摆脱 `implementation_ref` 字符串，治理也还未全面消费 `ImplementationRecord` 的更多深层字段
- 阶段 5：进行中
  - 已完成：最小 `ArchitectureReflectionRecord` 与 reflection CLI 已落地
  - 已完成：`reflection inspect/list` 已可直接显示近期 guardrail block 摘要
  - 已完成：guardrail snapshot 已写入 reflection record，不再只依赖读取时临时聚合
  - 已完成：guardrail snapshot 已支持 `action / target_type / target_id / skill / reason` 多分面摘要
  - 已完成：review record 已写入 guardrail snapshot，review inspect/list 可直接显示近期风险热点
  - 已完成：`review record` / `reflection record` 会自动吸收 implementation hotspot，补全 follow-up、drift、freeze action 和 evidence refs
  - 已完成：进化面已提供 `review suggest`，可直接列出基于 implementation hotspot 的结构化审查候选
  - 已完成：进化面已提供 `review materialize`，可将当前审查候选直接落成正式 review record
  - 已完成：`review suggest/materialize` 已具备同一 hotspot review_id 防重复能力，已有 review 会被显式标记并在 materialize 时跳过
  - 已完成：`review suggest/materialize` 已可识别“新热点 / 恶化热点 / 已有热点”，恶化热点会生成新的 refresh review 候选
  - 已完成：恶化热点已接入最小阈值策略，只有 guardrail 次数翻倍或绝对增量达到门槛时才生成 refresh review
  - 已完成：恶化判定已升级为多因子严重性模型，开始综合 `recommended_by`、`active_tasks`、高风险 flags 与 block 次数
  - 已完成：implementation hotspot 的排序、review/reflection follow-up 和 review suggestion rationale 已开始吸收 `runtime_assignment_count` 与 `execution_count`
  - 已完成：refresh review 的严重性模型与权重配置已正式吸收 `runtime_assignment_count` 与 `execution_count`
  - 已完成：refresh 阈值与严重性权重已支持从实现体 `constraints` 读取，不再完全硬编码
  - 已完成：技能对象已支持 `governance_policy`，同一 skill 下的实现体可共享 refresh 阈值与严重性权重默认值
  - 已完成：全局 `GovernanceDefaultsRecord` 已落地，可承载跨 skill 的基础治理默认策略
  - 已完成：review refresh 策略已形成 `implementation constraints -> skill governance_policy -> global governance defaults -> system default` 的分层优先级
  - 已完成：进化面已提供 `governance-defaults inspect`，全局治理默认策略已具备最小只读入口
  - 已完成：进化面已提供 `governance-defaults set`，全局治理默认策略已具备最小增量写入口
  - 已完成：`governance-defaults inspect` 已可直接显示当前已知治理策略键，便于发现可配置的 refresh / 严重性参数
  - 已完成：`registry overview --with-details` 已可显示 global governance defaults 摘要与当前生效键
  - 已完成：implementation hotspot 已可显示 refresh / 严重性参数的命中来源层，覆盖 `implementation / skill / global / built_in`
  - 已完成：implementation hotspot 已可直接显示 `runtime_assignment` / `execution` 严重性权重及其来源层
