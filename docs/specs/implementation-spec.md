# Implementation And Genome Spec

`implementation.json` 描述 Hive 当前的具体实现体。  
`genome.json` 描述 Implementation 的可变异空间。

这两个文件共同支撑：

- 实现替换
- 分裂
- 评估
- 进化

## 1. `implementation.json`

### 最小示例

```json
{
  "impl_id": "impl_v2",
  "hive_id": "summarizer",
  "version": "2.0.0",
  "status": "active",
  "executor": "composite",
  "entrypoint": "summarize",
  "components": {
    "prompt": "prompts/system.md",
    "config": "config/runtime.json",
    "script": "scripts/summarize.py",
    "binary": "bin/ranker.exe"
  },
  "strategy": {
    "mode": "extract_then_compress",
    "llm_model": "gpt-4.1",
    "temperature": 0.2,
    "tool_order": ["retrieve_context", "run_script", "summarize", "rank_relevance"]
  },
  "compatibility": {
    "capability": "summarize_text",
    "input_schema_version": "1.0.0",
    "output_schema_version": "1.0.0"
  },
  "constraints": {
    "max_cost": 0.02,
    "max_latency_ms": 5000
  },
  "origin": {
    "source": "mutation",
    "parent_impl": "impl_v1",
    "created_at": "2026-03-25T00:00:00Z"
  }
}
```

### 字段说明

- `impl_id`
  - Implementation 唯一标识
- `hive_id`
  - 所属 Hive
- `version`
  - 实现体版本
- `status`
  - 建议值：
    - `active`
    - `candidate`
    - `deprecated`
    - `failed`

### `components`

记录实现体由哪些资源构成：

- `prompt`
- `config`
- `script`
- `binary`
- `assets`

### `strategy`

描述运行策略：

- 模式
- 使用的模型
- 温度等超参数
- 工具调用顺序
- 其他扩展参数

### `compatibility`

用于验证该实现体是否还能被视为当前 Capability 的兼容实现：

- `capability`
- `input_schema_version`
- `output_schema_version`

### `constraints`

运行限制，例如：

- 最大成本
- 最大延迟

### `origin`

记录实现来源：

- 手工创建
- mutation
- recombination
- imported

## 2. `genome.json`

Genome 不是当前配置，而是变异规则。

### 最小示例

```json
{
  "hive_id": "summarizer",
  "impl_id": "impl_v2",
  "mutable_genes": {
    "prompt_template": {
      "gene_type": "Enum",
      "options": ["compact", "strict_json", "analytical"]
    },
    "temperature": {
      "gene_type": "Float",
      "min": 0.0,
      "max": 0.8,
      "step": 0.1
    },
    "tool_order": {
      "gene_type": "Sequence",
      "options": ["retrieve_context", "run_script", "rank_relevance"]
    },
    "script_variant": {
      "gene_type": "Enum",
      "options": ["summarize_v1.py", "summarize_v2.py", "summarize_v3.py"]
    },
    "reranker_enabled": {
      "gene_type": "Bool"
    }
  },
  "immutable_fields": [
    "capability",
    "input_schema_version",
    "output_schema_version",
    "entrypoint"
  ],
  "mutation_policy": {
    "max_mutations_per_generation": 3,
    "allow_component_swap": true,
    "allow_prompt_rewrite": true,
    "allow_freeform_code_edit": false
  }
}
```

### 字段说明

- `mutable_genes`
  - 定义允许变异的字段及取值范围
- `immutable_fields`
  - 明确哪些内容不可变
- `mutation_policy`
  - 控制变异力度和边界

## 3. 分裂模型

`split` 的语义建议定义为：

- 复制当前实现体
- 在 Genome 允许范围内产生一个或多个候选变体
- 保持同一 Capability 不变

例如：

- `impl_v2 -> impl_v2_a`
- `impl_v2 -> impl_v2_b`

分裂产生的是不同实现分支，而不是新能力。

## 4. 推荐模型

推荐建议作为显式记录存在，而不是隐式关系。

示例：

```json
{
  "from_hive": "planner",
  "to_hive": "summarizer",
  "recommended_impl": "impl_v2_b",
  "reason": "high downstream_acceptance on long documents",
  "score": 0.87,
  "timestamp": "2026-03-25T00:00:00Z"
}
```

最终是否采用，由路由器或 orchestrator 决定。

## 5. 第一版约束建议

建议第一版仅允许：

- 参数微调
- Prompt 模板切换
- 工具顺序变化
- 脚本/二进制组件切换
- 受约束 split

不建议第一版允许：

- 自动自由改写任意代码
- 自动变更 Capability
- 自动变更输入输出 schema

## 6. 评估优先于进化

只有先定义清楚评估机制，进化才有意义。

建议每次执行都记录：

- 产出结果
- 指标值
- 综合得分
- 是否被下游接受
- 执行成本和延迟

然后再基于评估决定：

- 是否保留候选实现
- 是否切换 active implementation
- 是否淘汰分支
