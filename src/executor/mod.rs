use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::core::{EXECUTION_SCHEMA_VERSION, current_timestamp};
use crate::registry::ImplementationRecord;
use crate::runtime::ImplementationSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionKind {
    Skill,
    Tool,
}

impl ExecutionKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Skill => "skill",
            Self::Tool => "tool",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Simulated,
    Succeeded,
    Failed,
    TimedOut,
}

impl ExecutionStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Simulated => "simulated",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::TimedOut => "timed_out",
        }
    }
}

const DEFAULT_SHELL_TIMEOUT_MS: u64 = 5_000;
const DEFAULT_MAX_OUTPUT_BYTES: usize = 4_096;
const DEFAULT_OPENAI_TIMEOUT_SECS: u64 = 60;
const GENERIC_LLM_PROVIDER_ENV: &str = "HONEYCOMB_LLM_PROVIDER";
const GENERIC_LLM_API_KEY_ENV: &str = "HONEYCOMB_LLM_API_KEY";
const GENERIC_LLM_BASE_URL_ENV: &str = "HONEYCOMB_LLM_BASE_URL";
const GENERIC_LLM_ENDPOINT_PATH_ENV: &str = "HONEYCOMB_LLM_ENDPOINT_PATH";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SkillProviderKind {
    OpenAIResponses,
    OpenAICompatibleResponses,
    OpenAICompatibleChat,
    MiniMaxChat,
    OllamaResponses,
    OllamaGenerate,
}

impl SkillProviderKind {
    fn runner(self) -> &'static str {
        match self {
            Self::OpenAIResponses => "openai-responses",
            Self::OpenAICompatibleResponses => "openai-compatible-responses",
            Self::OpenAICompatibleChat => "openai-compatible-chat",
            Self::MiniMaxChat => "minimax-chat",
            Self::OllamaResponses => "ollama-responses",
            Self::OllamaGenerate => "ollama-generate",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SkillProviderConfig {
    kind: SkillProviderKind,
    base_url: String,
    endpoint_path: String,
    api_key: Option<String>,
    model: String,
    timeout_secs: u64,
    reasoning_effort: Option<String>,
}

fn default_execution_status() -> ExecutionStatus {
    ExecutionStatus::Simulated
}

fn default_execution_runner() -> String {
    "local-simulated".to_owned()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionRecord {
    pub schema_version: String,
    pub execution_id: String,
    pub kind: ExecutionKind,
    pub target_id: String,
    pub task_id: Option<String>,
    pub assignment_id: Option<String>,
    pub implementation_ref: Option<String>,
    #[serde(default)]
    pub implementation_snapshot: Option<ImplementationSnapshot>,
    #[serde(default)]
    pub skill_refs: Vec<String>,
    #[serde(default)]
    pub tool_refs: Vec<String>,
    pub input: String,
    #[serde(default)]
    pub plan_steps: Vec<String>,
    pub output: String,
    #[serde(default = "default_execution_runner")]
    pub runner: String,
    #[serde(default)]
    pub exit_code: Option<i32>,
    #[serde(default = "default_execution_status")]
    pub status: ExecutionStatus,
    pub recorded_at: String,
}

impl ExecutionRecord {
    pub fn new(
        execution_id: String,
        kind: ExecutionKind,
        target_id: String,
        task_id: Option<String>,
        assignment_id: Option<String>,
        implementation_ref: Option<String>,
        implementation_snapshot: Option<ImplementationSnapshot>,
        skill_refs: Vec<String>,
        tool_refs: Vec<String>,
        input: String,
        plan_steps: Vec<String>,
        output: String,
        runner: String,
        exit_code: Option<i32>,
        status: ExecutionStatus,
    ) -> Self {
        Self {
            schema_version: EXECUTION_SCHEMA_VERSION.to_owned(),
            execution_id,
            kind,
            target_id,
            task_id,
            assignment_id,
            implementation_ref,
            implementation_snapshot,
            skill_refs,
            tool_refs,
            input,
            plan_steps,
            output,
            runner,
            exit_code,
            status,
            recorded_at: current_timestamp(),
        }
    }

    pub fn simulated(
        execution_id: String,
        kind: ExecutionKind,
        target_id: String,
        task_id: Option<String>,
        assignment_id: Option<String>,
        implementation_ref: Option<String>,
        implementation_snapshot: Option<ImplementationSnapshot>,
        skill_refs: Vec<String>,
        tool_refs: Vec<String>,
        input: String,
        plan_steps: Vec<String>,
        output: String,
    ) -> Self {
        Self::new(
            execution_id,
            kind,
            target_id,
            task_id,
            assignment_id,
            implementation_ref,
            implementation_snapshot,
            skill_refs,
            tool_refs,
            input,
            plan_steps,
            output,
            "local-simulated".to_owned(),
            None,
            ExecutionStatus::Simulated,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolExecutionOutcome {
    pub runner: String,
    pub exit_code: Option<i32>,
    pub status: ExecutionStatus,
    pub plan_steps: Vec<String>,
    pub output: String,
}

impl ToolExecutionOutcome {
    pub fn simulated(entrypoint: &str, input: &str) -> Self {
        Self {
            runner: "local-simulated".to_owned(),
            exit_code: None,
            status: ExecutionStatus::Simulated,
            plan_steps: vec![format!(
                "simulate_tool entrypoint={} input={}",
                entrypoint, input
            )],
            output: format!("simulated tool execution via {entrypoint}"),
        }
    }
}

pub fn execute_tool_entrypoint(entrypoint: &str, input: &str) -> io::Result<ToolExecutionOutcome> {
    if let Some(command) = entrypoint.strip_prefix("shell://") {
        return execute_local_shell(command, input);
    }

    Ok(ToolExecutionOutcome::simulated(entrypoint, input))
}

pub fn execute_skill_implementation(
    root: &str,
    implementation: &ImplementationRecord,
    input: &str,
    tool_outputs: &[String],
) -> io::Result<ToolExecutionOutcome> {
    load_local_env(root)?;
    if let Some(provider) = resolve_skill_provider(implementation)? {
        if provider.api_key.is_none() {
            match provider.kind {
                SkillProviderKind::OpenAIResponses
                | SkillProviderKind::OpenAICompatibleResponses
                | SkillProviderKind::OpenAICompatibleChat
                | SkillProviderKind::MiniMaxChat => {
                    return Err(io::Error::other(
                        "LLM provider requires an API key. Set HONEYCOMB_LLM_API_KEY \
                         (with HONEYCOMB_LLM_PROVIDER), or provider-specific key env / provider_api_key.",
                    ));
                }
                SkillProviderKind::OllamaResponses | SkillProviderKind::OllamaGenerate => {}
            }
        }
        return execute_provider_skill(root, implementation, input, tool_outputs, &provider);
    }

    Ok(ToolExecutionOutcome {
        runner: "local-simulated".to_owned(),
        exit_code: None,
        status: ExecutionStatus::Simulated,
        plan_steps: vec![format!(
            "simulate_skill executor={} entry={}{}",
            implementation.executor, implementation.entry.kind, implementation.entry.path
        )],
        output: format!(
            "simulated skill execution for {} via {}:{}",
            implementation.implementation_id, implementation.entry.kind, implementation.entry.path
        ),
    })
}

fn load_local_env(root: &str) -> io::Result<()> {
    let mut candidates = Vec::new();
    let root_path = Path::new(root);
    candidates.push(root_path.join(".env"));
    candidates.push(root_path.join(".env.local"));
    if let Ok(current_dir) = env::current_dir() {
        candidates.push(current_dir.join(".env"));
        candidates.push(current_dir.join(".env.local"));
    }
    for path in candidates {
        if path.exists() {
            load_env_file(&path)?;
        }
    }
    Ok(())
}

fn load_env_file(path: &Path) -> io::Result<()> {
    let body = fs::read_to_string(path)?;
    for raw_line in body.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, raw_value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() || env::var_os(key).is_some() {
            continue;
        }
        let value = parse_env_value(raw_value.trim());
        // Safe here because this CLI is single-process scoped and uses env loading only
        // as a local fallback before provider resolution. Existing env vars always win.
        unsafe {
            env::set_var(key, value);
        }
    }
    Ok(())
}

fn parse_env_value(raw: &str) -> String {
    if raw.len() >= 2 {
        let first = raw.as_bytes()[0];
        let last = raw.as_bytes()[raw.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return raw[1..raw.len() - 1].to_owned();
        }
    }
    raw.to_owned()
}

fn execute_provider_skill(
    root: &str,
    implementation: &ImplementationRecord,
    input: &str,
    tool_outputs: &[String],
    provider: &SkillProviderConfig,
) -> io::Result<ToolExecutionOutcome> {
    match provider.kind {
        SkillProviderKind::OpenAIResponses
        | SkillProviderKind::OpenAICompatibleResponses
        | SkillProviderKind::OllamaResponses => {
            execute_responses_provider(root, implementation, input, tool_outputs, provider)
        }
        SkillProviderKind::OpenAICompatibleChat | SkillProviderKind::MiniMaxChat => {
            execute_chat_completions_provider(root, implementation, input, tool_outputs, provider)
        }
        SkillProviderKind::OllamaGenerate => {
            execute_ollama_generate(root, implementation, input, tool_outputs, provider)
        }
    }
}

fn resolve_skill_provider(
    implementation: &ImplementationRecord,
) -> io::Result<Option<SkillProviderConfig>> {
    let reasoning_effort = implementation.strategy.get("reasoning_effort").cloned();
    let timeout_secs = implementation
        .strategy
        .get("provider_timeout_secs")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_OPENAI_TIMEOUT_SECS);
    let model = implementation.entry.path.clone();
    let custom_base_url = implementation.strategy.get("provider_base_url").cloned();
    let custom_endpoint_path = implementation.strategy.get("provider_endpoint_path").cloned();
    let custom_api_key = implementation
        .strategy
        .get("provider_api_key")
        .cloned()
        .or_else(|| {
            implementation
                .strategy
                .get("provider_api_key_env")
                .and_then(|key| env::var(key).ok())
        });

    let provider = match implementation.executor.as_str() {
        "openai_responses" => Some(SkillProviderConfig {
            kind: SkillProviderKind::OpenAIResponses,
            base_url: custom_base_url
                .or_else(|| generic_provider_env(&["openai_responses", "openai"], GENERIC_LLM_BASE_URL_ENV))
                .unwrap_or_else(|| "https://api.openai.com/v1".to_owned()),
            endpoint_path: custom_endpoint_path
                .or_else(|| generic_provider_env(&["openai_responses", "openai"], GENERIC_LLM_ENDPOINT_PATH_ENV))
                .unwrap_or_else(|| "/responses".to_owned()),
            api_key: custom_api_key
                .or_else(|| env::var("OPENAI_API_KEY").ok())
                .or_else(|| generic_provider_env(&["openai_responses", "openai"], GENERIC_LLM_API_KEY_ENV)),
            model,
            timeout_secs,
            reasoning_effort,
        }),
        "openai_compatible_responses" => Some(SkillProviderConfig {
            kind: SkillProviderKind::OpenAICompatibleResponses,
            base_url: custom_base_url
                .or_else(|| env::var("OPENAI_COMPATIBLE_BASE_URL").ok())
                .or_else(|| {
                    generic_provider_env(
                        &["openai_compatible_responses", "openai_compatible", "compatible"],
                        GENERIC_LLM_BASE_URL_ENV,
                    )
                })
                .unwrap_or_else(|| "http://localhost:11434/v1".to_owned()),
            endpoint_path: custom_endpoint_path
                .or_else(|| {
                    generic_provider_env(
                        &["openai_compatible_responses", "openai_compatible", "compatible"],
                        GENERIC_LLM_ENDPOINT_PATH_ENV,
                    )
                })
                .unwrap_or_else(|| "/responses".to_owned()),
            api_key: custom_api_key
                .or_else(|| env::var("OPENAI_COMPATIBLE_API_KEY").ok())
                .or_else(|| env::var("OPENAI_API_KEY").ok())
                .or_else(|| {
                    generic_provider_env(
                        &["openai_compatible_responses", "openai_compatible", "compatible"],
                        GENERIC_LLM_API_KEY_ENV,
                    )
                }),
            model,
            timeout_secs,
            reasoning_effort,
        }),
        "openai_compatible_chat" => Some(SkillProviderConfig {
            kind: SkillProviderKind::OpenAICompatibleChat,
            base_url: custom_base_url
                .or_else(|| env::var("OPENAI_COMPATIBLE_BASE_URL").ok())
                .or_else(|| {
                    generic_provider_env(
                        &["openai_compatible_chat", "openai_compatible", "compatible"],
                        GENERIC_LLM_BASE_URL_ENV,
                    )
                })
                .unwrap_or_else(|| "http://localhost:11434/v1".to_owned()),
            endpoint_path: custom_endpoint_path
                .or_else(|| {
                    generic_provider_env(
                        &["openai_compatible_chat", "openai_compatible", "compatible"],
                        GENERIC_LLM_ENDPOINT_PATH_ENV,
                    )
                })
                .unwrap_or_else(|| "/chat/completions".to_owned()),
            api_key: custom_api_key
                .or_else(|| env::var("OPENAI_COMPATIBLE_API_KEY").ok())
                .or_else(|| env::var("OPENAI_API_KEY").ok())
                .or_else(|| {
                    generic_provider_env(
                        &["openai_compatible_chat", "openai_compatible", "compatible"],
                        GENERIC_LLM_API_KEY_ENV,
                    )
                }),
            model,
            timeout_secs,
            reasoning_effort,
        }),
        "minimax_chat" => Some(SkillProviderConfig {
            kind: SkillProviderKind::MiniMaxChat,
            base_url: custom_base_url
                .or_else(|| env::var("MINIMAX_BASE_URL").ok())
                .or_else(|| {
                    generic_provider_env(&["minimax_chat", "minimax"], GENERIC_LLM_BASE_URL_ENV)
                })
                .unwrap_or_else(|| "https://api.minimaxi.com/v1".to_owned()),
            endpoint_path: custom_endpoint_path
                .or_else(|| {
                    generic_provider_env(
                        &["minimax_chat", "minimax"],
                        GENERIC_LLM_ENDPOINT_PATH_ENV,
                    )
                })
                .unwrap_or_else(|| "/chat/completions".to_owned()),
            api_key: custom_api_key
                .or_else(|| env::var("MINIMAX_API_KEY").ok())
                .or_else(|| env::var("OPENAI_API_KEY").ok())
                .or_else(|| {
                    generic_provider_env(&["minimax_chat", "minimax"], GENERIC_LLM_API_KEY_ENV)
                }),
            model,
            timeout_secs,
            reasoning_effort,
        }),
        "ollama_responses" => Some(SkillProviderConfig {
            kind: SkillProviderKind::OllamaResponses,
            base_url: custom_base_url
                .or_else(|| env::var("OLLAMA_BASE_URL").ok())
                .or_else(|| {
                    generic_provider_env(&["ollama_responses", "ollama"], GENERIC_LLM_BASE_URL_ENV)
                })
                .unwrap_or_else(|| "http://localhost:11434/v1".to_owned()),
            endpoint_path: custom_endpoint_path
                .or_else(|| {
                    generic_provider_env(
                        &["ollama_responses", "ollama"],
                        GENERIC_LLM_ENDPOINT_PATH_ENV,
                    )
                })
                .unwrap_or_else(|| "/responses".to_owned()),
            api_key: custom_api_key
                .or_else(|| env::var("OLLAMA_API_KEY").ok())
                .or_else(|| Some("ollama".to_owned())),
            model,
            timeout_secs,
            reasoning_effort,
        }),
        "ollama_generate" => Some(SkillProviderConfig {
            kind: SkillProviderKind::OllamaGenerate,
            base_url: custom_base_url
                .or_else(|| env::var("OLLAMA_BASE_URL").ok())
                .or_else(|| {
                    generic_provider_env(&["ollama_generate", "ollama"], GENERIC_LLM_BASE_URL_ENV)
                })
                .unwrap_or_else(|| "http://localhost:11434/api".to_owned()),
            endpoint_path: custom_endpoint_path
                .or_else(|| {
                    generic_provider_env(
                        &["ollama_generate", "ollama"],
                        GENERIC_LLM_ENDPOINT_PATH_ENV,
                    )
                })
                .unwrap_or_else(|| "/generate".to_owned()),
            api_key: custom_api_key
                .or_else(|| env::var("OLLAMA_API_KEY").ok())
                .or_else(|| {
                    generic_provider_env(&["ollama_generate", "ollama"], GENERIC_LLM_API_KEY_ENV)
                }),
            model,
            timeout_secs,
            reasoning_effort,
        }),
        _ => None,
    };

    Ok(provider)
}

fn generic_provider_env(aliases: &[&str], key: &str) -> Option<String> {
    let selected = env::var(GENERIC_LLM_PROVIDER_ENV).ok()?;
    let selected = selected.trim().to_ascii_lowercase();
    let matches = aliases
        .iter()
        .any(|alias| selected == alias.trim().to_ascii_lowercase());
    if matches { env::var(key).ok() } else { None }
}

fn execute_chat_completions_provider(
    root: &str,
    implementation: &ImplementationRecord,
    input: &str,
    tool_outputs: &[String],
    provider: &SkillProviderConfig,
) -> io::Result<ToolExecutionOutcome> {
    let client = Client::builder()
        .timeout(Duration::from_secs(provider.timeout_secs))
        .build()
        .map_err(io::Error::other)?;
    let prompt_text = load_prompt_component(root, implementation.components.get("prompt"))?;
    let input_text = merge_skill_input(input, tool_outputs);
    let payload = build_chat_completions_payload(
        provider.model.as_str(),
        prompt_text.as_deref(),
        &input_text,
        provider.reasoning_effort.as_deref(),
        matches!(provider.kind, SkillProviderKind::MiniMaxChat),
    );
    let endpoint = provider_endpoint(provider);
    let request = client
        .post(endpoint)
        .header("content-type", "application/json")
        .json(&payload);
    let request = if let Some(api_key) = provider.api_key.as_deref() {
        request.bearer_auth(api_key)
    } else {
        request
    };
    let response = request.send().map_err(io::Error::other)?;
    let status_code = response.status();
    let body = response.text().map_err(io::Error::other)?;

    if !status_code.is_success() {
        return Ok(ToolExecutionOutcome {
            runner: provider.kind.runner().to_owned(),
            exit_code: Some(status_code.as_u16() as i32),
            status: ExecutionStatus::Failed,
            plan_steps: vec![
                format!("provider={} model={}", provider.kind.runner(), provider.model),
                format!("provider_http_status={status_code}"),
            ],
            output: truncate_output(body, DEFAULT_MAX_OUTPUT_BYTES).0,
        });
    }

    let value: Value = serde_json::from_str(&body).map_err(io::Error::other)?;
    let output = extract_chat_completions_output_text(&value)
        .unwrap_or_else(|| truncate_output(body, DEFAULT_MAX_OUTPUT_BYTES).0);
    let mut plan_steps = vec![
        format!("provider={} model={}", provider.kind.runner(), provider.model),
        format!(
            "provider_prompt_loaded={}",
            if prompt_text.is_some() { "true" } else { "false" }
        ),
    ];
    if let Some(effort) = provider.reasoning_effort.as_deref() {
        plan_steps.push(format!("provider_reasoning_effort={effort}"));
    }
    if matches!(provider.kind, SkillProviderKind::MiniMaxChat) {
        plan_steps.push("provider_reasoning_split=true".to_owned());
    }
    if !tool_outputs.is_empty() {
        plan_steps.push(format!("tool_context_count={}", tool_outputs.len()));
    }

    Ok(ToolExecutionOutcome {
        runner: provider.kind.runner().to_owned(),
        exit_code: Some(status_code.as_u16() as i32),
        status: ExecutionStatus::Succeeded,
        plan_steps,
        output,
    })
}

fn execute_responses_provider(
    root: &str,
    implementation: &ImplementationRecord,
    input: &str,
    tool_outputs: &[String],
    provider: &SkillProviderConfig,
) -> io::Result<ToolExecutionOutcome> {
    let client = Client::builder()
        .timeout(Duration::from_secs(provider.timeout_secs))
        .build()
        .map_err(io::Error::other)?;
    let prompt_text = load_prompt_component(root, implementation.components.get("prompt"))?;
    let input_text = merge_skill_input(input, tool_outputs);
    let payload = build_openai_responses_payload(
        provider.model.as_str(),
        prompt_text.as_deref(),
        &input_text,
        provider.reasoning_effort.as_deref(),
    );
    let endpoint = provider_endpoint(provider);

    let request = client
        .post(endpoint)
        .header("content-type", "application/json")
        .json(&payload);
    let request = if let Some(api_key) = provider.api_key.as_deref() {
        request.bearer_auth(api_key)
    } else {
        request
    };
    let response = request.send().map_err(io::Error::other)?;
    let status_code = response.status();
    let body = response.text().map_err(io::Error::other)?;

    if !status_code.is_success() {
        return Ok(ToolExecutionOutcome {
            runner: provider.kind.runner().to_owned(),
            exit_code: Some(status_code.as_u16() as i32),
            status: ExecutionStatus::Failed,
            plan_steps: vec![
                format!("provider={} model={}", provider.kind.runner(), provider.model),
                format!("provider_http_status={status_code}"),
            ],
            output: truncate_output(body, DEFAULT_MAX_OUTPUT_BYTES).0,
        });
    }

    let value: Value = serde_json::from_str(&body).map_err(io::Error::other)?;
    let output = extract_openai_response_output_text(&value)
        .unwrap_or_else(|| truncate_output(body, DEFAULT_MAX_OUTPUT_BYTES).0);
    let mut plan_steps = vec![
        format!("provider={} model={}", provider.kind.runner(), provider.model),
        format!(
            "provider_prompt_loaded={}",
            if prompt_text.is_some() { "true" } else { "false" }
        ),
    ];
    if let Some(effort) = provider.reasoning_effort.as_deref() {
        plan_steps.push(format!("provider_reasoning_effort={effort}"));
    }
    if !tool_outputs.is_empty() {
        plan_steps.push(format!("tool_context_count={}", tool_outputs.len()));
    }

    Ok(ToolExecutionOutcome {
        runner: provider.kind.runner().to_owned(),
        exit_code: Some(status_code.as_u16() as i32),
        status: ExecutionStatus::Succeeded,
        plan_steps,
        output,
    })
}

fn execute_ollama_generate(
    root: &str,
    implementation: &ImplementationRecord,
    input: &str,
    tool_outputs: &[String],
    provider: &SkillProviderConfig,
) -> io::Result<ToolExecutionOutcome> {
    let client = Client::builder()
        .timeout(Duration::from_secs(provider.timeout_secs))
        .build()
        .map_err(io::Error::other)?;
    let prompt_text = load_prompt_component(root, implementation.components.get("prompt"))?;
    let merged_input = merge_skill_input(input, tool_outputs);
    let payload = build_ollama_generate_payload(
        provider.model.as_str(),
        prompt_text.as_deref(),
        &merged_input,
        provider.reasoning_effort.as_deref(),
    );
    let endpoint = provider_endpoint(provider);
    let request = client
        .post(endpoint)
        .header("content-type", "application/json")
        .json(&payload);
    let request = if let Some(api_key) = provider.api_key.as_deref() {
        request.bearer_auth(api_key)
    } else {
        request
    };
    let response = request.send().map_err(io::Error::other)?;
    let status_code = response.status();
    let body = response.text().map_err(io::Error::other)?;

    if !status_code.is_success() {
        return Ok(ToolExecutionOutcome {
            runner: provider.kind.runner().to_owned(),
            exit_code: Some(status_code.as_u16() as i32),
            status: ExecutionStatus::Failed,
            plan_steps: vec![
                format!("provider={} model={}", provider.kind.runner(), provider.model),
                format!("provider_http_status={status_code}"),
            ],
            output: truncate_output(body, DEFAULT_MAX_OUTPUT_BYTES).0,
        });
    }

    let value: Value = serde_json::from_str(&body).map_err(io::Error::other)?;
    let output = value
        .get("response")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| truncate_output(body, DEFAULT_MAX_OUTPUT_BYTES).0);
    let mut plan_steps = vec![
        format!("provider={} model={}", provider.kind.runner(), provider.model),
        format!(
            "provider_prompt_loaded={}",
            if prompt_text.is_some() { "true" } else { "false" }
        ),
        "provider_stream=false".to_owned(),
    ];
    if let Some(effort) = provider.reasoning_effort.as_deref() {
        plan_steps.push(format!("provider_reasoning_effort={effort}"));
    }
    if !tool_outputs.is_empty() {
        plan_steps.push(format!("tool_context_count={}", tool_outputs.len()));
    }

    Ok(ToolExecutionOutcome {
        runner: provider.kind.runner().to_owned(),
        exit_code: Some(status_code.as_u16() as i32),
        status: ExecutionStatus::Succeeded,
        plan_steps,
        output,
    })
}

fn load_prompt_component(root: &str, prompt_component: Option<&String>) -> io::Result<Option<String>> {
    let Some(prompt_component) = prompt_component else {
        return Ok(None);
    };
    let prompt_path = Path::new(prompt_component);
    let candidates = if prompt_path.is_absolute() {
        vec![prompt_path.to_path_buf()]
    } else {
        vec![
            Path::new(root).join(prompt_component),
            std::env::current_dir()?.join(prompt_component),
        ]
    };
    for candidate in candidates {
        if candidate.exists() {
            return fs::read_to_string(candidate).map(Some);
        }
    }
    Ok(None)
}

fn merge_skill_input(input: &str, tool_outputs: &[String]) -> String {
    if tool_outputs.is_empty() {
        return input.to_owned();
    }
    format!(
        "{input}\n\nTool context:\n{}",
        tool_outputs.join("\n")
    )
}

fn provider_endpoint(provider: &SkillProviderConfig) -> String {
    format!(
        "{}{}",
        provider.base_url.trim_end_matches('/'),
        provider.endpoint_path
    )
}

fn build_openai_responses_payload(
    model: &str,
    instructions: Option<&str>,
    input: &str,
    reasoning_effort: Option<&str>,
) -> Value {
    let mut payload = json!({
        "model": model,
        "input": input,
    });
    if let Some(instructions) = instructions
        && !instructions.trim().is_empty()
    {
        payload["instructions"] = Value::String(instructions.to_owned());
    }
    if let Some(reasoning_effort) = reasoning_effort
        && !reasoning_effort.trim().is_empty()
    {
        payload["reasoning"] = json!({
            "effort": reasoning_effort,
        });
    }
    payload
}

fn build_ollama_generate_payload(
    model: &str,
    system: Option<&str>,
    input: &str,
    reasoning_effort: Option<&str>,
) -> Value {
    let mut payload = json!({
        "model": model,
        "prompt": input,
        "stream": false,
    });
    if let Some(system) = system
        && !system.trim().is_empty()
    {
        payload["system"] = Value::String(system.to_owned());
    }
    if let Some(reasoning_effort) = reasoning_effort
        && !reasoning_effort.trim().is_empty()
    {
        payload["think"] = Value::String(reasoning_effort.to_owned());
    }
    payload
}

fn build_chat_completions_payload(
    model: &str,
    system: Option<&str>,
    input: &str,
    reasoning_effort: Option<&str>,
    reasoning_split: bool,
) -> Value {
    let mut messages = Vec::new();
    if let Some(system) = system
        && !system.trim().is_empty()
    {
        messages.push(json!({
            "role": "system",
            "content": system,
        }));
    }
    messages.push(json!({
        "role": "user",
        "content": input,
    }));

    let mut payload = json!({
        "model": model,
        "messages": messages,
    });
    if let Some(reasoning_effort) = reasoning_effort
        && !reasoning_effort.trim().is_empty()
    {
        payload["reasoning"] = json!({
            "effort": reasoning_effort,
        });
    }
    if reasoning_split {
        payload["reasoning_split"] = Value::Bool(true);
    }
    payload
}

fn extract_chat_completions_output_text(value: &Value) -> Option<String> {
    let choice = value.get("choices")?.as_array()?.first()?;
    let message = choice.get("message")?;
    if let Some(content) = message.get("content").and_then(Value::as_str) {
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_owned());
        }
    }
    None
}

fn extract_openai_response_output_text(value: &Value) -> Option<String> {
    if let Some(output_text) = value.get("output_text").and_then(Value::as_str) {
        return Some(output_text.trim().to_owned());
    }

    let output = value.get("output")?.as_array()?;
    let mut fragments = Vec::new();
    for item in output {
        let Some(content) = item.get("content").and_then(Value::as_array) else {
            continue;
        };
        for block in content {
            let Some(text) = block.get("text").and_then(Value::as_str) else {
                continue;
            };
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                fragments.push(trimmed.to_owned());
            }
        }
    }
    if fragments.is_empty() {
        None
    } else {
        Some(fragments.join("\n\n"))
    }
}

fn execute_local_shell(command: &str, input: &str) -> io::Result<ToolExecutionOutcome> {
    execute_local_shell_with_limits(
        command,
        input,
        DEFAULT_SHELL_TIMEOUT_MS,
        DEFAULT_MAX_OUTPUT_BYTES,
    )
}

fn execute_local_shell_with_limits(
    command: &str,
    input: &str,
    timeout_ms: u64,
    max_output_bytes: usize,
) -> io::Result<ToolExecutionOutcome> {
    let mut child = Command::new("sh")
        .arg("-lc")
        .arg(command)
        .env("HONEYCOMB_TOOL_INPUT", input)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input.as_bytes())?;
    }

    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    let mut timed_out = false;
    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        if Instant::now() >= deadline {
            timed_out = true;
            child.kill()?;
            break;
        }
        sleep(Duration::from_millis(10));
    }

    let output = child.wait_with_output()?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let combined_output = match (stdout.is_empty(), stderr.is_empty()) {
        (false, true) => stdout,
        (true, false) => stderr,
        (false, false) => format!("{stdout}\n[stderr]\n{stderr}"),
        (true, true) => "<no output>".to_owned(),
    };
    let (combined_output, output_truncated) = truncate_output(combined_output, max_output_bytes);
    let status = if timed_out {
        ExecutionStatus::TimedOut
    } else if output.status.success() {
        ExecutionStatus::Succeeded
    } else {
        ExecutionStatus::Failed
    };
    let mut plan_steps = vec![
        format!("shell_execute command={command}"),
        format!("shell_timeout_ms={timeout_ms}"),
    ];
    if output_truncated {
        plan_steps.push(format!("output_truncated max_bytes={max_output_bytes}"));
    }
    if timed_out {
        plan_steps.push("shell_killed_on_timeout".to_owned());
    }

    Ok(ToolExecutionOutcome {
        runner: "local-shell".to_owned(),
        exit_code: output.status.code(),
        status,
        plan_steps,
        output: combined_output,
    })
}

fn truncate_output(output: String, max_output_bytes: usize) -> (String, bool) {
    if output.len() <= max_output_bytes {
        return (output, false);
    }

    let mut truncated = String::new();
    for ch in output.chars() {
        if truncated.len() + ch.len_utf8() > max_output_bytes {
            break;
        }
        truncated.push(ch);
    }
    truncated.push_str("\n[truncated]");
    (truncated, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::{Mutex, OnceLock};

    fn env_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn non_shell_entrypoint_stays_simulated() {
        let outcome = execute_tool_entrypoint("tool://browser/login", "hello")
            .expect("simulated entrypoint should succeed");

        assert_eq!(outcome.runner, "local-simulated");
        assert_eq!(outcome.status, ExecutionStatus::Simulated);
    }

    /// `execute_local_shell` spawns `sh -lc` (Unix). Windows CI/dev shells omit `sh` by default.
    #[cfg(unix)]
    #[test]
    fn shell_entrypoint_executes_command() {
        let outcome = execute_tool_entrypoint(
            "shell://printf 'shell:%s' \"$HONEYCOMB_TOOL_INPUT\"",
            "world",
        )
        .expect("shell entrypoint should execute");

        assert_eq!(outcome.runner, "local-shell");
        assert_eq!(outcome.status, ExecutionStatus::Succeeded);
        assert_eq!(outcome.output, "shell:world");
    }

    #[cfg(unix)]
    #[test]
    fn shell_entrypoint_times_out() {
        let outcome = execute_local_shell_with_limits("sleep 0.05", "", 10, 1024)
            .expect("shell command should return timeout outcome");

        assert_eq!(outcome.runner, "local-shell");
        assert_eq!(outcome.status, ExecutionStatus::TimedOut);
        assert!(
            outcome
                .plan_steps
                .iter()
                .any(|step| step == "shell_killed_on_timeout")
        );
    }

    #[cfg(unix)]
    #[test]
    fn shell_entrypoint_truncates_output() {
        let outcome =
            execute_local_shell_with_limits("printf 'abcdefghijklmnopqrstuvwxyz'", "", 1000, 8)
                .expect("shell command should execute");

        assert_eq!(outcome.status, ExecutionStatus::Succeeded);
        assert!(outcome.output.ends_with("[truncated]"));
        assert!(
            outcome
                .plan_steps
                .iter()
                .any(|step| step == "output_truncated max_bytes=8")
        );
    }

    #[test]
    fn build_openai_responses_payload_includes_optional_fields() {
        let payload = build_openai_responses_payload(
            "gpt-4.1-mini",
            Some("system prompt"),
            "hello",
            Some("low"),
        );

        assert_eq!(payload["model"], "gpt-4.1-mini");
        assert_eq!(payload["input"], "hello");
        assert_eq!(payload["instructions"], "system prompt");
        assert_eq!(payload["reasoning"]["effort"], "low");
    }

    #[test]
    fn extract_openai_response_output_text_reads_output_items() {
        let value = json!({
            "output": [
                {
                    "content": [
                        {
                            "type": "output_text",
                            "text": "hello from model"
                        }
                    ]
                }
            ]
        });

        assert_eq!(
            extract_openai_response_output_text(&value).as_deref(),
            Some("hello from model")
        );
    }

    #[test]
    fn build_ollama_generate_payload_includes_optional_fields() {
        let payload =
            build_ollama_generate_payload("llama3.2", Some("system prompt"), "hello", Some("low"));

        assert_eq!(payload["model"], "llama3.2");
        assert_eq!(payload["prompt"], "hello");
        assert_eq!(payload["system"], "system prompt");
        assert_eq!(payload["think"], "low");
        assert_eq!(payload["stream"], false);
    }

    #[test]
    fn resolve_skill_provider_supports_ollama_and_openai_compatible() {
        let mut compatible = ImplementationRecord::new(
            "impl://compatible/demo".to_owned(),
            "demo".to_owned(),
            "openai_compatible_responses".to_owned(),
            crate::registry::ImplementationEntry::new("model".to_owned(), "gpt-4.1-mini".to_owned()),
            crate::registry::ImplementationCompatibility::new(
                "demo".to_owned(),
                "1.0.0".to_owned(),
                "1.0.0".to_owned(),
            ),
        );
        compatible.strategy.insert(
            "provider_base_url".to_owned(),
            "http://localhost:8000/v1".to_owned(),
        );
        let compatible_provider =
            resolve_skill_provider(&compatible).expect("provider should resolve");
        assert_eq!(
            compatible_provider.expect("provider should exist").base_url,
            "http://localhost:8000/v1"
        );

        let ollama = ImplementationRecord::new(
            "impl://ollama/demo".to_owned(),
            "demo".to_owned(),
            "ollama_generate".to_owned(),
            crate::registry::ImplementationEntry::new("model".to_owned(), "llama3.2".to_owned()),
            crate::registry::ImplementationCompatibility::new(
                "demo".to_owned(),
                "1.0.0".to_owned(),
                "1.0.0".to_owned(),
            ),
        );
        let ollama_provider = resolve_skill_provider(&ollama).expect("provider should resolve");
        let ollama_provider = ollama_provider.expect("provider should exist");
        assert_eq!(ollama_provider.kind, SkillProviderKind::OllamaGenerate);
        assert_eq!(ollama_provider.base_url, "http://localhost:11434/api");
        assert_eq!(ollama_provider.endpoint_path, "/generate");
    }

    #[test]
    fn generic_provider_env_matches_selected_type() {
        let _guard = env_test_lock().lock().expect("env test lock should hold");
        unsafe {
            env::set_var(GENERIC_LLM_PROVIDER_ENV, "minimax");
            env::set_var(GENERIC_LLM_API_KEY_ENV, "generic-key");
        }

        assert_eq!(
            generic_provider_env(&["minimax_chat", "minimax"], GENERIC_LLM_API_KEY_ENV)
                .as_deref(),
            Some("generic-key")
        );
        assert_eq!(
            generic_provider_env(&["openai", "openai_responses"], GENERIC_LLM_API_KEY_ENV),
            None
        );

        unsafe {
            env::remove_var(GENERIC_LLM_PROVIDER_ENV);
            env::remove_var(GENERIC_LLM_API_KEY_ENV);
        }
    }

    #[test]
    fn build_chat_completions_payload_includes_messages_and_reasoning_split() {
        let payload = build_chat_completions_payload(
            "MiniMax-M2.5",
            Some("system prompt"),
            "hello",
            Some("low"),
            true,
        );

        assert_eq!(payload["model"], "MiniMax-M2.5");
        assert_eq!(payload["messages"][0]["role"], "system");
        assert_eq!(payload["messages"][0]["content"], "system prompt");
        assert_eq!(payload["messages"][1]["role"], "user");
        assert_eq!(payload["messages"][1]["content"], "hello");
        assert_eq!(payload["reasoning"]["effort"], "low");
        assert_eq!(payload["reasoning_split"], true);
    }

    #[test]
    fn parse_env_value_trims_matching_quotes() {
        assert_eq!(parse_env_value("\"hello\""), "hello");
        assert_eq!(parse_env_value("'world'"), "world");
        assert_eq!(parse_env_value("plain"), "plain");
    }

    #[test]
    fn load_env_file_sets_missing_values_without_overriding_existing_ones() {
        let _guard = env_test_lock().lock().expect("env test lock should hold");
        let unique = format!(
            "HONEYCOMB_TEST_ENV_{}",
            current_timestamp().replace(':', "_")
        );
        let root = std::env::temp_dir().join(format!(
            "honeycomb-env-test-{}",
            current_timestamp().replace(':', "_")
        ));
        fs::create_dir_all(&root).expect("temp root should create");
        let env_path = root.join(".env");
        fs::write(&env_path, format!("{unique}=from_file\n"))
            .expect("env file should write");

        unsafe {
            env::remove_var(&unique);
        }
        load_env_file(&env_path).expect("env file should load");
        assert_eq!(env::var(&unique).as_deref(), Ok("from_file"));

        unsafe {
            env::set_var(&unique, "already_set");
        }
        load_env_file(&env_path).expect("env file should load twice");
        assert_eq!(env::var(&unique).as_deref(), Ok("already_set"));

        unsafe {
            env::remove_var(&unique);
        }
        let _ = fs::remove_dir_all(root);
    }
}
