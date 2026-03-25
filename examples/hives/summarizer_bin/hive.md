---
id: summarizer_bin
name: Summarizer Binary Hive
version: 0.1.0
kind: hive
capability: summarize_text

interface:
  input_schema: interface/input.schema.json
  output_schema: interface/output.schema.json

execution:
  executor: process
  entrypoint: summarize
  timeout_ms: 30000
  max_steps: 6

state:
  persistent: true
  slots:
    - current_task
    - last_result
    - retry_count

dependencies:
  required_capabilities: []
  optional_capabilities: []

tools:
  allowed:
    - run_binary

evaluation:
  metrics:
    - accuracy
    - brevity
    - format_compliance
  fitness_formula: "0.5*accuracy + 0.3*brevity + 0.2*format_compliance"

evolution:
  implementation_mutable: true
  interface_mutable: false
  split_allowed: true
  recommendable: true
---

# Purpose
负责把输入文本压缩为结构化摘要，由目录内独立二进制执行。

# Rules
- 不得编造输入中不存在的信息
- 输出必须符合 output schema
