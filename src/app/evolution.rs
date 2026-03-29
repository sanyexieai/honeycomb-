use std::process::ExitCode;

use serde::Serialize;

use crate::governance::{
    ArchitectureGuardrailSnapshot, ArchitectureReflectionDecision, ArchitectureReflectionRecord,
    ArchitectureReviewDecision, ArchitectureReviewRecord, ArchitectureReviewStatus, EvolutionPlan,
    FitnessReport, GovernanceDecision, GovernedImplementation, GuardrailSnapshotCount,
    ReviewTargetPlane,
};
use crate::registry::{GovernanceDefaultsRecord, ImplementationRecord, SkillRecord};
use crate::runtime::{AuditRecord, TaskRecord};
use crate::storage::{
    append_evolution_audit, list_architecture_reflections, list_architecture_reviews,
    list_execution_records, list_fitness_runs, list_implementations, list_policy_alert_acks,
    list_shell_approval_requests, list_skills, list_task_submissions, list_tools,
    load_architecture_reflection, load_architecture_review, load_evolution_audits,
    load_fitness_run, load_governance_defaults, load_implementation, load_skill,
    load_task_assignments, load_tool,
    persist_architecture_reflection, persist_architecture_review, persist_fitness_run,
    persist_governance_defaults, update_fitness_plan, update_skill,
    validate_skill_implementation_refs,
};

use super::cli::{BinaryRole, Command, option_value, option_values};

fn joined_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "<none>".to_owned()
    } else {
        values.join(", ")
    }
}

fn is_legacy_demo_task(record: &crate::runtime::TaskRecord) -> bool {
    record.task_spec.skill_refs.is_empty()
        && (record.task_spec.task_id.starts_with("task-demo-flow-")
            || record.task_spec.task_id == "task-xhs-demo")
}

fn runtime_task_implementation_id(task: &TaskRecord) -> Option<&str> {
    task.task_spec
        .implementation_snapshot
        .as_ref()
        .map(|snapshot| snapshot.implementation_id.as_str())
        .or(task.task_spec.implementation_ref.as_deref())
}

fn runtime_assignment_implementation_id(
    assignment: &crate::runtime::Assignment,
) -> Option<&str> {
    assignment
        .implementation_snapshot
        .as_ref()
        .map(|snapshot| snapshot.implementation_id.as_str())
        .or(assignment.implementation_ref.as_deref())
}

fn print_runtime_usage(root: &str, implementation_id: &str) -> std::io::Result<()> {
    let (_, tasks) = list_task_submissions(root)?;
    let matched_tasks = tasks
        .into_iter()
        .filter(|record| runtime_task_implementation_id(record) == Some(implementation_id))
        .collect::<Vec<_>>();

    println!("  runtime_task_count: {}", matched_tasks.len());
    for task in matched_tasks {
        let (_, assignments) = load_task_assignments(root, &task.task_spec.task_id)?;
        let matched_assignments = assignments
            .into_iter()
            .filter(|assignment| runtime_assignment_implementation_id(assignment) == Some(implementation_id))
            .collect::<Vec<_>>();

        println!(
            "  - task={} status={} goal={} assignment_count={}",
            task.task_spec.task_id,
            task.task_runtime.status.as_str(),
            task.task_spec.goal,
            matched_assignments.len()
        );
        for assignment in matched_assignments {
            println!(
                "    assignment={} worker={} status={} skills={} tools={}",
                assignment.assignment_id,
                assignment.worker_node_id,
                assignment.status.as_str(),
                joined_or_none(&assignment.skill_refs),
                joined_or_none(&assignment.tool_refs)
            );
        }
    }
    Ok(())
}

fn validate_registry_refs(
    root: &str,
    skill_refs: &[String],
    tool_refs: &[String],
) -> std::io::Result<()> {
    for skill_ref in skill_refs {
        load_skill(root, skill_ref)?;
    }

    for tool_ref in tool_refs {
        load_tool(root, tool_ref)?;
    }

    Ok(())
}

fn approval_request_age_ms(request: &crate::registry::ShellApprovalRequest) -> Option<u128> {
    let now = crate::core::parse_unix_ms_timestamp(&crate::core::current_timestamp())?;
    let requested_at = crate::core::parse_unix_ms_timestamp(&request.requested_at)?;
    Some(now.saturating_sub(requested_at))
}

fn approval_age_bucket(age_ms: Option<u128>) -> &'static str {
    match age_ms {
        Some(age) if age < 5 * 60 * 1000 => "fresh",
        Some(age) if age < 60 * 60 * 1000 => "stale",
        Some(_) => "overdue",
        None => "unknown",
    }
}

fn blocked_tool_alert_id(tool_id: &str) -> String {
    format!("blocked-tool-{tool_id}")
}

fn overdue_request_alert_id(request_id: &str) -> String {
    format!("overdue-request-{request_id}")
}

#[derive(Serialize)]
struct RegistryOverviewRecommendedSkillJson {
    skill_id: String,
    implementation_id: String,
    decision: String,
}

#[derive(Serialize)]
struct RegistryOverviewCountJson {
    id: String,
    task_count: usize,
}

#[derive(Serialize)]
struct RegistryOverviewImplementationUsageJson {
    implementation_id: String,
    recommended_by_skill_count: usize,
    runtime_task_count: usize,
    active_task_count: usize,
    runtime_assignment_count: usize,
    execution_count: usize,
}

#[derive(Serialize)]
struct RegistryOverviewImplementationSignalJson {
    implementation_id: String,
    skill_id: String,
    executor: String,
    mode: String,
    max_cost: String,
    max_latency_ms: String,
    flags: Vec<String>,
}

#[derive(Serialize)]
struct RegistryOverviewImplementationFlagCountJson {
    flag: String,
    count: usize,
}

#[derive(Serialize)]
struct RegistryOverviewImplementationHotspotJson {
    implementation_id: String,
    skill_id: String,
    executor: String,
    recent_guardrail_block_count: usize,
    top_reason: String,
    recommended_by_skill_count: usize,
    runtime_task_count: usize,
    active_task_count: usize,
    runtime_assignment_count: usize,
    execution_count: usize,
    flags: Vec<String>,
    refresh_min_absolute_increase: usize,
    refresh_min_multiplier: f64,
    refresh_min_severity_delta: usize,
    severity_weight_recommended_by: usize,
    severity_weight_active_tasks: usize,
    severity_weight_runtime_assignments: usize,
    severity_weight_executions: usize,
    severity_weight_severe_flags: usize,
    refresh_min_absolute_increase_source: String,
    refresh_min_multiplier_source: String,
    refresh_min_severity_delta_source: String,
    severity_weight_recommended_by_source: String,
    severity_weight_active_tasks_source: String,
    severity_weight_runtime_assignments_source: String,
    severity_weight_executions_source: String,
    severity_weight_severe_flags_source: String,
}

#[derive(Serialize)]
struct RegistryOverviewGovernanceDefaultJson {
    key: String,
    value: String,
}

#[derive(Serialize)]
struct RegistryOverviewGovernanceDefaultsJson {
    loaded: bool,
    policy_count: usize,
    updated_at: Option<String>,
    loaded_from: Option<String>,
    policies: Vec<RegistryOverviewGovernanceDefaultJson>,
}

#[derive(Serialize)]
struct ReviewSuggestionJson {
    suggested_review_id: String,
    suggestion_state: String,
    source_review_id: Option<String>,
    title: String,
    change_scope: String,
    target_plane: String,
    target_modules: Vec<String>,
    rationale: String,
    proposed_decision: String,
    required_followups: Vec<String>,
    evidence_refs: Vec<String>,
    implementation_id: String,
    skill_id: String,
    recent_guardrail_block_count: usize,
    recommended_by_skill_count: usize,
    active_task_count: usize,
    already_recorded: bool,
}

#[derive(Serialize)]
struct GovernanceDefaultsInspectJson<'a> {
    policy_count: usize,
    governance_policy: &'a std::collections::BTreeMap<String, String>,
    known_policy_keys: &'static [&'static str],
    updated_at: Option<&'a str>,
    loaded_from: String,
}

const KNOWN_GOVERNANCE_POLICY_KEYS: &[&str] = &[
    "review_refresh_min_absolute_increase",
    "review_refresh_min_multiplier",
    "review_refresh_min_severity_delta",
    "review_severity_weight_recommended_by",
    "review_severity_weight_active_tasks",
    "review_severity_weight_runtime_assignments",
    "review_severity_weight_executions",
    "review_severity_weight_severe_flags",
];

#[derive(Serialize)]
struct RegistryOverviewPolicyShellToolJson {
    tool_id: String,
    owner: String,
    scheme: String,
    trust_tier: String,
    allow_shell: bool,
    pending: bool,
}

#[derive(Serialize)]
struct RegistryOverviewPolicyOwnerJson {
    owner: String,
    count: usize,
}

#[derive(Serialize)]
struct RegistryOverviewPolicyAgeBucketJson {
    bucket: String,
    count: usize,
}

#[derive(Serialize)]
struct RegistryOverviewPolicyRequestJson {
    request_id: String,
    tool_id: String,
    owner: String,
    requested_by: String,
    requested_at: String,
    age_ms: Option<u128>,
    age_bucket: String,
}

#[derive(Serialize)]
struct RegistryOverviewPolicyAlertStatusJson {
    kind: String,
    target: String,
    acked: bool,
}

#[derive(Serialize)]
struct RegistryOverviewPolicyInboxJson {
    kind: String,
    target: String,
    owner: String,
    requested_by: Option<String>,
}

#[derive(Serialize)]
struct RegistryOverviewPolicyRecentChangeJson {
    timestamp: String,
    action: String,
    tool_id: String,
    result: String,
    detail: String,
}

#[derive(Serialize)]
struct RegistryOverviewGapBlockedShellToolJson {
    tool_id: String,
    owner: String,
    scheme: String,
    trust_tier: String,
    allow_shell: bool,
    pending: bool,
}

#[derive(Serialize)]
struct RegistryOverviewPolicyJson {
    tool_count: usize,
    shell_tool_count: usize,
    shell_tool_allowed_count: usize,
    shell_tool_blocked_count: usize,
    shell_tool_pending_count: usize,
    shell_request_count: usize,
    shell_request_pending_count: usize,
    shell_request_overdue_count: usize,
    alert_count: usize,
    unacked_alert_count: usize,
    acked_alert_count: usize,
    inbox_blocked_tool_count: usize,
    inbox_overdue_request_count: usize,
    inbox_count: usize,
    inbox_owner_count: usize,
    shell_request_pending_owner_count: usize,
    shell_request_pending_age_bucket_count: usize,
    recent_change_count: usize,
    shell_tools: Vec<RegistryOverviewPolicyShellToolJson>,
    inbox_owners: Vec<RegistryOverviewPolicyOwnerJson>,
    pending_request_owners: Vec<RegistryOverviewPolicyOwnerJson>,
    pending_request_age_buckets: Vec<RegistryOverviewPolicyAgeBucketJson>,
    pending_requests: Vec<RegistryOverviewPolicyRequestJson>,
    alert_statuses: Vec<RegistryOverviewPolicyAlertStatusJson>,
    inbox_alerts: Vec<RegistryOverviewPolicyInboxJson>,
    recent_changes: Vec<RegistryOverviewPolicyRecentChangeJson>,
}

#[derive(Serialize)]
struct RegistryOverviewGapsJson {
    skill_without_recommendation_count: usize,
    skill_without_recommendation: Vec<String>,
    task_without_implementation_no_skill_count: usize,
    task_without_implementation_no_skill: Vec<String>,
    task_without_implementation_missing_recommendation_count: usize,
    task_without_implementation_missing_recommendation: Vec<String>,
    blocked_shell_tool_count: usize,
    blocked_shell_tools: Vec<RegistryOverviewGapBlockedShellToolJson>,
    pending_shell_tool_count: usize,
    pending_shell_tools: Vec<String>,
}

#[derive(Serialize)]
struct RegistryOverviewDetailsJson {
    governance_defaults: RegistryOverviewGovernanceDefaultsJson,
    recommended_skill_detail_count: usize,
    recommended_skills: Vec<RegistryOverviewRecommendedSkillJson>,
    implementation_usage_detail_count: usize,
    implementation_usage: Vec<RegistryOverviewImplementationUsageJson>,
    implementation_signal_detail_count: usize,
    implementation_signals: Vec<RegistryOverviewImplementationSignalJson>,
    implementation_flag_count: usize,
    implementation_flags: Vec<RegistryOverviewImplementationFlagCountJson>,
    implementation_hotspot_detail_count: usize,
    implementation_hotspots: Vec<RegistryOverviewImplementationHotspotJson>,
    skill_usage_detail_count: usize,
    skill_usage: Vec<RegistryOverviewCountJson>,
    tool_usage_detail_count: usize,
    tool_usage: Vec<RegistryOverviewCountJson>,
}

#[derive(Serialize)]
struct RegistryOverviewJson {
    skills_dir: String,
    implementations_dir: String,
    tools_dir: String,
    fitness_dir: String,
    tasks_dir: String,
    exclude_legacy: bool,
    skill_count: usize,
    skill_with_recommendation_count: usize,
    implementation_count: usize,
    tool_count: usize,
    fitness_count: usize,
    fitness_promote_count: usize,
    fitness_hold_count: usize,
    fitness_observe_count: usize,
    fitness_deprecate_count: usize,
    task_count: usize,
    completed_task_count: usize,
    implementation_bound_task_count: usize,
    assignment_count: usize,
    completed_assignment_count: usize,
    policy: Option<RegistryOverviewPolicyJson>,
    gaps: Option<RegistryOverviewGapsJson>,
    details: Option<RegistryOverviewDetailsJson>,
}

#[derive(Serialize)]
struct ImplementationInspectJson<'a> {
    implementation_id: &'a str,
    skill_id: &'a str,
    executor: &'a str,
    entry_kind: &'a str,
    entry_path: &'a str,
    component_count: usize,
    strategy_count: usize,
    constraint_count: usize,
    compatibility_capability: &'a str,
    input_schema_version: &'a str,
    output_schema_version: &'a str,
    prompt_component: Option<&'a str>,
    config_component: Option<&'a str>,
    strategy_mode: Option<&'a str>,
    max_cost: Option<&'a str>,
    max_latency_ms: Option<&'a str>,
    governance_flags: Vec<String>,
    recent_guardrail_block_count: usize,
    top_guardrail_reason: Option<&'a str>,
    recommended_by_skill_count: usize,
    runtime_task_count: usize,
    active_task_count: usize,
    runtime_assignment_count: usize,
    execution_count: usize,
    origin_source: Option<&'a str>,
    origin_parent_impl: Option<&'a str>,
    loaded_from: String,
}

#[derive(Debug, Clone, Default)]
struct ImplementationUsageSummary {
    recommended_by_skill_count: usize,
    runtime_task_count: usize,
    active_task_count: usize,
    runtime_assignment_count: usize,
    execution_count: usize,
}

#[derive(Debug, Clone, Default)]
struct ImplementationGuardrailSummary {
    recent_guardrail_block_count: usize,
    top_reason: Option<String>,
}

struct GuardrailAuditSummary {
    total_count: usize,
    action_counts: Vec<(String, usize)>,
    reason_counts: Vec<(String, usize)>,
    target_type_counts: Vec<(String, usize)>,
    target_id_counts: Vec<(String, usize)>,
    skill_counts: Vec<(String, usize)>,
}

fn summarize_governance_defaults_for_overview(
    loaded: Option<(std::path::PathBuf, GovernanceDefaultsRecord)>,
) -> RegistryOverviewGovernanceDefaultsJson {
    match loaded {
        Some((path, record)) => {
            let policies = record
                .governance_policy
                .into_iter()
                .map(|(key, value)| RegistryOverviewGovernanceDefaultJson { key, value })
                .collect::<Vec<_>>();
            RegistryOverviewGovernanceDefaultsJson {
                loaded: true,
                policy_count: policies.len(),
                updated_at: record.updated_at,
                loaded_from: Some(path.display().to_string()),
                policies,
            }
        }
        None => RegistryOverviewGovernanceDefaultsJson {
            loaded: false,
            policy_count: 0,
            updated_at: None,
            loaded_from: None,
            policies: Vec::new(),
        },
    }
}

fn resolve_usize_setting_with_source(
    implementation_values: &std::collections::BTreeMap<String, String>,
    skill_values: Option<&std::collections::BTreeMap<String, String>>,
    global_values: Option<&std::collections::BTreeMap<String, String>>,
    key: &str,
    default: usize,
) -> (usize, &'static str) {
    if let Some(value) = implementation_values
        .get(key)
        .and_then(|value| value.parse::<usize>().ok())
    {
        return (value, "implementation");
    }
    if let Some(value) = skill_values
        .and_then(|values| values.get(key))
        .and_then(|value| value.parse::<usize>().ok())
    {
        return (value, "skill");
    }
    if let Some(value) = global_values
        .and_then(|values| values.get(key))
        .and_then(|value| value.parse::<usize>().ok())
    {
        return (value, "global");
    }
    (default, "built_in")
}

fn resolve_f64_setting_with_source(
    implementation_values: &std::collections::BTreeMap<String, String>,
    skill_values: Option<&std::collections::BTreeMap<String, String>>,
    global_values: Option<&std::collections::BTreeMap<String, String>>,
    key: &str,
    default: f64,
) -> (f64, &'static str) {
    if let Some(value) = implementation_values
        .get(key)
        .and_then(|value| value.parse::<f64>().ok())
    {
        return (value, "implementation");
    }
    if let Some(value) = skill_values
        .and_then(|values| values.get(key))
        .and_then(|value| value.parse::<f64>().ok())
    {
        return (value, "skill");
    }
    if let Some(value) = global_values
        .and_then(|values| values.get(key))
        .and_then(|value| value.parse::<f64>().ok())
    {
        return (value, "global");
    }
    (default, "built_in")
}

fn guardrail_snapshot_from_summary(
    summary: GuardrailAuditSummary,
) -> ArchitectureGuardrailSnapshot {
    ArchitectureGuardrailSnapshot::new(
        "last_30_days".to_owned(),
        summary.total_count,
        summary
            .action_counts
            .into_iter()
            .map(|(label, count)| GuardrailSnapshotCount::new(label, count))
            .collect(),
        summary
            .reason_counts
            .into_iter()
            .map(|(label, count)| GuardrailSnapshotCount::new(label, count))
            .collect(),
        summary
            .target_type_counts
            .into_iter()
            .map(|(label, count)| GuardrailSnapshotCount::new(label, count))
            .collect(),
        summary
            .target_id_counts
            .into_iter()
            .map(|(label, count)| GuardrailSnapshotCount::new(label, count))
            .collect(),
        summary
            .skill_counts
            .into_iter()
            .map(|(label, count)| GuardrailSnapshotCount::new(label, count))
            .collect(),
    )
}

pub(crate) fn handle(command: Command, args: &[String]) -> ExitCode {
    match command {
        Command::FitnessRun => handle_fitness_run(args),
        Command::FitnessExplain => handle_fitness_explain(args),
        Command::AuditTail => handle_evolution_audit_tail(args),
        Command::LineageShow => handle_lineage_show(args),
        Command::GovernancePlan => handle_governance_plan(args),
        Command::GovernanceApply => handle_governance_apply(args),
        Command::ReflectionRecord => handle_reflection_record(args),
        Command::ReflectionInspect => handle_reflection_inspect(args),
        Command::ReflectionList => handle_reflection_list(args),
        Command::ReviewRecord => handle_review_record(args),
        Command::ReviewSuggest => handle_review_suggest(args),
        Command::ReviewMaterialize => handle_review_materialize(args),
        Command::ReviewInspect => handle_review_inspect(args),
        Command::ReviewList => handle_review_list(args),
        Command::GovernanceDefaultsInspect => handle_governance_defaults_inspect(args),
        Command::GovernanceDefaultsSet => handle_governance_defaults_set(args),
        Command::RegistrySync => handle_registry_sync(args),
        Command::RegistryOverview => handle_registry_overview(args),
        Command::ImplementationInspect => handle_implementation_inspect(args),
        Command::ImplementationList => handle_implementation_list(args),
        other => {
            println!(
                "{} command scaffold: {}",
                BinaryRole::Evolution.binary_name(),
                super::cli::command_name(&other)
            );
            ExitCode::SUCCESS
        }
    }
}

fn print_implementation_record(
    record: &ImplementationRecord,
    loaded_from: Option<&std::path::Path>,
) {
    let governed = GovernedImplementation::from_record(record);
    let governance_flags = implementation_governance_flags(&governed);
    println!("  implementation_id: {}", record.implementation_id);
    println!("  skill_id: {}", record.skill_id);
    println!("  executor: {}", record.executor);
    println!("  entry.kind: {}", record.entry.kind);
    println!("  entry.path: {}", record.entry.path);
    println!("  component_count: {}", record.components.len());
    for (key, value) in &record.components {
        println!("  component: {key}={value}");
    }
    println!("  strategy_count: {}", record.strategy.len());
    for (key, value) in &record.strategy {
        println!("  strategy: {key}={value}");
    }
    println!(
        "  strategy_mode: {}",
        governed.strategy_mode.as_deref().unwrap_or("<none>")
    );
    println!(
        "  compatibility: capability={} input_schema_version={} output_schema_version={}",
        record.compatibility.capability,
        record.compatibility.input_schema_version,
        record.compatibility.output_schema_version
    );
    println!("  constraint_count: {}", record.constraints.len());
    for (key, value) in &record.constraints {
        println!("  constraint: {key}={value}");
    }
    println!(
        "  prompt_component: {}",
        governed.prompt_component.as_deref().unwrap_or("<none>")
    );
    println!(
        "  config_component: {}",
        governed.config_component.as_deref().unwrap_or("<none>")
    );
    println!(
        "  max_cost: {}",
        governed.max_cost.as_deref().unwrap_or("<none>")
    );
    println!(
        "  max_latency_ms: {}",
        governed.max_latency_ms.as_deref().unwrap_or("<none>")
    );
    println!(
        "  governance_flags: {}",
        if governance_flags.is_empty() {
            "<none>".to_owned()
        } else {
            governance_flags.join(", ")
        }
    );
    println!(
        "  origin.source: {}",
        record
            .origin
            .as_ref()
            .map(|origin| origin.source.as_str())
            .unwrap_or("<none>")
    );
    println!(
        "  origin.parent_impl: {}",
        record
            .origin
            .as_ref()
            .and_then(|origin| origin.parent_impl.as_deref())
            .unwrap_or("<none>")
    );
    if let Some(path) = loaded_from {
        println!("  loaded_from: {}", path.display());
    }
}

fn print_governance_defaults_record(
    record: &GovernanceDefaultsRecord,
    loaded_from: Option<&std::path::Path>,
) {
    println!("  policy_count: {}", record.governance_policy.len());
    println!(
        "  known_policy_keys: {}",
        KNOWN_GOVERNANCE_POLICY_KEYS.join(", ")
    );
    for (key, value) in &record.governance_policy {
        println!("  policy: {key}={value}");
    }
    println!(
        "  updated_at: {}",
        record.updated_at.as_deref().unwrap_or("<none>")
    );
    if let Some(path) = loaded_from {
        println!("  loaded_from: {}", path.display());
    }
}

fn handle_governance_defaults_inspect(args: &[String]) -> ExitCode {
    let as_json = args.iter().any(|arg| arg == "--json");
    let root = option_value(args, "--root").unwrap_or(".");

    let (path, record) = match load_governance_defaults(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to inspect governance defaults: {error}");
            return ExitCode::from(1);
        }
    };

    if as_json {
        let payload = GovernanceDefaultsInspectJson {
            policy_count: record.governance_policy.len(),
            governance_policy: &record.governance_policy,
            known_policy_keys: KNOWN_GOVERNANCE_POLICY_KEYS,
            updated_at: record.updated_at.as_deref(),
            loaded_from: path.display().to_string(),
        };
        match serde_json::to_string_pretty(&payload) {
            Ok(body) => {
                println!("{body}");
                return ExitCode::SUCCESS;
            }
            Err(error) => {
                eprintln!("failed to serialize governance defaults inspect json: {error}");
                return ExitCode::from(1);
            }
        }
    }

    println!("governance-defaults inspect loaded");
    print_governance_defaults_record(&record, Some(&path));
    ExitCode::SUCCESS
}

fn parse_policy_assignment(value: &str) -> Result<(String, String), String> {
    let Some((key, raw_value)) = value.split_once('=') else {
        return Err(format!(
            "invalid policy assignment `{value}`; expected KEY=VALUE"
        ));
    };
    let key = key.trim();
    let raw_value = raw_value.trim();
    if key.is_empty() {
        return Err(format!(
            "invalid policy assignment `{value}`; key must not be empty"
        ));
    }
    if raw_value.is_empty() {
        return Err(format!(
            "invalid policy assignment `{value}`; value must not be empty"
        ));
    }
    Ok((key.to_owned(), raw_value.to_owned()))
}

fn handle_governance_defaults_set(args: &[String]) -> ExitCode {
    let root = option_value(args, "--root").unwrap_or(".");
    let policy_assignments = option_values(args, "--policy");
    let clear_policy_keys = option_values(args, "--clear-policy");

    if policy_assignments.is_empty() && clear_policy_keys.is_empty() {
        eprintln!(
            "governance-defaults set requires at least one --policy KEY=VALUE or --clear-policy KEY"
        );
        return ExitCode::from(2);
    }

    let mut record = load_governance_defaults(root)
        .map(|(_, record)| record)
        .unwrap_or_else(|_| GovernanceDefaultsRecord::new());

    for assignment in policy_assignments {
        let (key, value) = match parse_policy_assignment(&assignment) {
            Ok(value) => value,
            Err(message) => {
                eprintln!("{message}");
                return ExitCode::from(2);
            }
        };
        record.governance_policy.insert(key, value);
    }

    for key in clear_policy_keys {
        let trimmed = key.trim();
        if trimmed.is_empty() {
            eprintln!("invalid --clear-policy value; key must not be empty");
            return ExitCode::from(2);
        }
        record.governance_policy.remove(trimmed);
    }

    record.updated_at = Some(crate::core::current_timestamp());

    let path = match persist_governance_defaults(root, &record) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("failed to persist governance defaults: {error}");
            return ExitCode::from(1);
        }
    };

    println!("governance-defaults set applied");
    print_governance_defaults_record(&record, Some(&path));
    ExitCode::SUCCESS
}

fn handle_implementation_inspect(args: &[String]) -> ExitCode {
    let implementation_id = option_value(args, "--implementation-id")
        .or_else(|| option_value(args, "--implementation"))
        .unwrap_or("impl-demo");
    let as_json = args.iter().any(|arg| arg == "--json");
    let root = option_value(args, "--root").unwrap_or(".");

    let (path, record) = match load_implementation(root, implementation_id) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to inspect implementation: {error}");
            return ExitCode::from(1);
        }
    };
    let governed = GovernedImplementation::from_record(&record);
    let governance_flags = implementation_governance_flags(&governed);
    let (_, skills) = match list_skills(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load skills for implementation inspect: {error}");
            return ExitCode::from(1);
        }
    };
    let (_, tasks) = match list_task_submissions(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load tasks for implementation inspect: {error}");
            return ExitCode::from(1);
        }
    };
    let usage = summarize_implementation_usage(root, &skills, &tasks);
    let guardrails =
        match summarize_guardrail_implementation_audits(root, None, Some(30 * 24 * 60 * 60 * 1000))
        {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to summarize implementation guardrails: {error}");
                return ExitCode::from(1);
            }
        };
    let implementation_usage = usage
        .get(&record.implementation_id)
        .cloned()
        .unwrap_or_default();
    let implementation_guardrail = guardrails
        .get(&record.implementation_id)
        .cloned()
        .unwrap_or_default();

    if as_json {
        let payload = ImplementationInspectJson {
            implementation_id: &record.implementation_id,
            skill_id: &record.skill_id,
            executor: &record.executor,
            entry_kind: &record.entry.kind,
            entry_path: &record.entry.path,
            component_count: record.components.len(),
            strategy_count: record.strategy.len(),
            constraint_count: record.constraints.len(),
            compatibility_capability: &record.compatibility.capability,
            input_schema_version: &record.compatibility.input_schema_version,
            output_schema_version: &record.compatibility.output_schema_version,
            prompt_component: governed.prompt_component.as_deref(),
            config_component: governed.config_component.as_deref(),
            strategy_mode: governed.strategy_mode.as_deref(),
            max_cost: governed.max_cost.as_deref(),
            max_latency_ms: governed.max_latency_ms.as_deref(),
            governance_flags,
            recent_guardrail_block_count: implementation_guardrail.recent_guardrail_block_count,
            top_guardrail_reason: implementation_guardrail.top_reason.as_deref(),
            recommended_by_skill_count: implementation_usage.recommended_by_skill_count,
            runtime_task_count: implementation_usage.runtime_task_count,
            active_task_count: implementation_usage.active_task_count,
            runtime_assignment_count: implementation_usage.runtime_assignment_count,
            execution_count: implementation_usage.execution_count,
            origin_source: record.origin.as_ref().map(|origin| origin.source.as_str()),
            origin_parent_impl: record
                .origin
                .as_ref()
                .and_then(|origin| origin.parent_impl.as_deref()),
            loaded_from: path.display().to_string(),
        };
        match serde_json::to_string_pretty(&payload) {
            Ok(body) => {
                println!("{body}");
                return ExitCode::SUCCESS;
            }
            Err(error) => {
                eprintln!("failed to serialize implementation inspect json: {error}");
                return ExitCode::from(1);
            }
        }
    }

    println!("implementation inspect loaded");
    print_implementation_record(&record, Some(&path));
    println!(
        "  recent_guardrail_block_count: {}",
        implementation_guardrail.recent_guardrail_block_count
    );
    println!(
        "  top_guardrail_reason: {}",
        implementation_guardrail
            .top_reason
            .as_deref()
            .unwrap_or("<none>")
    );
    println!(
        "  recommended_by_skill_count: {}",
        implementation_usage.recommended_by_skill_count
    );
    println!(
        "  runtime_task_count: {}",
        implementation_usage.runtime_task_count
    );
    println!(
        "  active_task_count: {}",
        implementation_usage.active_task_count
    );
    println!(
        "  runtime_assignment_count: {}",
        implementation_usage.runtime_assignment_count
    );
    println!("  execution_count: {}", implementation_usage.execution_count);
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::process::ExitCode;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        RegistryOverviewImplementationHotspotJson, ReviewSuggestionJson,
        append_guardrail_block_audit, build_review_suggestions, collect_implementation_hotspots,
        enrich_reflection_inputs_from_hotspots, enrich_review_inputs_from_hotspots,
        handle_governance_defaults_set, implementation_governance_flags,
        materialize_review_from_suggestion, parse_policy_assignment, recommended_decision,
        select_governance_candidate, select_registry_sync_candidate,
        summarize_governance_defaults_for_overview, summarize_guardrail_audits,
        summarize_guardrail_implementation_audits, summarize_implementation_usage,
    };
    use crate::governance::{
        ArchitectureReflectionDecision, ArchitectureReviewDecision, ArchitectureReviewRecord,
        ArchitectureReviewStatus, EvolutionPlan, FitnessReport, GovernanceDecision,
        GovernedImplementation, ReviewTargetPlane,
    };
    use crate::registry::{GovernanceDefaultsRecord, SkillRecord};
    use crate::runtime::{TaskRecord, TaskRuntime, TaskSpec, TaskStatus};
    use crate::storage::{
        load_architecture_reflection, load_architecture_review, load_evolution_audits,
        load_governance_defaults, persist_fitness_run, persist_implementation, persist_skill,
        persist_task_submission,
    };

    fn unique_test_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("honeycomb-evolution-app-test-{nanos}"))
    }

    fn governed_implementation() -> GovernedImplementation {
        GovernedImplementation {
            implementation_id: "impl-xhs-v1".to_owned(),
            skill_id: "xhs_publish".to_owned(),
            executor: "worker_process".to_owned(),
            entry_kind: "script".to_owned(),
            entry_path: "scripts/impl-xhs-v1.sh".to_owned(),
            component_count: 2,
            strategy_count: 1,
            constraint_count: 2,
            prompt_component: Some("prompts/xhs.md".to_owned()),
            config_component: Some("config/xhs.json".to_owned()),
            strategy_mode: Some("draft_then_publish".to_owned()),
            max_cost: Some("0.02".to_owned()),
            max_latency_ms: Some("5000".to_owned()),
            origin_source: Some("manual".to_owned()),
        }
    }

    fn sample_task(
        task_id: &str,
        implementation_id: Option<&str>,
        status: TaskStatus,
    ) -> TaskRecord {
        let spec = TaskSpec::new(
            task_id.to_owned(),
            "tenant-a".to_owned(),
            "ns-a".to_owned(),
            "goal".to_owned(),
            implementation_id.map(str::to_owned),
            Vec::new(),
            Vec::new(),
        );
        let mut runtime = TaskRuntime::queued(task_id.to_owned(), "queen-a".to_owned());
        runtime.status = status;
        TaskRecord {
            schema_version: "task.v1".to_owned(),
            task_spec: spec,
            task_runtime: runtime,
        }
    }

    #[test]
    fn recommended_decision_caps_promote_when_constraints_are_expensive() {
        let mut implementation = governed_implementation();
        implementation.max_cost = Some("0.08".to_owned());
        implementation.max_latency_ms = Some("12000".to_owned());

        let decision = recommended_decision(0.97, &implementation);

        assert_eq!(decision, GovernanceDecision::Hold);
    }

    #[test]
    fn summarize_governance_defaults_for_overview_preserves_sorted_keys() {
        let mut record = GovernanceDefaultsRecord::new();
        record.governance_policy.insert(
            "review_severity_weight_active_tasks".to_owned(),
            "4".to_owned(),
        );
        record.governance_policy.insert(
            "review_refresh_min_absolute_increase".to_owned(),
            "8".to_owned(),
        );
        record.updated_at = Some("unix_ms:123".to_owned());

        let summary = summarize_governance_defaults_for_overview(Some((
            PathBuf::from("/tmp/registry/governance-defaults.json"),
            record,
        )));

        assert!(summary.loaded);
        assert_eq!(summary.policy_count, 2);
        assert_eq!(summary.updated_at.as_deref(), Some("unix_ms:123"));
        assert_eq!(
            summary
                .policies
                .iter()
                .map(|item| item.key.as_str())
                .collect::<Vec<_>>(),
            vec![
                "review_refresh_min_absolute_increase",
                "review_severity_weight_active_tasks"
            ]
        );
    }

    #[test]
    fn collect_implementation_hotspots_marks_setting_source_layers() {
        let root = unique_test_root();
        let compatibility = crate::registry::ImplementationCompatibility::new(
            "publish".to_owned(),
            "1.0.0".to_owned(),
            "1.0.0".to_owned(),
        );
        let mut implementation = crate::registry::ImplementationRecord::new(
            "impl-source-layers".to_owned(),
            "xhs_publish".to_owned(),
            "worker_process".to_owned(),
            crate::registry::ImplementationEntry::new(
                "script".to_owned(),
                "scripts/impl-source-layers.sh".to_owned(),
            ),
            compatibility,
        );
        implementation.constraints.insert(
            "review_refresh_min_absolute_increase".to_owned(),
            "12".to_owned(),
        );
        persist_implementation(&root, &implementation)
            .expect("implementation should persist for source layer test");

        let mut skill = SkillRecord::new(
            "xhs_publish".to_owned(),
            "XHS Publish".to_owned(),
            "publish to xhs".to_owned(),
            "impl-source-layers".to_owned(),
            "system".to_owned(),
            "v1".to_owned(),
            Vec::new(),
            None,
        );
        skill.recommended_implementation_id = Some("impl-source-layers".to_owned());
        skill.governance_policy.insert(
            "review_severity_weight_active_tasks".to_owned(),
            "5".to_owned(),
        );
        persist_skill(&root, &skill).expect("skill should persist for source layer test");

        let mut defaults = GovernanceDefaultsRecord::new();
        defaults.governance_policy.insert(
            "review_severity_weight_recommended_by".to_owned(),
            "7".to_owned(),
        );
        crate::storage::persist_governance_defaults(&root, &defaults)
            .expect("global defaults should persist for source layer test");

        let spec = TaskSpec::new(
            "task-source-layers".to_owned(),
            "tenant-a".to_owned(),
            "ns-a".to_owned(),
            "goal".to_owned(),
            Some("impl-source-layers".to_owned()),
            vec!["xhs_publish".to_owned()],
            Vec::new(),
        );
        let runtime = TaskRuntime::queued("task-source-layers".to_owned(), "queen-a".to_owned());
        persist_task_submission(&root, &spec, &runtime)
            .expect("task should persist for source layer test");
        append_guardrail_block_audit(
            root.to_str().expect("root should be utf-8"),
            "governance_plan_guardrail_block",
            "governance_candidate",
            "impl-source-layers",
            "implementation=impl-source-layers skill_ref=xhs_publish reason=extreme_cost_budget"
                .to_owned(),
        )
        .expect("guardrail audit should persist for source layer test");

        let hotspots = collect_implementation_hotspots(
            root.to_str().expect("root should be utf-8"),
            None,
            Some(30 * 24 * 60 * 60 * 1000),
            Some(5),
        )
        .expect("hotspots should collect");

        assert_eq!(hotspots.len(), 1);
        assert_eq!(
            hotspots[0].refresh_min_absolute_increase_source,
            "implementation"
        );
        assert_eq!(hotspots[0].severity_weight_active_tasks_source, "skill");
        assert_eq!(hotspots[0].severity_weight_recommended_by_source, "global");
        assert_eq!(hotspots[0].severity_weight_severe_flags_source, "built_in");

        if root.exists() {
            fs::remove_dir_all(root).expect("temp directory should be removed");
        }
    }

    #[test]
    fn parse_policy_assignment_rejects_missing_separator() {
        let error = parse_policy_assignment("review_refresh_min_absolute_increase")
            .expect_err("assignment without separator should fail");

        assert!(error.contains("expected KEY=VALUE"));
    }

    #[test]
    fn governance_defaults_set_persists_policy_updates() {
        let root = unique_test_root();

        let initial = handle_governance_defaults_set(&[
            "governance-defaults".to_owned(),
            "set".to_owned(),
            "--policy".to_owned(),
            "review_refresh_min_absolute_increase=8".to_owned(),
            "--policy".to_owned(),
            "review_severity_weight_active_tasks=4".to_owned(),
            "--root".to_owned(),
            root.to_string_lossy().into_owned(),
        ]);
        assert_eq!(initial, ExitCode::SUCCESS);

        let (_, defaults) =
            load_governance_defaults(&root).expect("governance defaults should load after set");
        assert_eq!(
            defaults
                .governance_policy
                .get("review_refresh_min_absolute_increase")
                .map(String::as_str),
            Some("8")
        );
        assert_eq!(
            defaults
                .governance_policy
                .get("review_severity_weight_active_tasks")
                .map(String::as_str),
            Some("4")
        );
        assert!(defaults.updated_at.is_some());

        let cleared = handle_governance_defaults_set(&[
            "governance-defaults".to_owned(),
            "set".to_owned(),
            "--clear-policy".to_owned(),
            "review_refresh_min_absolute_increase".to_owned(),
            "--root".to_owned(),
            root.to_string_lossy().into_owned(),
        ]);
        assert_eq!(cleared, ExitCode::SUCCESS);

        let (_, updated) =
            load_governance_defaults(&root).expect("governance defaults should reload");
        assert!(
            !updated
                .governance_policy
                .contains_key("review_refresh_min_absolute_increase")
        );
        assert_eq!(
            updated
                .governance_policy
                .get("review_severity_weight_active_tasks")
                .map(String::as_str),
            Some("4")
        );

        if root.exists() {
            fs::remove_dir_all(root).expect("temp directory should be removed");
        }
    }

    #[test]
    fn implementation_governance_flags_include_sparse_and_budget_signals() {
        let mut implementation = governed_implementation();
        implementation.component_count = 0;
        implementation.strategy_count = 0;
        implementation.max_cost = Some("0.30".to_owned());
        implementation.max_latency_ms = Some("25000".to_owned());

        let flags = implementation_governance_flags(&implementation);

        assert!(flags.iter().any(|flag| flag == "no_components"));
        assert!(flags.iter().any(|flag| flag == "no_strategy"));
        assert!(flags.iter().any(|flag| flag == "high_cost_budget"));
        assert!(flags.iter().any(|flag| flag == "high_latency_budget"));
    }

    #[test]
    fn registry_sync_candidate_skips_extreme_risk_implementations() {
        let root = unique_test_root();
        let mut risky = governed_implementation();
        risky.implementation_id = "impl-risky".to_owned();
        risky.max_cost = Some("0.30".to_owned());
        let safe = GovernedImplementation {
            implementation_id: "impl-safe".to_owned(),
            ..governed_implementation()
        };
        let risky_report = FitnessReport::new(
            risky.clone(),
            "0.99".to_owned(),
            "too expensive".to_owned(),
            vec!["xhs_publish".to_owned()],
            vec![],
        );
        let risky_plan = EvolutionPlan::observe(risky, "observe".to_owned());
        let safe_report = FitnessReport::new(
            safe.clone(),
            "0.90".to_owned(),
            "usable".to_owned(),
            vec!["xhs_publish".to_owned()],
            vec![],
        );
        let safe_plan = EvolutionPlan::observe(safe, "observe".to_owned());

        persist_fitness_run(&root, &risky_report, &risky_plan)
            .expect("risky fitness run should persist");
        persist_fitness_run(&root, &safe_report, &safe_plan)
            .expect("safe fitness run should persist");

        let candidate = select_registry_sync_candidate(
            root.to_str().expect("root should be utf-8"),
            "xhs_publish",
        )
        .expect("selection should succeed")
        .expect("candidate should exist");

        assert_eq!(candidate.fitness_report.implementation_id(), "impl-safe");

        if root.exists() {
            fs::remove_dir_all(root).expect("temp directory should be removed");
        }
    }

    #[test]
    fn registry_sync_candidate_prefers_lower_penalty_when_decision_matches() {
        let root = unique_test_root();
        let low_penalty = GovernedImplementation {
            implementation_id: "impl-low-penalty".to_owned(),
            ..governed_implementation()
        };
        let mut high_penalty = governed_implementation();
        high_penalty.implementation_id = "impl-high-penalty".to_owned();
        high_penalty.max_latency_ms = Some("7000".to_owned());

        let low_report = FitnessReport::new(
            low_penalty.clone(),
            "0.91".to_owned(),
            "balanced".to_owned(),
            vec!["xhs_publish".to_owned()],
            vec![],
        );
        let low_plan = EvolutionPlan::observe(low_penalty, "observe".to_owned());
        let high_report = FitnessReport::new(
            high_penalty.clone(),
            "0.95".to_owned(),
            "riskier".to_owned(),
            vec!["xhs_publish".to_owned()],
            vec![],
        );
        let high_plan = EvolutionPlan::observe(high_penalty, "observe".to_owned());

        persist_fitness_run(&root, &high_report, &high_plan)
            .expect("high penalty fitness run should persist");
        persist_fitness_run(&root, &low_report, &low_plan)
            .expect("low penalty fitness run should persist");

        let candidate = select_registry_sync_candidate(
            root.to_str().expect("root should be utf-8"),
            "xhs_publish",
        )
        .expect("selection should succeed")
        .expect("candidate should exist");

        assert_eq!(
            candidate.fitness_report.implementation_id(),
            "impl-low-penalty"
        );

        if root.exists() {
            fs::remove_dir_all(root).expect("temp directory should be removed");
        }
    }

    #[test]
    fn governance_candidate_skips_extreme_risk_implementations_by_default() {
        let root = unique_test_root();
        let mut risky = governed_implementation();
        risky.implementation_id = "impl-risky-governance".to_owned();
        risky.max_latency_ms = Some("25000".to_owned());
        let safe = GovernedImplementation {
            implementation_id: "impl-safe-governance".to_owned(),
            ..governed_implementation()
        };
        let risky_report = FitnessReport::new(
            risky.clone(),
            "0.99".to_owned(),
            "too slow".to_owned(),
            vec!["xhs_publish".to_owned()],
            vec![],
        );
        let risky_plan = EvolutionPlan::observe(risky, "observe".to_owned());
        let safe_report = FitnessReport::new(
            safe.clone(),
            "0.90".to_owned(),
            "usable".to_owned(),
            vec!["xhs_publish".to_owned()],
            vec![],
        );
        let safe_plan = EvolutionPlan::observe(safe, "observe".to_owned());

        persist_fitness_run(&root, &risky_report, &risky_plan)
            .expect("risky governance fitness run should persist");
        persist_fitness_run(&root, &safe_report, &safe_plan)
            .expect("safe governance fitness run should persist");

        let candidate = select_governance_candidate(
            root.to_str().expect("root should be utf-8"),
            None,
            Some("xhs_publish"),
            None,
        )
        .expect("selection should succeed")
        .expect("candidate should exist");

        assert_eq!(
            candidate.fitness_report.implementation_id(),
            "impl-safe-governance"
        );

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn governance_candidate_allows_explicit_implementation_override() {
        let root = unique_test_root();
        let mut risky = governed_implementation();
        risky.implementation_id = "impl-explicit-risky".to_owned();
        risky.max_cost = Some("0.30".to_owned());
        let risky_report = FitnessReport::new(
            risky.clone(),
            "0.99".to_owned(),
            "too expensive".to_owned(),
            vec!["xhs_publish".to_owned()],
            vec![],
        );
        let risky_plan = EvolutionPlan::observe(risky, "observe".to_owned());

        persist_fitness_run(&root, &risky_report, &risky_plan)
            .expect("explicit risky fitness run should persist");

        let candidate = select_governance_candidate(
            root.to_str().expect("root should be utf-8"),
            Some("impl-explicit-risky"),
            Some("xhs_publish"),
            None,
        )
        .expect("selection should succeed")
        .expect("explicit implementation should still be selectable");

        assert_eq!(
            candidate.fitness_report.implementation_id(),
            "impl-explicit-risky"
        );

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn append_guardrail_block_audit_persists_evolution_audit_record() {
        let root = unique_test_root();

        append_guardrail_block_audit(
            root.to_str().expect("root should be utf-8"),
            "governance_plan_guardrail_block",
            "governance_candidate",
            "impl-risky",
            "implementation=impl-risky reason=all matching governance candidates blocked by guardrails: extreme_cost_budget".to_owned(),
        )
        .expect("guardrail block audit should persist");

        let (_, audits) =
            load_evolution_audits(&root).expect("evolution audits should load after guardrail");
        let latest = audits.last().expect("at least one audit should exist");
        assert_eq!(latest.action, "governance_plan_guardrail_block");
        assert_eq!(latest.result, "guardrail_blocked");
        assert_eq!(latest.target_id, "impl-risky");
        assert!(latest.payload.contains("extreme_cost_budget"));

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn summarize_guardrail_audits_groups_actions_and_reasons() {
        let root = unique_test_root();

        append_guardrail_block_audit(
            root.to_str().expect("root should be utf-8"),
            "governance_plan_guardrail_block",
            "governance_candidate",
            "impl-a",
            "implementation=impl-a reason=all matching governance candidates blocked by guardrails: extreme_cost_budget".to_owned(),
        )
        .expect("first guardrail audit should persist");
        append_guardrail_block_audit(
            root.to_str().expect("root should be utf-8"),
            "registry_sync_guardrail_block",
            "skill",
            "xhs_publish",
            "skill=xhs_publish skipped_reasons=extreme_cost_budget".to_owned(),
        )
        .expect("second guardrail audit should persist");

        let summary =
            summarize_guardrail_audits(root.to_str().expect("root should be utf-8"), None, None)
                .expect("guardrail summary should load");

        assert_eq!(summary.total_count, 2);
        assert!(
            summary
                .action_counts
                .iter()
                .any(|(action, count)| action == "governance_plan_guardrail_block" && *count == 1)
        );
        assert!(
            summary
                .reason_counts
                .iter()
                .any(|(reason, count)| reason.contains("extreme_cost_budget") && *count >= 1)
        );
        assert!(
            summary
                .target_type_counts
                .iter()
                .any(|(label, count)| label == "skill" && *count == 1)
        );
        assert!(
            summary
                .target_id_counts
                .iter()
                .any(|(label, count)| label == "xhs_publish" && *count == 1)
        );

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn summarize_guardrail_audits_extracts_skill_facets() {
        let root = unique_test_root();

        append_guardrail_block_audit(
            root.to_str().expect("root should be utf-8"),
            "governance_plan_guardrail_block",
            "governance_candidate",
            "impl-a",
            "implementation=impl-a skill_ref=xhs_publish reason=all matching governance candidates blocked by guardrails: extreme_latency_budget".to_owned(),
        )
        .expect("guardrail audit should persist");

        let summary =
            summarize_guardrail_audits(root.to_str().expect("root should be utf-8"), None, None)
                .expect("summary should load");

        assert!(
            summary
                .skill_counts
                .iter()
                .any(|(label, count)| label == "xhs_publish" && *count == 1)
        );

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn summarize_guardrail_implementation_audits_groups_by_implementation() {
        let root = unique_test_root();

        append_guardrail_block_audit(
            root.to_str().expect("root should be utf-8"),
            "governance_plan_guardrail_block",
            "governance_candidate",
            "impl-a",
            "implementation=impl-a skill_ref=xhs_publish reason=extreme_cost_budget".to_owned(),
        )
        .expect("first implementation guardrail audit should persist");
        append_guardrail_block_audit(
            root.to_str().expect("root should be utf-8"),
            "governance_apply_guardrail_block",
            "governance_candidate",
            "impl-a",
            "implementation=impl-a skill_ref=xhs_publish reason=extreme_cost_budget".to_owned(),
        )
        .expect("second implementation guardrail audit should persist");
        append_guardrail_block_audit(
            root.to_str().expect("root should be utf-8"),
            "registry_sync_guardrail_block",
            "skill",
            "xhs_publish",
            "skill=xhs_publish skipped_reasons=extreme_cost_budget".to_owned(),
        )
        .expect("skill-level guardrail audit should persist");

        let summary = summarize_guardrail_implementation_audits(
            root.to_str().expect("root should be utf-8"),
            None,
            None,
        )
        .expect("implementation summary should load");
        let implementation = summary.get("impl-a").expect("impl-a should be summarized");

        assert_eq!(implementation.recent_guardrail_block_count, 2);
        assert_eq!(
            implementation.top_reason.as_deref(),
            Some("extreme_cost_budget")
        );
        assert_eq!(summary.len(), 1);

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn summarize_implementation_usage_tracks_recommendations_and_active_tasks() {
        let mut skill = SkillRecord::new(
            "xhs_publish".to_owned(),
            "XHS Publish".to_owned(),
            "publish to xhs".to_owned(),
            "impl-a".to_owned(),
            "system".to_owned(),
            "v1".to_owned(),
            Vec::new(),
            None,
        );
        skill.recommended_implementation_id = Some("impl-a".to_owned());
        let skills = vec![skill];
        let tasks = vec![
            sample_task("task-a", Some("impl-a"), TaskStatus::Queued),
            sample_task("task-b", Some("impl-a"), TaskStatus::Completed),
            sample_task("task-c", Some("impl-b"), TaskStatus::Running),
        ];

        let root = unique_test_root();
        let summary = summarize_implementation_usage(
            root.to_str().expect("root should be utf-8"),
            &skills,
            &tasks,
        );
        let impl_a = summary.get("impl-a").expect("impl-a should exist");
        let impl_b = summary.get("impl-b").expect("impl-b should exist");

        assert_eq!(impl_a.recommended_by_skill_count, 1);
        assert_eq!(impl_a.runtime_task_count, 2);
        assert_eq!(impl_a.active_task_count, 1);
        assert_eq!(impl_a.runtime_assignment_count, 0);
        assert_eq!(impl_a.execution_count, 0);
        assert_eq!(impl_b.recommended_by_skill_count, 0);
        assert_eq!(impl_b.runtime_task_count, 1);
        assert_eq!(impl_b.active_task_count, 1);
        assert_eq!(impl_b.runtime_assignment_count, 0);
        assert_eq!(impl_b.execution_count, 0);

        if root.exists() {
            fs::remove_dir_all(root).expect("temp directory should be removed");
        }
    }

    #[test]
    fn collect_implementation_hotspots_filters_to_recommended_or_active_guardrail_targets() {
        let root = unique_test_root();
        let compatibility = crate::registry::ImplementationCompatibility::new(
            "publish".to_owned(),
            "1.0.0".to_owned(),
            "1.0.0".to_owned(),
        );
        let implementation = crate::registry::ImplementationRecord::new(
            "impl-hot".to_owned(),
            "xhs_publish".to_owned(),
            "worker_process".to_owned(),
            crate::registry::ImplementationEntry::new(
                "script".to_owned(),
                "scripts/impl-hot.sh".to_owned(),
            ),
            compatibility,
        );
        persist_implementation(&root, &implementation)
            .expect("implementation should persist for hotspot collection");
        let mut skill = SkillRecord::new(
            "xhs_publish".to_owned(),
            "XHS Publish".to_owned(),
            "publish to xhs".to_owned(),
            "impl-hot".to_owned(),
            "system".to_owned(),
            "v1".to_owned(),
            Vec::new(),
            None,
        );
        skill.recommended_implementation_id = Some("impl-hot".to_owned());
        persist_skill(&root, &skill).expect("skill should persist for hotspot collection");
        let spec = TaskSpec::new(
            "task-hot".to_owned(),
            "tenant-a".to_owned(),
            "ns-a".to_owned(),
            "goal".to_owned(),
            Some("impl-hot".to_owned()),
            vec!["xhs_publish".to_owned()],
            Vec::new(),
        );
        let runtime = TaskRuntime::queued("task-hot".to_owned(), "queen-a".to_owned());
        persist_task_submission(&root, &spec, &runtime)
            .expect("task should persist for hotspot collection");
        append_guardrail_block_audit(
            root.to_str().expect("root should be utf-8"),
            "governance_plan_guardrail_block",
            "governance_candidate",
            "impl-hot",
            "implementation=impl-hot skill_ref=xhs_publish reason=extreme_cost_budget".to_owned(),
        )
        .expect("guardrail block audit should persist");

        let hotspots = collect_implementation_hotspots(
            root.to_str().expect("root should be utf-8"),
            None,
            Some(30 * 24 * 60 * 60 * 1000),
            Some(3),
        )
        .expect("hotspots should collect");

        assert_eq!(hotspots.len(), 1);
        assert_eq!(hotspots[0].implementation_id, "impl-hot");
        assert_eq!(hotspots[0].recommended_by_skill_count, 1);
        assert_eq!(hotspots[0].active_task_count, 1);

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn hotspot_enrichment_adds_followups_and_evidence() {
        let hotspots = vec![RegistryOverviewImplementationHotspotJson {
            implementation_id: "impl-hot".to_owned(),
            skill_id: "xhs_publish".to_owned(),
            executor: "worker_process".to_owned(),
            recent_guardrail_block_count: 3,
            top_reason: "extreme_cost_budget".to_owned(),
            recommended_by_skill_count: 1,
            runtime_task_count: 4,
            active_task_count: 2,
            runtime_assignment_count: 0,
            execution_count: 0,
            flags: vec!["high_cost_budget".to_owned()],
            refresh_min_absolute_increase: 3,
            refresh_min_multiplier: 2.0,
            refresh_min_severity_delta: 3,
            severity_weight_recommended_by: 2,
            severity_weight_active_tasks: 2,
            severity_weight_runtime_assignments: 1,
            severity_weight_executions: 1,
            severity_weight_severe_flags: 1,
            refresh_min_absolute_increase_source: "built_in".to_owned(),
            refresh_min_multiplier_source: "built_in".to_owned(),
            refresh_min_severity_delta_source: "built_in".to_owned(),
            severity_weight_recommended_by_source: "built_in".to_owned(),
            severity_weight_active_tasks_source: "built_in".to_owned(),
            severity_weight_runtime_assignments_source: "built_in".to_owned(),
            severity_weight_executions_source: "built_in".to_owned(),
            severity_weight_severe_flags_source: "built_in".to_owned(),
        }];
        let mut detected_drifts = Vec::new();
        let mut freeze_actions = Vec::new();
        let mut next_actions = Vec::new();
        let mut reflection_evidence = Vec::new();
        enrich_reflection_inputs_from_hotspots(
            &mut detected_drifts,
            &mut freeze_actions,
            &mut next_actions,
            &mut reflection_evidence,
            &hotspots,
        );
        let mut followups = Vec::new();
        let mut review_evidence = Vec::new();
        enrich_review_inputs_from_hotspots(&mut followups, &mut review_evidence, &hotspots);

        assert!(detected_drifts.iter().any(|line| line.contains("impl-hot")));
        assert!(
            freeze_actions
                .iter()
                .any(|line| line.contains("freeze recommendation"))
        );
        assert!(
            next_actions
                .iter()
                .any(|line| line.contains("guardrail reason extreme_cost_budget"))
        );
        assert!(
            reflection_evidence
                .iter()
                .any(|line| line == "implementation_hotspot:impl-hot")
        );
        assert!(
            followups
                .iter()
                .any(|line| line.contains("reassess implementation impl-hot"))
        );
        assert!(followups.iter().any(|line| line.contains("active task")));
        assert!(
            review_evidence
                .iter()
                .any(|line| line == "implementation_hotspot:impl-hot")
        );
    }

    #[test]
    fn build_review_suggestions_maps_hotspots_to_review_candidates() {
        let hotspots = vec![RegistryOverviewImplementationHotspotJson {
            implementation_id: "impl-hot".to_owned(),
            skill_id: "xhs_publish".to_owned(),
            executor: "worker_process".to_owned(),
            recent_guardrail_block_count: 4,
            top_reason: "extreme_cost_budget".to_owned(),
            recommended_by_skill_count: 1,
            runtime_task_count: 4,
            active_task_count: 1,
            runtime_assignment_count: 0,
            execution_count: 0,
            flags: vec!["high_cost_budget".to_owned()],
            refresh_min_absolute_increase: 3,
            refresh_min_multiplier: 2.0,
            refresh_min_severity_delta: 3,
            severity_weight_recommended_by: 2,
            severity_weight_active_tasks: 2,
            severity_weight_runtime_assignments: 1,
            severity_weight_executions: 1,
            severity_weight_severe_flags: 1,
            refresh_min_absolute_increase_source: "built_in".to_owned(),
            refresh_min_multiplier_source: "built_in".to_owned(),
            refresh_min_severity_delta_source: "built_in".to_owned(),
            severity_weight_recommended_by_source: "built_in".to_owned(),
            severity_weight_active_tasks_source: "built_in".to_owned(),
            severity_weight_runtime_assignments_source: "built_in".to_owned(),
            severity_weight_executions_source: "built_in".to_owned(),
            severity_weight_severe_flags_source: "built_in".to_owned(),
        }];

        let existing = vec![ArchitectureReviewRecord::new(
            "review-impl-hot-guardrail-hotspot".to_owned(),
            "guardrail hotspot review for impl-hot".to_owned(),
            "implementation_guardrail_hotspot".to_owned(),
            "system-guardrail".to_owned(),
            ReviewTargetPlane::Evolution,
            vec!["governance".to_owned()],
            false,
            true,
            false,
            true,
            false,
            ArchitectureReviewStatus::Open,
            ArchitectureReviewDecision::NeedsRedesign,
            "implementation impl-hot triggered 2 recent guardrail blocks for extreme_cost_budget while recommended_by=1 active_tasks=1".to_owned(),
            vec![],
            vec![],
            None,
        )];
        let suggestions = build_review_suggestions(&hotspots, &existing);

        assert_eq!(suggestions.len(), 1);
        assert_eq!(
            suggestions[0].suggested_review_id,
            "review-impl-hot-guardrail-hotspot-refresh-2"
        );
        assert_eq!(suggestions[0].proposed_decision, "needs_redesign");
        assert!(suggestions[0].rationale.contains("impl-hot"));
        assert!(
            suggestions[0]
                .required_followups
                .iter()
                .any(|line| line.contains("reassess implementation impl-hot"))
        );
        assert_eq!(suggestions[0].suggestion_state, "worsened");
        assert!(!suggestions[0].already_recorded);
        assert_eq!(
            suggestions[0].source_review_id.as_deref(),
            Some("review-impl-hot-guardrail-hotspot")
        );
        assert_eq!(
            suggestions[0].suggested_review_id,
            "review-impl-hot-guardrail-hotspot-refresh-2"
        );
    }

    #[test]
    fn build_review_suggestions_keeps_existing_when_increase_is_not_meaningful() {
        let hotspots = vec![RegistryOverviewImplementationHotspotJson {
            implementation_id: "impl-hot".to_owned(),
            skill_id: "xhs_publish".to_owned(),
            executor: "worker_process".to_owned(),
            recent_guardrail_block_count: 3,
            top_reason: "extreme_cost_budget".to_owned(),
            recommended_by_skill_count: 1,
            runtime_task_count: 4,
            active_task_count: 1,
            runtime_assignment_count: 0,
            execution_count: 0,
            flags: vec!["high_cost_budget".to_owned()],
            refresh_min_absolute_increase: 3,
            refresh_min_multiplier: 2.0,
            refresh_min_severity_delta: 3,
            severity_weight_recommended_by: 2,
            severity_weight_active_tasks: 2,
            severity_weight_runtime_assignments: 1,
            severity_weight_executions: 1,
            severity_weight_severe_flags: 1,
            refresh_min_absolute_increase_source: "built_in".to_owned(),
            refresh_min_multiplier_source: "built_in".to_owned(),
            refresh_min_severity_delta_source: "built_in".to_owned(),
            severity_weight_recommended_by_source: "built_in".to_owned(),
            severity_weight_active_tasks_source: "built_in".to_owned(),
            severity_weight_runtime_assignments_source: "built_in".to_owned(),
            severity_weight_executions_source: "built_in".to_owned(),
            severity_weight_severe_flags_source: "built_in".to_owned(),
        }];

        let existing = vec![ArchitectureReviewRecord::new(
            "review-impl-hot-guardrail-hotspot".to_owned(),
            "guardrail hotspot review for impl-hot".to_owned(),
            "implementation_guardrail_hotspot".to_owned(),
            "system-guardrail".to_owned(),
            ReviewTargetPlane::Evolution,
            vec!["governance".to_owned()],
            false,
            true,
            false,
            true,
            false,
            ArchitectureReviewStatus::Open,
            ArchitectureReviewDecision::NeedsRedesign,
            "implementation impl-hot triggered 2 recent guardrail blocks for extreme_cost_budget while recommended_by=1 active_tasks=1".to_owned(),
            vec![],
            vec![],
            None,
        )];
        let suggestions = build_review_suggestions(&hotspots, &existing);

        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].suggestion_state, "existing");
        assert!(suggestions[0].already_recorded);
        assert_eq!(
            suggestions[0].suggested_review_id,
            "review-impl-hot-guardrail-hotspot"
        );
    }

    #[test]
    fn build_review_suggestions_refreshes_when_severity_increases_without_large_count_jump() {
        let hotspots = vec![RegistryOverviewImplementationHotspotJson {
            implementation_id: "impl-hot".to_owned(),
            skill_id: "xhs_publish".to_owned(),
            executor: "worker_process".to_owned(),
            recent_guardrail_block_count: 3,
            top_reason: "extreme_cost_budget".to_owned(),
            recommended_by_skill_count: 2,
            runtime_task_count: 5,
            active_task_count: 2,
            runtime_assignment_count: 0,
            execution_count: 0,
            flags: vec![
                "high_cost_budget".to_owned(),
                "high_latency_budget".to_owned(),
            ],
            refresh_min_absolute_increase: 3,
            refresh_min_multiplier: 2.0,
            refresh_min_severity_delta: 3,
            severity_weight_recommended_by: 2,
            severity_weight_active_tasks: 2,
            severity_weight_runtime_assignments: 1,
            severity_weight_executions: 1,
            severity_weight_severe_flags: 1,
            refresh_min_absolute_increase_source: "built_in".to_owned(),
            refresh_min_multiplier_source: "built_in".to_owned(),
            refresh_min_severity_delta_source: "built_in".to_owned(),
            severity_weight_recommended_by_source: "built_in".to_owned(),
            severity_weight_active_tasks_source: "built_in".to_owned(),
            severity_weight_runtime_assignments_source: "built_in".to_owned(),
            severity_weight_executions_source: "built_in".to_owned(),
            severity_weight_severe_flags_source: "built_in".to_owned(),
        }];

        let existing = vec![ArchitectureReviewRecord::new(
            "review-impl-hot-guardrail-hotspot".to_owned(),
            "guardrail hotspot review for impl-hot".to_owned(),
            "implementation_guardrail_hotspot".to_owned(),
            "system-guardrail".to_owned(),
            ReviewTargetPlane::Evolution,
            vec!["governance".to_owned()],
            false,
            true,
            false,
            true,
            false,
            ArchitectureReviewStatus::Open,
            ArchitectureReviewDecision::NeedsRedesign,
            "implementation impl-hot triggered 2 recent guardrail blocks for extreme_cost_budget while recommended_by=1 active_tasks=1 severe_flags=0".to_owned(),
            vec![],
            vec![],
            None,
        )];
        let suggestions = build_review_suggestions(&hotspots, &existing);

        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].suggestion_state, "worsened");
        assert_eq!(
            suggestions[0].suggested_review_id,
            "review-impl-hot-guardrail-hotspot-refresh-2"
        );
        assert!(suggestions[0].rationale.contains("severe_flags=2"));
    }

    #[test]
    fn build_review_suggestions_respects_custom_refresh_policy_from_hotspot() {
        let hotspots = vec![RegistryOverviewImplementationHotspotJson {
            implementation_id: "impl-hot".to_owned(),
            skill_id: "xhs_publish".to_owned(),
            executor: "worker_process".to_owned(),
            recent_guardrail_block_count: 4,
            top_reason: "extreme_cost_budget".to_owned(),
            recommended_by_skill_count: 1,
            runtime_task_count: 4,
            active_task_count: 1,
            runtime_assignment_count: 0,
            execution_count: 0,
            flags: vec!["high_cost_budget".to_owned()],
            refresh_min_absolute_increase: 10,
            refresh_min_multiplier: 10.0,
            refresh_min_severity_delta: 10,
            severity_weight_recommended_by: 1,
            severity_weight_active_tasks: 1,
            severity_weight_runtime_assignments: 1,
            severity_weight_executions: 1,
            severity_weight_severe_flags: 1,
            refresh_min_absolute_increase_source: "implementation".to_owned(),
            refresh_min_multiplier_source: "implementation".to_owned(),
            refresh_min_severity_delta_source: "implementation".to_owned(),
            severity_weight_recommended_by_source: "implementation".to_owned(),
            severity_weight_active_tasks_source: "implementation".to_owned(),
            severity_weight_runtime_assignments_source: "implementation".to_owned(),
            severity_weight_executions_source: "implementation".to_owned(),
            severity_weight_severe_flags_source: "implementation".to_owned(),
        }];

        let existing = vec![ArchitectureReviewRecord::new(
            "review-impl-hot-guardrail-hotspot".to_owned(),
            "guardrail hotspot review for impl-hot".to_owned(),
            "implementation_guardrail_hotspot".to_owned(),
            "system-guardrail".to_owned(),
            ReviewTargetPlane::Evolution,
            vec!["governance".to_owned()],
            false,
            true,
            false,
            true,
            false,
            ArchitectureReviewStatus::Open,
            ArchitectureReviewDecision::NeedsRedesign,
            "implementation impl-hot triggered 2 recent guardrail blocks for extreme_cost_budget while recommended_by=1 active_tasks=1 assignments=0 executions=0 severe_flags=1 refresh_min_absolute_increase=10 refresh_min_multiplier=10 refresh_min_severity_delta=10 severity_weight_recommended_by=1 severity_weight_active_tasks=1 severity_weight_runtime_assignments=1 severity_weight_executions=1 severity_weight_severe_flags=1".to_owned(),
            vec![],
            vec![],
            None,
        )];

        let suggestions = build_review_suggestions(&hotspots, &existing);

        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].suggestion_state, "existing");
        assert!(suggestions[0].already_recorded);
    }

    #[test]
    fn build_review_suggestions_refreshes_when_assignment_and_execution_pressure_increases() {
        let hotspots = vec![RegistryOverviewImplementationHotspotJson {
            implementation_id: "impl-hot".to_owned(),
            skill_id: "xhs_publish".to_owned(),
            executor: "worker_process".to_owned(),
            recent_guardrail_block_count: 3,
            top_reason: "extreme_cost_budget".to_owned(),
            recommended_by_skill_count: 1,
            runtime_task_count: 4,
            active_task_count: 1,
            runtime_assignment_count: 3,
            execution_count: 4,
            flags: vec!["high_cost_budget".to_owned()],
            refresh_min_absolute_increase: 10,
            refresh_min_multiplier: 10.0,
            refresh_min_severity_delta: 3,
            severity_weight_recommended_by: 1,
            severity_weight_active_tasks: 1,
            severity_weight_runtime_assignments: 2,
            severity_weight_executions: 2,
            severity_weight_severe_flags: 1,
            refresh_min_absolute_increase_source: "implementation".to_owned(),
            refresh_min_multiplier_source: "implementation".to_owned(),
            refresh_min_severity_delta_source: "implementation".to_owned(),
            severity_weight_recommended_by_source: "implementation".to_owned(),
            severity_weight_active_tasks_source: "implementation".to_owned(),
            severity_weight_runtime_assignments_source: "implementation".to_owned(),
            severity_weight_executions_source: "implementation".to_owned(),
            severity_weight_severe_flags_source: "implementation".to_owned(),
        }];

        let existing = vec![ArchitectureReviewRecord::new(
            "review-impl-hot-guardrail-hotspot".to_owned(),
            "guardrail hotspot review for impl-hot".to_owned(),
            "implementation_guardrail_hotspot".to_owned(),
            "system-guardrail".to_owned(),
            ReviewTargetPlane::Evolution,
            vec!["governance".to_owned()],
            false,
            true,
            false,
            true,
            false,
            ArchitectureReviewStatus::Open,
            ArchitectureReviewDecision::NeedsRedesign,
            "implementation impl-hot triggered 2 recent guardrail blocks for extreme_cost_budget while recommended_by=1 active_tasks=1 assignments=1 executions=1 severe_flags=1 refresh_min_absolute_increase=10 refresh_min_multiplier=10 refresh_min_severity_delta=3 severity_weight_recommended_by=1 severity_weight_active_tasks=1 severity_weight_runtime_assignments=2 severity_weight_executions=2 severity_weight_severe_flags=1".to_owned(),
            vec![],
            vec![],
            None,
        )];

        let suggestions = build_review_suggestions(&hotspots, &existing);

        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].suggestion_state, "worsened");
        assert!(suggestions[0].rationale.contains("assignments=3"));
        assert!(suggestions[0].rationale.contains("executions=4"));
        assert!(
            suggestions[0]
                .rationale
                .contains("severity_weight_runtime_assignments=2")
        );
    }

    #[test]
    fn collect_implementation_hotspots_uses_skill_governance_policy_defaults() {
        let root = unique_test_root();
        let compatibility = crate::registry::ImplementationCompatibility::new(
            "publish".to_owned(),
            "1.0.0".to_owned(),
            "1.0.0".to_owned(),
        );
        let implementation = crate::registry::ImplementationRecord::new(
            "impl-skill-policy".to_owned(),
            "xhs_publish".to_owned(),
            "worker_process".to_owned(),
            crate::registry::ImplementationEntry::new(
                "script".to_owned(),
                "scripts/impl-skill-policy.sh".to_owned(),
            ),
            compatibility,
        );
        persist_implementation(&root, &implementation)
            .expect("implementation should persist for skill policy test");
        let mut skill = SkillRecord::new(
            "xhs_publish".to_owned(),
            "XHS Publish".to_owned(),
            "publish to xhs".to_owned(),
            "impl-skill-policy".to_owned(),
            "system".to_owned(),
            "v1".to_owned(),
            Vec::new(),
            None,
        );
        skill.recommended_implementation_id = Some("impl-skill-policy".to_owned());
        skill.governance_policy.insert(
            "review_refresh_min_absolute_increase".to_owned(),
            "9".to_owned(),
        );
        skill.governance_policy.insert(
            "review_severity_weight_active_tasks".to_owned(),
            "5".to_owned(),
        );
        persist_skill(&root, &skill).expect("skill should persist for skill policy test");
        let spec = TaskSpec::new(
            "task-skill-policy".to_owned(),
            "tenant-a".to_owned(),
            "ns-a".to_owned(),
            "goal".to_owned(),
            Some("impl-skill-policy".to_owned()),
            vec!["xhs_publish".to_owned()],
            Vec::new(),
        );
        let runtime = TaskRuntime::queued("task-skill-policy".to_owned(), "queen-a".to_owned());
        persist_task_submission(&root, &spec, &runtime)
            .expect("task should persist for skill policy test");
        append_guardrail_block_audit(
            root.to_str().expect("root should be utf-8"),
            "governance_plan_guardrail_block",
            "governance_candidate",
            "impl-skill-policy",
            "implementation=impl-skill-policy skill_ref=xhs_publish reason=extreme_cost_budget"
                .to_owned(),
        )
        .expect("guardrail audit should persist for skill policy test");

        let hotspots = collect_implementation_hotspots(
            root.to_str().expect("root should be utf-8"),
            None,
            Some(30 * 24 * 60 * 60 * 1000),
            Some(5),
        )
        .expect("hotspots should collect");

        assert_eq!(hotspots.len(), 1);
        assert_eq!(hotspots[0].refresh_min_absolute_increase, 9);
        assert_eq!(hotspots[0].severity_weight_active_tasks, 5);

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn collect_implementation_hotspots_uses_global_governance_policy_defaults() {
        let root = unique_test_root();
        let compatibility = crate::registry::ImplementationCompatibility::new(
            "publish".to_owned(),
            "1.0.0".to_owned(),
            "1.0.0".to_owned(),
        );
        let implementation = crate::registry::ImplementationRecord::new(
            "impl-global-policy".to_owned(),
            "video_publish".to_owned(),
            "worker_process".to_owned(),
            crate::registry::ImplementationEntry::new(
                "script".to_owned(),
                "scripts/impl-global-policy.sh".to_owned(),
            ),
            compatibility,
        );
        persist_implementation(&root, &implementation)
            .expect("implementation should persist for global policy test");
        let mut skill = SkillRecord::new(
            "video_publish".to_owned(),
            "Video Publish".to_owned(),
            "publish video".to_owned(),
            "impl-global-policy".to_owned(),
            "system".to_owned(),
            "v1".to_owned(),
            Vec::new(),
            None,
        );
        skill.recommended_implementation_id = Some("impl-global-policy".to_owned());
        persist_skill(&root, &skill).expect("skill should persist for global policy test");

        let mut defaults = crate::registry::GovernanceDefaultsRecord::new();
        defaults.governance_policy.insert(
            "review_refresh_min_absolute_increase".to_owned(),
            "11".to_owned(),
        );
        defaults.governance_policy.insert(
            "review_severity_weight_active_tasks".to_owned(),
            "6".to_owned(),
        );
        crate::storage::persist_governance_defaults(&root, &defaults)
            .expect("global governance defaults should persist");

        let spec = TaskSpec::new(
            "task-global-policy".to_owned(),
            "tenant-a".to_owned(),
            "ns-a".to_owned(),
            "goal".to_owned(),
            Some("impl-global-policy".to_owned()),
            vec!["video_publish".to_owned()],
            Vec::new(),
        );
        let runtime = TaskRuntime::queued("task-global-policy".to_owned(), "queen-a".to_owned());
        persist_task_submission(&root, &spec, &runtime)
            .expect("task should persist for global policy test");
        append_guardrail_block_audit(
            root.to_str().expect("root should be utf-8"),
            "governance_plan_guardrail_block",
            "governance_candidate",
            "impl-global-policy",
            "implementation=impl-global-policy skill_ref=video_publish reason=extreme_latency_budget"
                .to_owned(),
        )
        .expect("guardrail audit should persist for global policy test");

        let hotspots = collect_implementation_hotspots(
            root.to_str().expect("root should be utf-8"),
            None,
            Some(30 * 24 * 60 * 60 * 1000),
            Some(5),
        )
        .expect("hotspots should collect");

        assert_eq!(hotspots.len(), 1);
        assert_eq!(hotspots[0].refresh_min_absolute_increase, 11);
        assert_eq!(hotspots[0].severity_weight_active_tasks, 6);

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn materialize_review_from_suggestion_builds_open_review_record() {
        let suggestion = ReviewSuggestionJson {
            suggested_review_id: "review-impl-hot-guardrail-hotspot".to_owned(),
            title: "guardrail hotspot review for impl-hot".to_owned(),
            change_scope: "implementation_guardrail_hotspot".to_owned(),
            target_plane: "evolution".to_owned(),
            target_modules: vec!["governance".to_owned(), "registry".to_owned()],
            rationale: "implementation impl-hot triggered hotspots".to_owned(),
            proposed_decision: "needs_redesign".to_owned(),
            required_followups: vec!["reassess implementation impl-hot".to_owned()],
            evidence_refs: vec!["implementation_hotspot:impl-hot".to_owned()],
            implementation_id: "impl-hot".to_owned(),
            skill_id: "xhs_publish".to_owned(),
            recent_guardrail_block_count: 4,
            recommended_by_skill_count: 1,
            active_task_count: 1,
            already_recorded: false,
            suggestion_state: "new".to_owned(),
            source_review_id: None,
        };

        let review = materialize_review_from_suggestion("system-guardrail", &suggestion, None);

        assert_eq!(review.review_id, "review-impl-hot-guardrail-hotspot");
        assert_eq!(review.status, ArchitectureReviewStatus::Open);
        assert_eq!(review.decision, ArchitectureReviewDecision::NeedsRedesign);
        assert_eq!(review.target_plane, ReviewTargetPlane::Evolution);
        assert!(review.writes_long_term);
        assert!(review.touches_registry);
        assert_eq!(review.requested_by, "system-guardrail");
    }

    #[test]
    fn reflection_record_persists_guardrail_snapshot() {
        let root = unique_test_root();
        append_guardrail_block_audit(
            root.to_str().expect("root should be utf-8"),
            "registry_sync_guardrail_block",
            "skill",
            "xhs_publish",
            "skill=xhs_publish skipped_reasons=extreme_cost_budget".to_owned(),
        )
        .expect("guardrail block audit should persist");

        let reflection = crate::governance::ArchitectureReflectionRecord::new(
            "arch-reflection-snapshot".to_owned(),
            "snapshot reflection".to_owned(),
            "2026-W13".to_owned(),
            "local-dev".to_owned(),
            ArchitectureReflectionDecision::DriftDetected,
            "summary".to_owned(),
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            Some(super::guardrail_snapshot_from_summary(
                summarize_guardrail_audits(
                    root.to_str().expect("root should be utf-8"),
                    None,
                    Some(30 * 24 * 60 * 60 * 1000),
                )
                .expect("summary should build"),
            )),
        );
        crate::storage::persist_architecture_reflection(&root, &reflection)
            .expect("reflection should persist");

        let (_, loaded) = load_architecture_reflection(&root, "arch-reflection-snapshot")
            .expect("reflection should load");
        assert_eq!(
            loaded
                .guardrail_snapshot
                .as_ref()
                .map(|snapshot| snapshot.total_count),
            Some(1)
        );

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn review_record_persists_guardrail_snapshot() {
        let root = unique_test_root();
        append_guardrail_block_audit(
            root.to_str().expect("root should be utf-8"),
            "governance_plan_guardrail_block",
            "governance_candidate",
            "impl-risky",
            "implementation=impl-risky skill_ref=xhs_publish reason=all matching governance candidates blocked by guardrails: extreme_cost_budget".to_owned(),
        )
        .expect("guardrail block audit should persist");

        let review = crate::governance::ArchitectureReviewRecord::new(
            "arch-review-snapshot".to_owned(),
            "snapshot review".to_owned(),
            "governance candidate".to_owned(),
            "local-dev".to_owned(),
            ReviewTargetPlane::Evolution,
            vec!["governance".to_owned()],
            false,
            true,
            false,
            true,
            false,
            ArchitectureReviewStatus::Completed,
            ArchitectureReviewDecision::PassWithFollowup,
            "summary".to_owned(),
            vec![],
            vec![],
            Some(super::guardrail_snapshot_from_summary(
                summarize_guardrail_audits(
                    root.to_str().expect("root should be utf-8"),
                    None,
                    Some(30 * 24 * 60 * 60 * 1000),
                )
                .expect("summary should build"),
            )),
        );
        crate::storage::persist_architecture_review(&root, &review).expect("review should persist");

        let (_, loaded) =
            load_architecture_review(&root, "arch-review-snapshot").expect("review should load");
        assert_eq!(
            loaded
                .guardrail_snapshot
                .as_ref()
                .map(|snapshot| snapshot.total_count),
            Some(1)
        );

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }
}

fn handle_implementation_list(args: &[String]) -> ExitCode {
    let skill_id = option_value(args, "--skill-id");
    let executor = option_value(args, "--executor");
    let root = option_value(args, "--root").unwrap_or(".");

    let (dir, implementations) = match list_implementations(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to list implementations: {error}");
            return ExitCode::from(1);
        }
    };

    let filtered = implementations
        .into_iter()
        .filter(|record| skill_id.is_none_or(|value| record.skill_id == value))
        .filter(|record| executor.is_none_or(|value| record.executor == value))
        .collect::<Vec<_>>();
    let (_, skills) = match list_skills(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load skills for implementation list: {error}");
            return ExitCode::from(1);
        }
    };
    let (_, tasks) = match list_task_submissions(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load tasks for implementation list: {error}");
            return ExitCode::from(1);
        }
    };
    let usage = summarize_implementation_usage(root, &skills, &tasks);
    let guardrails =
        match summarize_guardrail_implementation_audits(root, None, Some(30 * 24 * 60 * 60 * 1000))
        {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to summarize implementation guardrails: {error}");
                return ExitCode::from(1);
            }
        };

    println!("implementation list loaded");
    println!("  read_from: {}", dir.display());
    println!("  skill_id: {}", skill_id.unwrap_or("<none>"));
    println!("  executor: {}", executor.unwrap_or("<none>"));
    println!("  implementation_count: {}", filtered.len());
    for record in filtered {
        let governed = GovernedImplementation::from_record(&record);
        let flags = implementation_governance_flags(&governed);
        let implementation_usage = usage
            .get(&record.implementation_id)
            .cloned()
            .unwrap_or_default();
        let implementation_guardrail = guardrails
            .get(&record.implementation_id)
            .cloned()
            .unwrap_or_default();
        println!(
            "  - {} skill={} executor={} entry={} capability={} mode={} max_cost={} max_latency_ms={} guardrails={} top_reason={} recommended_by={} runtime_tasks={} active_tasks={} assignments={} executions={} flags={} origin={}",
            record.implementation_id,
            record.skill_id,
            record.executor,
            record.entry.path,
            record.compatibility.capability,
            governed.strategy_mode.as_deref().unwrap_or("<none>"),
            governed.max_cost.as_deref().unwrap_or("<none>"),
            governed.max_latency_ms.as_deref().unwrap_or("<none>"),
            implementation_guardrail.recent_guardrail_block_count,
            implementation_guardrail
                .top_reason
                .as_deref()
                .unwrap_or("<none>"),
            implementation_usage.recommended_by_skill_count,
            implementation_usage.runtime_task_count,
            implementation_usage.active_task_count,
            implementation_usage.runtime_assignment_count,
            implementation_usage.execution_count,
            if flags.is_empty() {
                "<none>".to_owned()
            } else {
                flags.join(", ")
            },
            record
                .origin
                .as_ref()
                .map(|origin| origin.source.as_str())
                .unwrap_or("<none>")
        );
    }

    ExitCode::SUCCESS
}

fn parse_architecture_reflection_decision(value: &str) -> Option<ArchitectureReflectionDecision> {
    match value {
        "no_major_drift" => Some(ArchitectureReflectionDecision::NoMajorDrift),
        "drift_detected" => Some(ArchitectureReflectionDecision::DriftDetected),
        _ => None,
    }
}

fn handle_reflection_record(args: &[String]) -> ExitCode {
    let reflection_id = option_value(args, "--reflection-id").unwrap_or("arch-reflection-demo");
    let title = option_value(args, "--title").unwrap_or("architecture reflection");
    let period_label = option_value(args, "--period-label").unwrap_or("unspecified-period");
    let recorded_by = option_value(args, "--recorded-by").unwrap_or("local-dev");
    let decision =
        match option_value(args, "--decision").and_then(parse_architecture_reflection_decision) {
            Some(value) => value,
            None => ArchitectureReflectionDecision::DriftDetected,
        };
    let summary = option_value(args, "--summary").unwrap_or("reflection recorded");
    let root = option_value(args, "--root").unwrap_or(".");
    let guardrail_snapshot =
        match summarize_guardrail_audits(root, None, Some(30 * 24 * 60 * 60 * 1000)) {
            Ok(value) => Some(guardrail_snapshot_from_summary(value)),
            Err(error) => {
                eprintln!("failed to summarize guardrail audits for reflection record: {error}");
                return ExitCode::from(1);
            }
        };
    let implementation_hotspots = match collect_implementation_hotspots(
        root,
        None,
        Some(30 * 24 * 60 * 60 * 1000),
        Some(3),
    ) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to collect implementation hotspots for reflection record: {error}");
            return ExitCode::from(1);
        }
    };
    let mut detected_drifts = option_values(args, "--drift");
    let mut freeze_actions = option_values(args, "--freeze-action");
    let mut next_actions = option_values(args, "--next-action");
    let mut evidence_refs = option_values(args, "--evidence-ref");
    enrich_reflection_inputs_from_hotspots(
        &mut detected_drifts,
        &mut freeze_actions,
        &mut next_actions,
        &mut evidence_refs,
        &implementation_hotspots,
    );

    let reflection = ArchitectureReflectionRecord::new(
        reflection_id.to_owned(),
        title.to_owned(),
        period_label.to_owned(),
        recorded_by.to_owned(),
        decision,
        summary.to_owned(),
        detected_drifts,
        freeze_actions,
        next_actions,
        option_values(args, "--review-ref"),
        evidence_refs,
        guardrail_snapshot,
    );

    let path = match persist_architecture_reflection(root, &reflection) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("failed to persist architecture reflection: {error}");
            return ExitCode::from(1);
        }
    };

    println!("reflection record completed");
    println!("  reflection_id: {}", reflection.reflection_id);
    println!("  title: {}", reflection.title);
    println!("  period_label: {}", reflection.period_label);
    println!("  decision: {}", reflection.decision.as_str());
    println!("  summary: {}", reflection.summary);
    println!(
        "  detected_drifts: {}",
        joined_or_none(&reflection.detected_drifts)
    );
    println!(
        "  freeze_actions: {}",
        joined_or_none(&reflection.freeze_actions)
    );
    println!(
        "  next_actions: {}",
        joined_or_none(&reflection.next_actions)
    );
    println!("  review_refs: {}", joined_or_none(&reflection.review_refs));
    println!(
        "  evidence_refs: {}",
        joined_or_none(&reflection.evidence_refs)
    );
    println!(
        "  guardrail_snapshot_count: {}",
        reflection
            .guardrail_snapshot
            .as_ref()
            .map(|snapshot| snapshot.total_count)
            .unwrap_or(0)
    );
    println!("  written_to: {}", path.display());

    ExitCode::SUCCESS
}

fn handle_reflection_inspect(args: &[String]) -> ExitCode {
    let reflection_id = option_value(args, "--reflection-id").unwrap_or("arch-reflection-demo");
    let root = option_value(args, "--root").unwrap_or(".");

    let (path, reflection) = match load_architecture_reflection(root, reflection_id) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to inspect architecture reflection: {error}");
            return ExitCode::from(1);
        }
    };

    println!("reflection inspect loaded");
    println!("  reflection_id: {}", reflection.reflection_id);
    println!("  title: {}", reflection.title);
    println!("  period_label: {}", reflection.period_label);
    println!("  recorded_by: {}", reflection.recorded_by);
    println!("  decision: {}", reflection.decision.as_str());
    println!("  summary: {}", reflection.summary);
    println!(
        "  detected_drifts: {}",
        joined_or_none(&reflection.detected_drifts)
    );
    println!(
        "  freeze_actions: {}",
        joined_or_none(&reflection.freeze_actions)
    );
    println!(
        "  next_actions: {}",
        joined_or_none(&reflection.next_actions)
    );
    println!("  review_refs: {}", joined_or_none(&reflection.review_refs));
    println!(
        "  evidence_refs: {}",
        joined_or_none(&reflection.evidence_refs)
    );
    println!("  created_at: {}", reflection.created_at);
    println!("  updated_at: {}", reflection.updated_at);
    let guardrail_snapshot = if let Some(snapshot) = reflection.guardrail_snapshot.as_ref() {
        snapshot.clone()
    } else {
        match summarize_guardrail_audits(
            root,
            Some(&reflection.created_at),
            Some(30 * 24 * 60 * 60 * 1000),
        ) {
            Ok(value) => guardrail_snapshot_from_summary(value),
            Err(error) => {
                eprintln!("failed to summarize guardrail audits for reflection inspect: {error}");
                return ExitCode::from(1);
            }
        }
    };
    println!(
        "  recent_guardrail_block_count: {}",
        guardrail_snapshot.total_count
    );
    println!(
        "  recent_guardrail_window: {}",
        guardrail_snapshot.window_label
    );
    for row in &guardrail_snapshot.action_counts {
        println!(
            "  recent_guardrail_action: action={} count={}",
            row.label, row.count
        );
    }
    for row in guardrail_snapshot.target_type_counts.iter().take(5) {
        println!(
            "  recent_guardrail_target_type: target_type={} count={}",
            row.label, row.count
        );
    }
    for row in guardrail_snapshot.target_id_counts.iter().take(5) {
        println!(
            "  recent_guardrail_target: target={} count={}",
            row.label, row.count
        );
    }
    for row in guardrail_snapshot.skill_counts.iter().take(5) {
        println!(
            "  recent_guardrail_skill: skill={} count={}",
            row.label, row.count
        );
    }
    for row in guardrail_snapshot.reason_counts.iter().take(5) {
        println!(
            "  recent_guardrail_reason: reason={} count={}",
            row.label, row.count
        );
    }
    println!("  loaded_from: {}", path.display());

    ExitCode::SUCCESS
}

fn handle_reflection_list(args: &[String]) -> ExitCode {
    let decision_filter = option_value(args, "--decision");
    let root = option_value(args, "--root").unwrap_or(".");

    let (dir, mut reflections) = match list_architecture_reflections(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to list architecture reflections: {error}");
            return ExitCode::from(1);
        }
    };

    if let Some(decision) = decision_filter {
        reflections.retain(|reflection| reflection.decision.as_str() == decision);
    }

    println!("reflection list loaded");
    println!("  reflection_dir: {}", dir.display());
    println!("  decision_filter: {}", decision_filter.unwrap_or("<none>"));
    println!("  count: {}", reflections.len());
    for reflection in reflections {
        let guardrail_snapshot = if let Some(snapshot) = reflection.guardrail_snapshot.as_ref() {
            snapshot.clone()
        } else {
            match summarize_guardrail_audits(
                root,
                Some(&reflection.created_at),
                Some(30 * 24 * 60 * 60 * 1000),
            ) {
                Ok(value) => guardrail_snapshot_from_summary(value),
                Err(error) => {
                    eprintln!("failed to summarize guardrail audits for reflection list: {error}");
                    return ExitCode::from(1);
                }
            }
        };
        println!(
            "  reflection={} period={} decision={} drifts={} recent_guardrail_blocks={}",
            reflection.reflection_id,
            reflection.period_label,
            reflection.decision.as_str(),
            reflection.detected_drifts.len(),
            guardrail_snapshot.total_count
        );
    }

    ExitCode::SUCCESS
}

fn parse_review_target_plane(value: &str) -> Option<ReviewTargetPlane> {
    match value {
        "execution" => Some(ReviewTargetPlane::Execution),
        "evolution" => Some(ReviewTargetPlane::Evolution),
        "cross_plane" => Some(ReviewTargetPlane::CrossPlane),
        _ => None,
    }
}

fn parse_architecture_review_status(value: &str) -> Option<ArchitectureReviewStatus> {
    match value {
        "open" => Some(ArchitectureReviewStatus::Open),
        "completed" => Some(ArchitectureReviewStatus::Completed),
        _ => None,
    }
}

fn parse_architecture_review_decision(value: &str) -> Option<ArchitectureReviewDecision> {
    match value {
        "pass" => Some(ArchitectureReviewDecision::Pass),
        "pass_with_followup" => Some(ArchitectureReviewDecision::PassWithFollowup),
        "needs_redesign" => Some(ArchitectureReviewDecision::NeedsRedesign),
        "blocked" => Some(ArchitectureReviewDecision::Blocked),
        _ => None,
    }
}

fn handle_review_record(args: &[String]) -> ExitCode {
    let review_id = option_value(args, "--review-id").unwrap_or("arch-review-demo");
    let title = option_value(args, "--title").unwrap_or("architecture review");
    let change_scope = option_value(args, "--change-scope").unwrap_or("unspecified");
    let requested_by = option_value(args, "--requested-by").unwrap_or("local-dev");
    let target_plane =
        match option_value(args, "--target-plane").and_then(parse_review_target_plane) {
            Some(value) => value,
            None => ReviewTargetPlane::Execution,
        };
    let status = match option_value(args, "--status").and_then(parse_architecture_review_status) {
        Some(value) => value,
        None => ArchitectureReviewStatus::Completed,
    };
    let decision =
        match option_value(args, "--decision").and_then(parse_architecture_review_decision) {
            Some(value) => value,
            None => ArchitectureReviewDecision::PassWithFollowup,
        };
    let rationale = option_value(args, "--rationale").unwrap_or("review recorded");
    let root = option_value(args, "--root").unwrap_or(".");
    let guardrail_snapshot =
        match summarize_guardrail_audits(root, None, Some(30 * 24 * 60 * 60 * 1000)) {
            Ok(value) => Some(guardrail_snapshot_from_summary(value)),
            Err(error) => {
                eprintln!("failed to summarize guardrail audits for review record: {error}");
                return ExitCode::from(1);
            }
        };
    let implementation_hotspots = match collect_implementation_hotspots(
        root,
        None,
        Some(30 * 24 * 60 * 60 * 1000),
        Some(3),
    ) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to collect implementation hotspots for review record: {error}");
            return ExitCode::from(1);
        }
    };
    let mut required_followups = option_values(args, "--followup");
    let mut evidence_refs = option_values(args, "--evidence-ref");
    enrich_review_inputs_from_hotspots(
        &mut required_followups,
        &mut evidence_refs,
        &implementation_hotspots,
    );

    let review = ArchitectureReviewRecord::new(
        review_id.to_owned(),
        title.to_owned(),
        change_scope.to_owned(),
        requested_by.to_owned(),
        target_plane,
        option_values(args, "--target-module"),
        args.iter().any(|arg| arg == "--writes-runtime"),
        args.iter().any(|arg| arg == "--writes-long-term"),
        args.iter().any(|arg| arg == "--mutates-historical-facts"),
        args.iter().any(|arg| arg == "--touches-registry"),
        args.iter().any(|arg| arg == "--touches-approval-or-policy"),
        status,
        decision,
        rationale.to_owned(),
        required_followups,
        evidence_refs,
        guardrail_snapshot,
    );

    let path = match persist_architecture_review(root, &review) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("failed to persist architecture review: {error}");
            return ExitCode::from(1);
        }
    };

    println!("review record completed");
    println!("  review_id: {}", review.review_id);
    println!("  title: {}", review.title);
    println!("  target_plane: {}", review.target_plane.as_str());
    println!("  status: {}", review.status.as_str());
    println!("  decision: {}", review.decision.as_str());
    println!(
        "  target_modules: {}",
        joined_or_none(&review.target_modules)
    );
    println!(
        "  writes_runtime: {}",
        if review.writes_runtime {
            "true"
        } else {
            "false"
        }
    );
    println!(
        "  writes_long_term: {}",
        if review.writes_long_term {
            "true"
        } else {
            "false"
        }
    );
    println!(
        "  mutates_historical_facts: {}",
        if review.mutates_historical_facts {
            "true"
        } else {
            "false"
        }
    );
    println!("  rationale: {}", review.rationale);
    println!(
        "  required_followups: {}",
        joined_or_none(&review.required_followups)
    );
    println!("  evidence_refs: {}", joined_or_none(&review.evidence_refs));
    println!(
        "  guardrail_snapshot_count: {}",
        review
            .guardrail_snapshot
            .as_ref()
            .map(|snapshot| snapshot.total_count)
            .unwrap_or(0)
    );
    println!("  written_to: {}", path.display());

    ExitCode::SUCCESS
}

fn handle_review_suggest(args: &[String]) -> ExitCode {
    let root = option_value(args, "--root").unwrap_or(".");
    let limit = option_value(args, "--limit")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(5);
    let as_json = args.iter().any(|arg| arg == "--json");

    let hotspots = match collect_implementation_hotspots(
        root,
        None,
        Some(30 * 24 * 60 * 60 * 1000),
        Some(limit),
    ) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to collect review suggestions: {error}");
            return ExitCode::from(1);
        }
    };
    let (_, reviews) = match list_architecture_reviews(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load existing reviews for suggestions: {error}");
            return ExitCode::from(1);
        }
    };
    let suggestions = build_review_suggestions(&hotspots, &reviews);

    if as_json {
        match serde_json::to_string_pretty(&suggestions) {
            Ok(body) => {
                println!("{body}");
                return ExitCode::SUCCESS;
            }
            Err(error) => {
                eprintln!("failed to serialize review suggestions: {error}");
                return ExitCode::from(1);
            }
        }
    }

    println!("review suggest loaded");
    println!("  root: {root}");
    println!("  suggestion_count: {}", suggestions.len());
    for suggestion in suggestions {
        println!(
            "  suggestion={} state={} source_review={} implementation={} skill={} decision={} guardrails={} recommended_by={} active_tasks={} already_recorded={}",
            suggestion.suggested_review_id,
            suggestion.suggestion_state,
            suggestion.source_review_id.as_deref().unwrap_or("<none>"),
            suggestion.implementation_id,
            suggestion.skill_id,
            suggestion.proposed_decision,
            suggestion.recent_guardrail_block_count,
            suggestion.recommended_by_skill_count,
            suggestion.active_task_count,
            if suggestion.already_recorded {
                "true"
            } else {
                "false"
            }
        );
        println!("    title={}", suggestion.title);
        println!("    rationale={}", suggestion.rationale);
        println!(
            "    followups={}",
            joined_or_none(&suggestion.required_followups)
        );
        println!("    evidence={}", joined_or_none(&suggestion.evidence_refs));
    }

    ExitCode::SUCCESS
}

fn handle_review_materialize(args: &[String]) -> ExitCode {
    let root = option_value(args, "--root").unwrap_or(".");
    let limit = option_value(args, "--limit")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(5);
    let requested_by = option_value(args, "--requested-by").unwrap_or("system-guardrail");
    let guardrail_snapshot =
        match summarize_guardrail_audits(root, None, Some(30 * 24 * 60 * 60 * 1000)) {
            Ok(value) => Some(guardrail_snapshot_from_summary(value)),
            Err(error) => {
                eprintln!("failed to summarize guardrail audits for review materialize: {error}");
                return ExitCode::from(1);
            }
        };

    let hotspots = match collect_implementation_hotspots(
        root,
        None,
        Some(30 * 24 * 60 * 60 * 1000),
        Some(limit),
    ) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to collect review materialization candidates: {error}");
            return ExitCode::from(1);
        }
    };
    let (_, reviews) = match list_architecture_reviews(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load existing reviews for materialization: {error}");
            return ExitCode::from(1);
        }
    };
    let suggestions = build_review_suggestions(&hotspots, &reviews);
    let mut materialized_count = 0usize;
    let mut skipped_existing_count = 0usize;

    println!("review materialize completed");
    println!("  root: {root}");
    println!("  requested_by: {requested_by}");
    println!("  candidate_count: {}", suggestions.len());
    for suggestion in suggestions {
        if suggestion.already_recorded {
            skipped_existing_count += 1;
            println!(
                "  skipped_existing_review={} implementation={}",
                suggestion.suggested_review_id, suggestion.implementation_id
            );
            continue;
        }
        let review = materialize_review_from_suggestion(
            requested_by,
            &suggestion,
            guardrail_snapshot.clone(),
        );
        let path = match persist_architecture_review(root, &review) {
            Ok(path) => path,
            Err(error) => {
                eprintln!(
                    "failed to persist materialized review {}: {error}",
                    review.review_id
                );
                return ExitCode::from(1);
            }
        };
        materialized_count += 1;
        println!(
            "  review={} decision={} implementation={} written_to={}",
            review.review_id,
            review.decision.as_str(),
            suggestion.implementation_id,
            path.display()
        );
    }
    println!("  materialized_count: {}", materialized_count);
    println!("  skipped_existing_count: {}", skipped_existing_count);

    ExitCode::SUCCESS
}

fn handle_review_inspect(args: &[String]) -> ExitCode {
    let review_id = option_value(args, "--review-id").unwrap_or("arch-review-demo");
    let root = option_value(args, "--root").unwrap_or(".");

    let (path, review) = match load_architecture_review(root, review_id) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to inspect architecture review: {error}");
            return ExitCode::from(1);
        }
    };

    println!("review inspect loaded");
    println!("  review_id: {}", review.review_id);
    println!("  title: {}", review.title);
    println!("  change_scope: {}", review.change_scope);
    println!("  requested_by: {}", review.requested_by);
    println!("  target_plane: {}", review.target_plane.as_str());
    println!("  status: {}", review.status.as_str());
    println!("  decision: {}", review.decision.as_str());
    println!(
        "  target_modules: {}",
        joined_or_none(&review.target_modules)
    );
    println!(
        "  writes_runtime: {}",
        if review.writes_runtime {
            "true"
        } else {
            "false"
        }
    );
    println!(
        "  writes_long_term: {}",
        if review.writes_long_term {
            "true"
        } else {
            "false"
        }
    );
    println!(
        "  mutates_historical_facts: {}",
        if review.mutates_historical_facts {
            "true"
        } else {
            "false"
        }
    );
    println!(
        "  touches_registry: {}",
        if review.touches_registry {
            "true"
        } else {
            "false"
        }
    );
    println!(
        "  touches_approval_or_policy: {}",
        if review.touches_approval_or_policy {
            "true"
        } else {
            "false"
        }
    );
    println!("  rationale: {}", review.rationale);
    println!(
        "  required_followups: {}",
        joined_or_none(&review.required_followups)
    );
    println!("  evidence_refs: {}", joined_or_none(&review.evidence_refs));
    let guardrail_snapshot = if let Some(snapshot) = review.guardrail_snapshot.as_ref() {
        snapshot.clone()
    } else {
        match summarize_guardrail_audits(
            root,
            Some(&review.created_at),
            Some(30 * 24 * 60 * 60 * 1000),
        ) {
            Ok(value) => guardrail_snapshot_from_summary(value),
            Err(error) => {
                eprintln!("failed to summarize guardrail audits for review inspect: {error}");
                return ExitCode::from(1);
            }
        }
    };
    println!(
        "  recent_guardrail_block_count: {}",
        guardrail_snapshot.total_count
    );
    for row in &guardrail_snapshot.action_counts {
        println!(
            "  recent_guardrail_action: action={} count={}",
            row.label, row.count
        );
    }
    for row in guardrail_snapshot.skill_counts.iter().take(5) {
        println!(
            "  recent_guardrail_skill: skill={} count={}",
            row.label, row.count
        );
    }
    for row in guardrail_snapshot.reason_counts.iter().take(5) {
        println!(
            "  recent_guardrail_reason: reason={} count={}",
            row.label, row.count
        );
    }
    println!("  created_at: {}", review.created_at);
    println!("  updated_at: {}", review.updated_at);
    println!("  loaded_from: {}", path.display());

    ExitCode::SUCCESS
}

fn handle_review_list(args: &[String]) -> ExitCode {
    let decision_filter = option_value(args, "--decision");
    let status_filter = option_value(args, "--status");
    let root = option_value(args, "--root").unwrap_or(".");

    let (dir, mut reviews) = match list_architecture_reviews(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to list architecture reviews: {error}");
            return ExitCode::from(1);
        }
    };

    if let Some(decision) = decision_filter {
        reviews.retain(|review| review.decision.as_str() == decision);
    }
    if let Some(status) = status_filter {
        reviews.retain(|review| review.status.as_str() == status);
    }

    println!("review list loaded");
    println!("  review_dir: {}", dir.display());
    println!("  decision_filter: {}", decision_filter.unwrap_or("<none>"));
    println!("  status_filter: {}", status_filter.unwrap_or("<none>"));
    println!("  count: {}", reviews.len());
    for review in reviews {
        let guardrail_snapshot = if let Some(snapshot) = review.guardrail_snapshot.as_ref() {
            snapshot.clone()
        } else {
            match summarize_guardrail_audits(
                root,
                Some(&review.created_at),
                Some(30 * 24 * 60 * 60 * 1000),
            ) {
                Ok(value) => guardrail_snapshot_from_summary(value),
                Err(error) => {
                    eprintln!("failed to summarize guardrail audits for review list: {error}");
                    return ExitCode::from(1);
                }
            }
        };
        println!(
            "  review={} plane={} status={} decision={} scope={} recent_guardrail_blocks={}",
            review.review_id,
            review.target_plane.as_str(),
            review.status.as_str(),
            review.decision.as_str(),
            review.change_scope,
            guardrail_snapshot.total_count
        );
    }

    ExitCode::SUCCESS
}

fn handle_fitness_run(args: &[String]) -> ExitCode {
    let implementation_id = option_value(args, "--implementation").unwrap_or("impl-demo");
    let score = option_value(args, "--score").unwrap_or("0.80");
    let summary = option_value(args, "--summary").unwrap_or("initial fitness report");
    let skill_refs = option_values(args, "--skill-ref");
    let tool_refs = option_values(args, "--tool-ref");
    let root = option_value(args, "--root").unwrap_or(".");

    if let Err(error) = validate_registry_refs(root, &skill_refs, &tool_refs) {
        eprintln!("failed to validate fitness capability refs: {error}");
        return ExitCode::from(1);
    }
    let (_, implementation_record) = match load_implementation(root, implementation_id) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load implementation for fitness run: {error}");
            return ExitCode::from(1);
        }
    };
    let (_, implementation_skill) = match load_skill(root, &implementation_record.skill_id) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load implementation skill for fitness run: {error}");
            return ExitCode::from(1);
        }
    };
    if let Err(error) = validate_skill_implementation_refs(root, &implementation_skill) {
        eprintln!("failed to validate implementation skill for fitness run: {error}");
        return ExitCode::from(1);
    }
    let implementation = GovernedImplementation::from_record(&implementation_record);
    let mut skill_refs = skill_refs;
    if !skill_refs
        .iter()
        .any(|skill| skill == &implementation.skill_id)
    {
        skill_refs.insert(0, implementation.skill_id.clone());
    }

    let report = FitnessReport::new(
        implementation.clone(),
        score.to_owned(),
        summary.to_owned(),
        skill_refs,
        tool_refs,
    );
    let plan = EvolutionPlan::observe(
        implementation,
        "default to observe until governance thresholds are wired".to_owned(),
    );

    let output_path = match persist_fitness_run(root, &report, &plan) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("failed to persist fitness run: {error}");
            return ExitCode::from(1);
        }
    };

    println!("fitness run completed");
    println!("  implementation_id: {}", report.implementation_id());
    println!(
        "  implementation_skill_id: {}",
        report.implementation.skill_id
    );
    println!(
        "  implementation_executor: {}",
        report.implementation.executor
    );
    println!(
        "  implementation_mode: {}",
        report
            .implementation
            .strategy_mode
            .as_deref()
            .unwrap_or("<none>")
    );
    println!(
        "  implementation_prompt: {}",
        report
            .implementation
            .prompt_component
            .as_deref()
            .unwrap_or("<none>")
    );
    println!(
        "  implementation_max_cost: {}",
        report
            .implementation
            .max_cost
            .as_deref()
            .unwrap_or("<none>")
    );
    println!(
        "  implementation_max_latency_ms: {}",
        report
            .implementation
            .max_latency_ms
            .as_deref()
            .unwrap_or("<none>")
    );
    println!("  score: {}", report.score);
    println!("  summary: {}", report.summary);
    println!("  skill_refs: {}", joined_or_none(&report.skill_refs));
    println!("  tool_refs: {}", joined_or_none(&report.tool_refs));
    println!("  decision: {}", plan.decision.as_str());
    println!("  rationale: {}", plan.rationale);
    println!("  written_to: {}", output_path.display());

    ExitCode::SUCCESS
}

fn handle_fitness_explain(args: &[String]) -> ExitCode {
    let implementation_id = option_value(args, "--implementation").unwrap_or("impl-demo");
    let with_runtime = args.iter().any(|arg| arg == "--with-runtime");
    let root = option_value(args, "--root").unwrap_or(".");

    let (path, record) = match load_fitness_run(root, implementation_id) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to explain fitness run: {error}");
            return ExitCode::from(1);
        }
    };

    println!("fitness explain loaded");
    println!("  schema_version: {}", record.schema_version);
    println!(
        "  implementation_id: {}",
        record.fitness_report.implementation_id()
    );
    println!(
        "  implementation_skill_id: {}",
        record.fitness_report.implementation.skill_id
    );
    println!(
        "  implementation_executor: {}",
        record.fitness_report.implementation.executor
    );
    println!(
        "  implementation_mode: {}",
        record
            .fitness_report
            .implementation
            .strategy_mode
            .as_deref()
            .unwrap_or("<none>")
    );
    println!(
        "  implementation_prompt: {}",
        record
            .fitness_report
            .implementation
            .prompt_component
            .as_deref()
            .unwrap_or("<none>")
    );
    println!(
        "  implementation_max_cost: {}",
        record
            .fitness_report
            .implementation
            .max_cost
            .as_deref()
            .unwrap_or("<none>")
    );
    println!(
        "  implementation_max_latency_ms: {}",
        record
            .fitness_report
            .implementation
            .max_latency_ms
            .as_deref()
            .unwrap_or("<none>")
    );
    println!("  score: {}", record.fitness_report.score);
    println!("  summary: {}", record.fitness_report.summary);
    println!(
        "  skill_refs: {}",
        joined_or_none(&record.fitness_report.skill_refs)
    );
    println!(
        "  tool_refs: {}",
        joined_or_none(&record.fitness_report.tool_refs)
    );
    println!("  decision: {}", record.evolution_plan.decision.as_str());
    println!("  rationale: {}", record.evolution_plan.rationale);
    println!("  read_from: {}", path.display());
    if with_runtime {
        if let Err(error) = print_runtime_usage(root, record.fitness_report.implementation_id()) {
            eprintln!("failed to load runtime usage for fitness explain: {error}");
            return ExitCode::from(1);
        }
    }

    ExitCode::SUCCESS
}

fn handle_evolution_audit_tail(args: &[String]) -> ExitCode {
    let root = option_value(args, "--root").unwrap_or(".");

    let (path, audits) = match load_evolution_audits(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to read evolution audit: {error}");
            return ExitCode::from(1);
        }
    };

    println!("evolution audit loaded");
    println!("  read_from: {}", path.display());
    println!("  audit_count: {}", audits.len());
    for audit in audits {
        println!(
            "  - [{}] {} {} {} -> {} ({}) detail={}",
            audit.timestamp,
            audit.actor_type,
            audit.actor_id,
            audit.action,
            audit.target_id,
            audit.result,
            audit.payload
        );
    }

    ExitCode::SUCCESS
}

fn handle_lineage_show(args: &[String]) -> ExitCode {
    let root = option_value(args, "--root").unwrap_or(".");
    let skill_ref = option_value(args, "--skill-ref");
    let tool_ref = option_value(args, "--tool-ref");
    let with_runtime = args.iter().any(|arg| arg == "--with-runtime");

    let (dir, records) = match list_fitness_runs(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to list fitness lineage: {error}");
            return ExitCode::from(1);
        }
    };

    let filtered = records
        .into_iter()
        .filter(|record| {
            let skill_match = skill_ref.is_none_or(|value| {
                record
                    .fitness_report
                    .skill_refs
                    .iter()
                    .any(|skill| skill == value)
            });
            let tool_match = tool_ref.is_none_or(|value| {
                record
                    .fitness_report
                    .tool_refs
                    .iter()
                    .any(|tool| tool == value)
            });
            skill_match && tool_match
        })
        .collect::<Vec<_>>();

    println!("lineage show loaded");
    println!("  read_from: {}", dir.display());
    println!("  skill_ref: {}", skill_ref.unwrap_or("<none>"));
    println!("  tool_ref: {}", tool_ref.unwrap_or("<none>"));
    println!("  record_count: {}", filtered.len());
    for record in filtered {
        println!(
            "  - {} score={} decision={} executor={} mode={} max_cost={} max_latency_ms={} skills={} tools={}",
            record.fitness_report.implementation_id(),
            record.fitness_report.score,
            record.evolution_plan.decision.as_str(),
            record.fitness_report.implementation.executor,
            record
                .fitness_report
                .implementation
                .strategy_mode
                .as_deref()
                .unwrap_or("<none>"),
            record
                .fitness_report
                .implementation
                .max_cost
                .as_deref()
                .unwrap_or("<none>"),
            record
                .fitness_report
                .implementation
                .max_latency_ms
                .as_deref()
                .unwrap_or("<none>"),
            joined_or_none(&record.fitness_report.skill_refs),
            joined_or_none(&record.fitness_report.tool_refs)
        );
        if with_runtime {
            if let Err(error) = print_runtime_usage(root, record.fitness_report.implementation_id())
            {
                eprintln!("failed to load runtime usage for lineage show: {error}");
                return ExitCode::from(1);
            }
        }
    }

    ExitCode::SUCCESS
}

fn parse_score(score: &str) -> f64 {
    score.parse::<f64>().unwrap_or(0.0)
}

fn parse_constraint_f64(value: Option<&str>) -> Option<f64> {
    value.and_then(|raw| raw.parse::<f64>().ok())
}

fn extract_guardrail_field(payload: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    payload
        .split_whitespace()
        .find_map(|segment| segment.strip_prefix(&prefix))
        .map(|value| value.trim_end_matches(',').trim())
        .filter(|value| !value.is_empty() && *value != "<none>")
        .map(str::to_owned)
}

fn extract_guardrail_reason(payload: &str) -> String {
    payload
        .split("reason=")
        .nth(1)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(payload)
        .to_owned()
}

fn extract_guardrail_skill(payload: &str) -> Option<String> {
    for key in ["skill", "skill_ref"] {
        if let Some(value) = extract_guardrail_field(payload, key) {
            return Some(value);
        }
    }
    None
}

fn extract_guardrail_implementation(payload: &str) -> Option<String> {
    extract_guardrail_field(payload, "implementation")
}

fn summarize_implementation_usage(
    root: &str,
    skills: &[SkillRecord],
    tasks: &[TaskRecord],
) -> std::collections::BTreeMap<String, ImplementationUsageSummary> {
    let mut usage = std::collections::BTreeMap::<String, ImplementationUsageSummary>::new();
    for skill in skills {
        if let Some(implementation_id) = &skill.recommended_implementation_id {
            usage
                .entry(implementation_id.clone())
                .or_default()
                .recommended_by_skill_count += 1;
        }
    }
    for task in tasks {
        let Some(implementation_id) = runtime_task_implementation_id(task) else {
            continue;
        };
        let entry = usage.entry(implementation_id.to_owned()).or_default();
        entry.runtime_task_count += 1;
        if task.task_runtime.status.as_str() != "completed" {
            entry.active_task_count += 1;
        }

        if let Ok((_, assignments)) = load_task_assignments(root, &task.task_spec.task_id) {
            for assignment in assignments {
                let Some(assignment_implementation_id) =
                    runtime_assignment_implementation_id(&assignment)
                else {
                    continue;
                };
                usage.entry(assignment_implementation_id.to_owned())
                    .or_default()
                    .runtime_assignment_count += 1;
            }
        }
    }

    if let Ok((_, execution_records)) = list_execution_records(root) {
        for record in execution_records {
            let Some(execution_implementation_id) = record
                .implementation_snapshot
                .as_ref()
                .map(|snapshot| snapshot.implementation_id.as_str())
                .or(record.implementation_ref.as_deref())
            else {
                continue;
            };
            usage.entry(execution_implementation_id.to_owned())
                .or_default()
                .execution_count += 1;
        }
    }
    usage
}

fn summarize_guardrail_implementation_audits(
    root: &str,
    end_timestamp: Option<&str>,
    window_ms: Option<u128>,
) -> std::io::Result<std::collections::BTreeMap<String, ImplementationGuardrailSummary>> {
    let (_, audits) = load_evolution_audits(root)?;
    let end_ms = end_timestamp.and_then(crate::core::parse_unix_ms_timestamp);
    let start_ms = match (end_ms, window_ms) {
        (Some(end), Some(window)) => Some(end.saturating_sub(window)),
        _ => None,
    };
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    let mut reasons =
        std::collections::BTreeMap::<String, std::collections::BTreeMap<String, usize>>::new();

    for audit in audits {
        if audit.result != "guardrail_blocked" {
            continue;
        }
        let audit_ms = crate::core::parse_unix_ms_timestamp(&audit.timestamp);
        if let Some(end) = end_ms {
            if audit_ms.is_some_and(|value| value > end) {
                continue;
            }
        }
        if let Some(start) = start_ms {
            if audit_ms.is_some_and(|value| value < start) {
                continue;
            }
        }
        let Some(implementation_id) = extract_guardrail_implementation(&audit.payload) else {
            continue;
        };
        *counts.entry(implementation_id.clone()).or_insert(0) += 1;
        *reasons
            .entry(implementation_id)
            .or_default()
            .entry(extract_guardrail_reason(&audit.payload))
            .or_insert(0) += 1;
    }

    let mut summaries = std::collections::BTreeMap::<String, ImplementationGuardrailSummary>::new();
    for (implementation_id, recent_guardrail_block_count) in counts {
        let top_reason = reasons
            .remove(&implementation_id)
            .and_then(|reason_counts| {
                reason_counts
                    .into_iter()
                    .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
                    .map(|(reason, _)| reason)
            });
        summaries.insert(
            implementation_id,
            ImplementationGuardrailSummary {
                recent_guardrail_block_count,
                top_reason,
            },
        );
    }
    Ok(summaries)
}

fn collect_implementation_hotspots(
    root: &str,
    end_timestamp: Option<&str>,
    window_ms: Option<u128>,
    limit: Option<usize>,
) -> std::io::Result<Vec<RegistryOverviewImplementationHotspotJson>> {
    let (_, implementations) = list_implementations(root)?;
    let (_, skills) = list_skills(root)?;
    let (_, tasks) = list_task_submissions(root)?;
    let global_policy = load_governance_defaults(root)
        .ok()
        .map(|(_, defaults)| defaults.governance_policy)
        .unwrap_or_default();
    let skill_policy = skills
        .iter()
        .map(|skill| (skill.skill_id.clone(), skill.governance_policy.clone()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let implementation_context = summarize_implementation_usage(root, &skills, &tasks);
    let implementation_guardrails =
        summarize_guardrail_implementation_audits(root, end_timestamp, window_ms)?;

    let mut implementation_hotspot_rows = implementations
        .iter()
        .filter_map(|record| {
            let usage = implementation_context
                .get(&record.implementation_id)
                .cloned()
                .unwrap_or_default();
            let guardrail = implementation_guardrails
                .get(&record.implementation_id)
                .cloned()
                .unwrap_or_default();
            if guardrail.recent_guardrail_block_count == 0 {
                return None;
            }
            if usage.recommended_by_skill_count == 0
                && usage.active_task_count == 0
                && usage.runtime_assignment_count == 0
                && usage.execution_count == 0
            {
                return None;
            }
            let governed = GovernedImplementation::from_record(record);
            let skill_policy = skill_policy.get(&record.skill_id);
            let (refresh_min_absolute_increase, refresh_min_absolute_increase_source) =
                resolve_usize_setting_with_source(
                    &record.constraints,
                    skill_policy,
                    Some(&global_policy),
                    "review_refresh_min_absolute_increase",
                    3,
                );
            let (refresh_min_multiplier, refresh_min_multiplier_source) =
                resolve_f64_setting_with_source(
                    &record.constraints,
                    skill_policy,
                    Some(&global_policy),
                    "review_refresh_min_multiplier",
                    2.0,
                );
            let (refresh_min_severity_delta, refresh_min_severity_delta_source) =
                resolve_usize_setting_with_source(
                    &record.constraints,
                    skill_policy,
                    Some(&global_policy),
                    "review_refresh_min_severity_delta",
                    3,
                );
            let (severity_weight_recommended_by, severity_weight_recommended_by_source) =
                resolve_usize_setting_with_source(
                    &record.constraints,
                    skill_policy,
                    Some(&global_policy),
                    "review_severity_weight_recommended_by",
                    2,
                );
            let (severity_weight_active_tasks, severity_weight_active_tasks_source) =
                resolve_usize_setting_with_source(
                    &record.constraints,
                    skill_policy,
                    Some(&global_policy),
                    "review_severity_weight_active_tasks",
                    2,
                );
            let (severity_weight_runtime_assignments, severity_weight_runtime_assignments_source) =
                resolve_usize_setting_with_source(
                    &record.constraints,
                    skill_policy,
                    Some(&global_policy),
                    "review_severity_weight_runtime_assignments",
                    1,
                );
            let (severity_weight_executions, severity_weight_executions_source) =
                resolve_usize_setting_with_source(
                    &record.constraints,
                    skill_policy,
                    Some(&global_policy),
                    "review_severity_weight_executions",
                    1,
                );
            let (severity_weight_severe_flags, severity_weight_severe_flags_source) =
                resolve_usize_setting_with_source(
                    &record.constraints,
                    skill_policy,
                    Some(&global_policy),
                    "review_severity_weight_severe_flags",
                    1,
                );
            Some(RegistryOverviewImplementationHotspotJson {
                implementation_id: record.implementation_id.clone(),
                skill_id: record.skill_id.clone(),
                executor: record.executor.clone(),
                recent_guardrail_block_count: guardrail.recent_guardrail_block_count,
                top_reason: guardrail.top_reason.unwrap_or_else(|| "<none>".to_owned()),
                recommended_by_skill_count: usage.recommended_by_skill_count,
                runtime_task_count: usage.runtime_task_count,
                active_task_count: usage.active_task_count,
                runtime_assignment_count: usage.runtime_assignment_count,
                execution_count: usage.execution_count,
                flags: implementation_governance_flags(&governed),
                refresh_min_absolute_increase,
                refresh_min_multiplier,
                refresh_min_severity_delta,
                severity_weight_recommended_by,
                severity_weight_active_tasks,
                severity_weight_runtime_assignments,
                severity_weight_executions,
                severity_weight_severe_flags,
                refresh_min_absolute_increase_source: refresh_min_absolute_increase_source
                    .to_owned(),
                refresh_min_multiplier_source: refresh_min_multiplier_source.to_owned(),
                refresh_min_severity_delta_source: refresh_min_severity_delta_source.to_owned(),
                severity_weight_recommended_by_source: severity_weight_recommended_by_source
                    .to_owned(),
                severity_weight_active_tasks_source: severity_weight_active_tasks_source.to_owned(),
                severity_weight_runtime_assignments_source: severity_weight_runtime_assignments_source
                    .to_owned(),
                severity_weight_executions_source: severity_weight_executions_source.to_owned(),
                severity_weight_severe_flags_source: severity_weight_severe_flags_source.to_owned(),
            })
        })
        .collect::<Vec<_>>();
    implementation_hotspot_rows.sort_by(|a, b| {
        b.recent_guardrail_block_count
            .cmp(&a.recent_guardrail_block_count)
            .then_with(|| b.execution_count.cmp(&a.execution_count))
            .then_with(|| b.runtime_assignment_count.cmp(&a.runtime_assignment_count))
            .then_with(|| {
                b.recommended_by_skill_count
                    .cmp(&a.recommended_by_skill_count)
            })
            .then_with(|| b.active_task_count.cmp(&a.active_task_count))
            .then_with(|| a.implementation_id.cmp(&b.implementation_id))
    });
    if let Some(limit) = limit {
        implementation_hotspot_rows.truncate(limit);
    }
    Ok(implementation_hotspot_rows)
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn enrich_reflection_inputs_from_hotspots(
    detected_drifts: &mut Vec<String>,
    freeze_actions: &mut Vec<String>,
    next_actions: &mut Vec<String>,
    evidence_refs: &mut Vec<String>,
    hotspots: &[RegistryOverviewImplementationHotspotJson],
) {
    for hotspot in hotspots {
        push_unique(
            detected_drifts,
            format!(
                "implementation {} triggered {} recent guardrail blocks while still recommended_by={} active_tasks={} assignments={} executions={}",
                hotspot.implementation_id,
                hotspot.recent_guardrail_block_count,
                hotspot.recommended_by_skill_count,
                hotspot.active_task_count,
                hotspot.runtime_assignment_count,
                hotspot.execution_count
            ),
        );
        push_unique(
            next_actions,
            format!(
                "review implementation {} for guardrail reason {} and adjust strategy/constraints before further promotion",
                hotspot.implementation_id, hotspot.top_reason
            ),
        );
        if hotspot.recommended_by_skill_count > 0 {
            push_unique(
                freeze_actions,
                format!(
                    "freeze recommendation changes for implementation {} until guardrail hotspot is resolved",
                    hotspot.implementation_id
                ),
            );
        }
        if hotspot.active_task_count > 0 {
            push_unique(
                next_actions,
                format!(
                    "inspect active tasks bound to implementation {} because {} tasks are still running behind a guardrail hotspot",
                    hotspot.implementation_id, hotspot.active_task_count
                ),
            );
        }
        if hotspot.runtime_assignment_count > 0 {
            push_unique(
                next_actions,
                format!(
                    "inspect runtime assignments bound to implementation {} because {} assignments still point at a guardrail hotspot",
                    hotspot.implementation_id, hotspot.runtime_assignment_count
                ),
            );
        }
        if hotspot.execution_count > 0 {
            push_unique(
                next_actions,
                format!(
                    "inspect recent execution records for implementation {} because {} executions passed through a guardrail hotspot",
                    hotspot.implementation_id, hotspot.execution_count
                ),
            );
        }
        push_unique(
            evidence_refs,
            format!("implementation_hotspot:{}", hotspot.implementation_id),
        );
    }
}

fn enrich_review_inputs_from_hotspots(
    required_followups: &mut Vec<String>,
    evidence_refs: &mut Vec<String>,
    hotspots: &[RegistryOverviewImplementationHotspotJson],
) {
    for hotspot in hotspots {
        push_unique(
            required_followups,
            format!(
                "reassess implementation {} because it triggered {} recent guardrail blocks for {}",
                hotspot.implementation_id, hotspot.recent_guardrail_block_count, hotspot.top_reason
            ),
        );
        if hotspot.recommended_by_skill_count > 0 {
            push_unique(
                required_followups,
                format!(
                    "review current recommendation bindings for implementation {} across {} skill(s)",
                    hotspot.implementation_id, hotspot.recommended_by_skill_count
                ),
            );
        }
        if hotspot.active_task_count > 0 {
            push_unique(
                required_followups,
                format!(
                    "inspect {} active task(s) still using implementation {} before approving further rollout",
                    hotspot.active_task_count, hotspot.implementation_id
                ),
            );
        }
        if hotspot.runtime_assignment_count > 0 {
            push_unique(
                required_followups,
                format!(
                    "inspect {} assignment(s) still bound to implementation {} before approving further rollout",
                    hotspot.runtime_assignment_count, hotspot.implementation_id
                ),
            );
        }
        if hotspot.execution_count > 0 {
            push_unique(
                required_followups,
                format!(
                    "review {} recent execution record(s) for implementation {} before approving further rollout",
                    hotspot.execution_count, hotspot.implementation_id
                ),
            );
        }
        push_unique(
            evidence_refs,
            format!("implementation_hotspot:{}", hotspot.implementation_id),
        );
    }
}

fn parse_materialized_review_guardrail_count(rationale: &str) -> Option<usize> {
    rationale
        .split("triggered ")
        .nth(1)
        .and_then(|tail| tail.split_whitespace().next())
        .and_then(|value| value.parse::<usize>().ok())
}

fn parse_rationale_metric(rationale: &str, label: &str) -> Option<usize> {
    rationale
        .split(&format!("{label}="))
        .nth(1)
        .and_then(|tail| {
            tail.split(|ch: char| !ch.is_ascii_digit())
                .next()
                .filter(|value| !value.is_empty())
        })
        .and_then(|value| value.parse::<usize>().ok())
}

fn severe_hotspot_flag_count(flags: &[String]) -> usize {
    flags
        .iter()
        .filter(|flag| {
            matches!(
                flag.as_str(),
                "high_cost_budget"
                    | "high_latency_budget"
                    | "no_components"
                    | "no_strategy"
                    | "shell_executor"
            )
        })
        .count()
}

fn hotspot_severity_score(
    guardrail_count: usize,
    recommended_by_skill_count: usize,
    active_task_count: usize,
    runtime_assignment_count: usize,
    execution_count: usize,
    severe_flag_count: usize,
    recommended_by_weight: usize,
    active_task_weight: usize,
    runtime_assignment_weight: usize,
    execution_weight: usize,
    severe_flag_weight: usize,
) -> usize {
    guardrail_count
        + (recommended_by_skill_count * recommended_by_weight)
        + (active_task_count * active_task_weight)
        + (runtime_assignment_count * runtime_assignment_weight)
        + (execution_count * execution_weight)
        + (severe_flag_count * severe_flag_weight)
}

fn hotspot_has_meaningful_guardrail_increase(
    current_count: usize,
    previous_count: usize,
    current_recommended_by: usize,
    previous_recommended_by: usize,
    current_active_tasks: usize,
    previous_active_tasks: usize,
    current_runtime_assignments: usize,
    previous_runtime_assignments: usize,
    current_executions: usize,
    previous_executions: usize,
    current_severe_flags: usize,
    previous_severe_flags: usize,
    min_absolute_increase: usize,
    min_multiplier: f64,
    min_severity_delta: usize,
    recommended_by_weight: usize,
    active_task_weight: usize,
    runtime_assignment_weight: usize,
    execution_weight: usize,
    severe_flag_weight: usize,
) -> bool {
    if current_count <= previous_count
        && current_recommended_by <= previous_recommended_by
        && current_active_tasks <= previous_active_tasks
        && current_runtime_assignments <= previous_runtime_assignments
        && current_executions <= previous_executions
        && current_severe_flags <= previous_severe_flags
    {
        return false;
    }
    let absolute_increase = current_count.saturating_sub(previous_count);
    let multiplier_triggered = if previous_count == 0 {
        current_count > 0
    } else {
        (current_count as f64) >= (previous_count as f64 * min_multiplier)
    };
    if multiplier_triggered || absolute_increase >= min_absolute_increase {
        return true;
    }
    let current_severity = hotspot_severity_score(
        current_count,
        current_recommended_by,
        current_active_tasks,
        current_runtime_assignments,
        current_executions,
        current_severe_flags,
        recommended_by_weight,
        active_task_weight,
        runtime_assignment_weight,
        execution_weight,
        severe_flag_weight,
    );
    let previous_severity = hotspot_severity_score(
        previous_count,
        previous_recommended_by,
        previous_active_tasks,
        previous_runtime_assignments,
        previous_executions,
        previous_severe_flags,
        recommended_by_weight,
        active_task_weight,
        runtime_assignment_weight,
        execution_weight,
        severe_flag_weight,
    );
    current_severity >= previous_severity.saturating_add(min_severity_delta)
}

fn next_review_refresh_id(
    base_review_id: &str,
    existing_reviews: &[ArchitectureReviewRecord],
) -> String {
    let mut max_suffix = 1usize;
    for review in existing_reviews {
        if review.review_id == base_review_id {
            continue;
        }
        let Some(suffix) = review
            .review_id
            .strip_prefix(base_review_id)
            .and_then(|tail| tail.strip_prefix("-refresh-"))
        else {
            continue;
        };
        if let Ok(value) = suffix.parse::<usize>() {
            max_suffix = max_suffix.max(value);
        }
    }
    format!("{base_review_id}-refresh-{}", max_suffix + 1)
}

fn build_review_suggestions(
    hotspots: &[RegistryOverviewImplementationHotspotJson],
    existing_reviews: &[ArchitectureReviewRecord],
) -> Vec<ReviewSuggestionJson> {
    hotspots
        .iter()
        .map(|hotspot| {
            let base_review_id = format!("review-{}-guardrail-hotspot", hotspot.implementation_id);
            let mut matching_reviews = existing_reviews
                .iter()
                .filter(|review| {
                    review.review_id == base_review_id
                        || review
                            .review_id
                            .starts_with(&format!("{base_review_id}-refresh-"))
                })
                .collect::<Vec<_>>();
            matching_reviews.sort_by(|a, b| a.review_id.cmp(&b.review_id));
            let latest_review = matching_reviews.last().copied();
            let latest_count = latest_review
                .and_then(|review| parse_materialized_review_guardrail_count(&review.rationale));
            let latest_recommended_by = latest_review
                .and_then(|review| parse_rationale_metric(&review.rationale, "recommended_by"));
            let latest_active_tasks =
                latest_review.and_then(|review| parse_rationale_metric(&review.rationale, "active_tasks"));
            let latest_runtime_assignments =
                latest_review.and_then(|review| parse_rationale_metric(&review.rationale, "assignments"));
            let latest_executions =
                latest_review.and_then(|review| parse_rationale_metric(&review.rationale, "executions"));
            let latest_severe_flags =
                latest_review.and_then(|review| parse_rationale_metric(&review.rationale, "severe_flags"));
            let current_severe_flags = severe_hotspot_flag_count(&hotspot.flags);
            let (suggested_review_id, suggestion_state, source_review_id, already_recorded) =
                match latest_review {
                    Some(review)
                        if latest_count.is_some_and(|count| {
                            hotspot_has_meaningful_guardrail_increase(
                                hotspot.recent_guardrail_block_count,
                                count,
                                hotspot.recommended_by_skill_count,
                                latest_recommended_by.unwrap_or(0),
                                hotspot.active_task_count,
                                latest_active_tasks.unwrap_or(0),
                                hotspot.runtime_assignment_count,
                                latest_runtime_assignments.unwrap_or(0),
                                hotspot.execution_count,
                                latest_executions.unwrap_or(0),
                                current_severe_flags,
                                latest_severe_flags.unwrap_or(0),
                                hotspot.refresh_min_absolute_increase,
                                hotspot.refresh_min_multiplier,
                                hotspot.refresh_min_severity_delta,
                                hotspot.severity_weight_recommended_by,
                                hotspot.severity_weight_active_tasks,
                                hotspot.severity_weight_runtime_assignments,
                                hotspot.severity_weight_executions,
                                hotspot.severity_weight_severe_flags,
                            )
                        }) =>
                    {
                        (
                            next_review_refresh_id(&base_review_id, existing_reviews),
                            "worsened".to_owned(),
                            Some(review.review_id.clone()),
                            false,
                        )
                    }
                    Some(review) => (
                        review.review_id.clone(),
                        "existing".to_owned(),
                        Some(review.review_id.clone()),
                        true,
                    ),
                    None => (base_review_id, "new".to_owned(), None, false),
                };
            let mut required_followups = Vec::new();
            let mut evidence_refs = Vec::new();
            enrich_review_inputs_from_hotspots(
                &mut required_followups,
                &mut evidence_refs,
                std::slice::from_ref(hotspot),
            );
            let proposed_decision =
                if hotspot.recommended_by_skill_count > 0
                    || hotspot.active_task_count > 0
                    || hotspot.runtime_assignment_count > 0
                    || hotspot.execution_count > 0
                {
                    "needs_redesign"
                } else {
                    "pass_with_followup"
                };
            ReviewSuggestionJson {
                suggested_review_id: suggested_review_id.clone(),
                suggestion_state,
                source_review_id,
                title: format!(
                    "guardrail hotspot review for {}",
                    hotspot.implementation_id
                ),
                change_scope: "implementation_guardrail_hotspot".to_owned(),
                target_plane: "evolution".to_owned(),
                target_modules: vec!["governance".to_owned(), "registry".to_owned()],
                rationale: format!(
                    "implementation {} triggered {} recent guardrail blocks for {} while recommended_by={} active_tasks={} assignments={} executions={} severe_flags={} refresh_min_absolute_increase={} refresh_min_multiplier={} refresh_min_severity_delta={} severity_weight_recommended_by={} severity_weight_active_tasks={} severity_weight_runtime_assignments={} severity_weight_executions={} severity_weight_severe_flags={}",
                    hotspot.implementation_id,
                    hotspot.recent_guardrail_block_count,
                    hotspot.top_reason,
                    hotspot.recommended_by_skill_count,
                    hotspot.active_task_count,
                    hotspot.runtime_assignment_count,
                    hotspot.execution_count,
                    current_severe_flags,
                    hotspot.refresh_min_absolute_increase,
                    hotspot.refresh_min_multiplier,
                    hotspot.refresh_min_severity_delta,
                    hotspot.severity_weight_recommended_by,
                    hotspot.severity_weight_active_tasks,
                    hotspot.severity_weight_runtime_assignments,
                    hotspot.severity_weight_executions,
                    hotspot.severity_weight_severe_flags
                ),
                proposed_decision: proposed_decision.to_owned(),
                required_followups,
                evidence_refs,
                implementation_id: hotspot.implementation_id.clone(),
                skill_id: hotspot.skill_id.clone(),
                recent_guardrail_block_count: hotspot.recent_guardrail_block_count,
                recommended_by_skill_count: hotspot.recommended_by_skill_count,
                active_task_count: hotspot.active_task_count,
                already_recorded,
            }
        })
        .collect()
}

fn materialize_review_from_suggestion(
    requested_by: &str,
    suggestion: &ReviewSuggestionJson,
    guardrail_snapshot: Option<ArchitectureGuardrailSnapshot>,
) -> ArchitectureReviewRecord {
    let decision = match suggestion.proposed_decision.as_str() {
        "needs_redesign" => ArchitectureReviewDecision::NeedsRedesign,
        "blocked" => ArchitectureReviewDecision::Blocked,
        "pass" => ArchitectureReviewDecision::Pass,
        _ => ArchitectureReviewDecision::PassWithFollowup,
    };
    ArchitectureReviewRecord::new(
        suggestion.suggested_review_id.clone(),
        suggestion.title.clone(),
        suggestion.change_scope.clone(),
        requested_by.to_owned(),
        ReviewTargetPlane::Evolution,
        suggestion.target_modules.clone(),
        false,
        true,
        false,
        true,
        false,
        ArchitectureReviewStatus::Open,
        decision,
        suggestion.rationale.clone(),
        suggestion.required_followups.clone(),
        suggestion.evidence_refs.clone(),
        guardrail_snapshot,
    )
}

fn summarize_guardrail_audits(
    root: &str,
    end_timestamp: Option<&str>,
    window_ms: Option<u128>,
) -> std::io::Result<GuardrailAuditSummary> {
    let (_, audits) = load_evolution_audits(root)?;
    let end_ms = end_timestamp.and_then(crate::core::parse_unix_ms_timestamp);
    let start_ms = match (end_ms, window_ms) {
        (Some(end), Some(window)) => Some(end.saturating_sub(window)),
        _ => None,
    };

    let mut action_counts = std::collections::BTreeMap::<String, usize>::new();
    let mut reason_counts = std::collections::BTreeMap::<String, usize>::new();
    let mut target_type_counts = std::collections::BTreeMap::<String, usize>::new();
    let mut target_id_counts = std::collections::BTreeMap::<String, usize>::new();
    let mut skill_counts = std::collections::BTreeMap::<String, usize>::new();
    let mut total_count = 0usize;

    for audit in audits {
        if audit.result != "guardrail_blocked" {
            continue;
        }
        let audit_ms = crate::core::parse_unix_ms_timestamp(&audit.timestamp);
        if let Some(end) = end_ms {
            if audit_ms.is_some_and(|value| value > end) {
                continue;
            }
        }
        if let Some(start) = start_ms {
            if audit_ms.is_some_and(|value| value < start) {
                continue;
            }
        }
        total_count += 1;
        *action_counts.entry(audit.action.clone()).or_insert(0) += 1;
        *target_type_counts
            .entry(audit.target_type.clone())
            .or_insert(0) += 1;
        *target_id_counts.entry(audit.target_id.clone()).or_insert(0) += 1;
        *reason_counts
            .entry(extract_guardrail_reason(&audit.payload))
            .or_insert(0) += 1;
        if let Some(skill) = extract_guardrail_skill(&audit.payload) {
            *skill_counts.entry(skill).or_insert(0) += 1;
        }
    }

    let mut action_counts = action_counts.into_iter().collect::<Vec<_>>();
    action_counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let mut reason_counts = reason_counts.into_iter().collect::<Vec<_>>();
    reason_counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let mut target_type_counts = target_type_counts.into_iter().collect::<Vec<_>>();
    target_type_counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let mut target_id_counts = target_id_counts.into_iter().collect::<Vec<_>>();
    target_id_counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let mut skill_counts = skill_counts.into_iter().collect::<Vec<_>>();
    skill_counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    Ok(GuardrailAuditSummary {
        total_count,
        action_counts,
        reason_counts,
        target_type_counts,
        target_id_counts,
        skill_counts,
    })
}

fn implementation_governance_flags(
    implementation: &crate::governance::GovernedImplementation,
) -> Vec<String> {
    let mut flags = Vec::new();
    if implementation.component_count == 0 {
        flags.push("no_components".to_owned());
    }
    if implementation.strategy_count == 0 {
        flags.push("no_strategy".to_owned());
    }
    if parse_constraint_f64(implementation.max_latency_ms.as_deref())
        .is_some_and(|value| value > 5_000.0)
    {
        flags.push("high_latency_budget".to_owned());
    }
    if parse_constraint_f64(implementation.max_cost.as_deref()).is_some_and(|value| value > 0.05) {
        flags.push("high_cost_budget".to_owned());
    }
    if implementation.executor == "shell" || implementation.entry_kind == "shell" {
        flags.push("shell_executor".to_owned());
    }
    flags
}

fn governance_candidate_should_skip(
    implementation: &crate::governance::GovernedImplementation,
) -> Option<&'static str> {
    if implementation.component_count == 0 {
        return Some("missing_components");
    }
    if implementation.strategy_count == 0 {
        return Some("missing_strategy");
    }
    if implementation.executor == "shell" || implementation.entry_kind == "shell" {
        return Some("shell_executor");
    }
    if parse_constraint_f64(implementation.max_latency_ms.as_deref())
        .is_some_and(|value| value > 20_000.0)
    {
        return Some("extreme_latency_budget");
    }
    if parse_constraint_f64(implementation.max_cost.as_deref()).is_some_and(|value| value > 0.20) {
        return Some("extreme_cost_budget");
    }
    None
}

fn governance_candidate_penalty(implementation: &crate::governance::GovernedImplementation) -> u8 {
    implementation_governance_flags(implementation).len() as u8
}

fn recommended_decision(
    score: f64,
    implementation: &crate::governance::GovernedImplementation,
) -> GovernanceDecision {
    if score >= 0.95 {
        let latency = parse_constraint_f64(implementation.max_latency_ms.as_deref());
        let cost = parse_constraint_f64(implementation.max_cost.as_deref());
        if latency.is_some_and(|value| value > 20_000.0) || cost.is_some_and(|value| value > 0.20) {
            GovernanceDecision::Observe
        } else if latency.is_some_and(|value| value > 5_000.0)
            || cost.is_some_and(|value| value > 0.05)
        {
            GovernanceDecision::Hold
        } else {
            GovernanceDecision::Promote
        }
    } else if score >= 0.85 {
        let latency = parse_constraint_f64(implementation.max_latency_ms.as_deref());
        let cost = parse_constraint_f64(implementation.max_cost.as_deref());
        if latency.is_some_and(|value| value > 20_000.0) || cost.is_some_and(|value| value > 0.20) {
            GovernanceDecision::Observe
        } else {
            GovernanceDecision::Hold
        }
    } else {
        GovernanceDecision::Observe
    }
}

fn decision_rank(decision: GovernanceDecision) -> u8 {
    match decision {
        GovernanceDecision::Promote => 3,
        GovernanceDecision::Hold => 2,
        GovernanceDecision::Observe => 1,
        GovernanceDecision::Deprecate => 0,
    }
}

fn build_plan_for_record(record: &crate::governance::FitnessRecord) -> EvolutionPlan {
    let score = parse_score(&record.fitness_report.score);
    let decision = recommended_decision(score, &record.fitness_report.implementation);
    let flags = implementation_governance_flags(&record.fitness_report.implementation);
    let rationale = format!(
        "score={} executor={} mode={} prompt={} max_cost={} max_latency_ms={} flags={} skills={} tools={}",
        record.fitness_report.score,
        record.fitness_report.implementation.executor,
        record
            .fitness_report
            .implementation
            .strategy_mode
            .as_deref()
            .unwrap_or("<none>"),
        record
            .fitness_report
            .implementation
            .prompt_component
            .as_deref()
            .unwrap_or("<none>"),
        record
            .fitness_report
            .implementation
            .max_cost
            .as_deref()
            .unwrap_or("<none>"),
        record
            .fitness_report
            .implementation
            .max_latency_ms
            .as_deref()
            .unwrap_or("<none>"),
        if flags.is_empty() {
            "<none>".to_owned()
        } else {
            flags.join(", ")
        },
        joined_or_none(&record.fitness_report.skill_refs),
        joined_or_none(&record.fitness_report.tool_refs)
    );

    EvolutionPlan {
        implementation: record.fitness_report.implementation.clone(),
        decision,
        rationale,
    }
}

fn select_governance_candidate(
    root: &str,
    implementation_id: Option<&str>,
    skill_ref: Option<&str>,
    tool_ref: Option<&str>,
) -> std::io::Result<Option<crate::governance::FitnessRecord>> {
    let (_, records) = list_fitness_runs(root)?;
    let mut filtered = records
        .into_iter()
        .filter(|record| {
            let impl_match = implementation_id
                .is_none_or(|value| record.fitness_report.implementation_id() == value);
            let skill_match = skill_ref.is_none_or(|value| {
                record
                    .fitness_report
                    .skill_refs
                    .iter()
                    .any(|skill| skill == value)
            });
            let tool_match = tool_ref.is_none_or(|value| {
                record
                    .fitness_report
                    .tool_refs
                    .iter()
                    .any(|tool| tool == value)
            });
            impl_match && skill_match && tool_match
        })
        .filter(|record| {
            implementation_id.is_some()
                || governance_candidate_should_skip(&record.fitness_report.implementation).is_none()
        })
        .collect::<Vec<_>>();

    filtered.sort_by(|a, b| {
        decision_rank(b.evolution_plan.decision)
            .cmp(&decision_rank(a.evolution_plan.decision))
            .then_with(|| {
                governance_candidate_penalty(&a.fitness_report.implementation).cmp(
                    &governance_candidate_penalty(&b.fitness_report.implementation),
                )
            })
            .then_with(|| {
                parse_score(&b.fitness_report.score)
                    .partial_cmp(&parse_score(&a.fitness_report.score))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| {
                a.fitness_report
                    .implementation_id()
                    .cmp(b.fitness_report.implementation_id())
            })
    });

    Ok(filtered.into_iter().next())
}

fn governance_candidate_block_reason(
    root: &str,
    implementation_id: Option<&str>,
    skill_ref: Option<&str>,
    tool_ref: Option<&str>,
) -> std::io::Result<Option<String>> {
    let (_, records) = list_fitness_runs(root)?;
    let matching = records
        .into_iter()
        .filter(|record| {
            let impl_match = implementation_id
                .is_none_or(|value| record.fitness_report.implementation_id() == value);
            let skill_match = skill_ref.is_none_or(|value| {
                record
                    .fitness_report
                    .skill_refs
                    .iter()
                    .any(|skill| skill == value)
            });
            let tool_match = tool_ref.is_none_or(|value| {
                record
                    .fitness_report
                    .tool_refs
                    .iter()
                    .any(|tool| tool == value)
            });
            impl_match && skill_match && tool_match
        })
        .collect::<Vec<_>>();
    if matching.is_empty() {
        return Ok(None);
    }
    let reasons = matching
        .iter()
        .filter_map(|record| {
            governance_candidate_should_skip(&record.fitness_report.implementation)
        })
        .collect::<Vec<_>>();
    if reasons.is_empty() {
        return Ok(Some("no eligible governance candidate".to_owned()));
    }
    Ok(Some(format!(
        "all matching governance candidates blocked by guardrails: {}",
        reasons.join(", ")
    )))
}

fn append_guardrail_block_audit(
    root: &str,
    action: &str,
    target_kind: &str,
    target_id: &str,
    detail: String,
) -> std::io::Result<()> {
    append_evolution_audit(
        root,
        &AuditRecord::now(
            format!("audit-{target_id}-{action}"),
            "system".to_owned(),
            "honeycomb-evolution".to_owned(),
            action.to_owned(),
            target_kind.to_owned(),
            target_id.to_owned(),
            String::new(),
            "guardrail_blocked".to_owned(),
            detail,
        ),
    )?;
    Ok(())
}

fn select_registry_sync_candidate(
    root: &str,
    skill_id: &str,
) -> std::io::Result<Option<crate::governance::FitnessRecord>> {
    let (_, records) = list_fitness_runs(root)?;
    let mut filtered = records
        .into_iter()
        .filter(|record| {
            record
                .fitness_report
                .skill_refs
                .iter()
                .any(|skill| skill == skill_id)
        })
        .filter(|record| {
            governance_candidate_should_skip(&record.fitness_report.implementation).is_none()
        })
        .collect::<Vec<_>>();

    filtered.sort_by(|a, b| {
        decision_rank(b.evolution_plan.decision)
            .cmp(&decision_rank(a.evolution_plan.decision))
            .then_with(|| {
                governance_candidate_penalty(&a.fitness_report.implementation).cmp(
                    &governance_candidate_penalty(&b.fitness_report.implementation),
                )
            })
            .then_with(|| {
                parse_score(&b.fitness_report.score)
                    .partial_cmp(&parse_score(&a.fitness_report.score))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| {
                a.fitness_report
                    .implementation_id()
                    .cmp(b.fitness_report.implementation_id())
            })
    });

    Ok(filtered.into_iter().next())
}

fn handle_governance_plan(args: &[String]) -> ExitCode {
    let root = option_value(args, "--root").unwrap_or(".");
    let implementation_id = option_value(args, "--implementation");
    let skill_ref = option_value(args, "--skill-ref");
    let tool_ref = option_value(args, "--tool-ref");

    let candidate = match select_governance_candidate(root, implementation_id, skill_ref, tool_ref)
    {
        Ok(Some(record)) => record,
        Ok(None) => {
            let reason =
                governance_candidate_block_reason(root, implementation_id, skill_ref, tool_ref)
                    .unwrap_or_else(|_| None)
                    .unwrap_or_else(|| "no matching fitness record".to_owned());
            let target_id = implementation_id
                .or(skill_ref)
                .or(tool_ref)
                .unwrap_or("governance-plan");
            let detail = format!(
                "implementation={} skill_ref={} tool_ref={} reason={}",
                implementation_id.unwrap_or("<none>"),
                skill_ref.unwrap_or("<none>"),
                tool_ref.unwrap_or("<none>"),
                reason
            );
            if let Err(error) = append_guardrail_block_audit(
                root,
                "governance_plan_guardrail_block",
                "governance_candidate",
                target_id,
                detail,
            ) {
                eprintln!("failed to append governance plan guardrail audit: {error}");
            }
            eprintln!("failed to build governance plan: {reason}");
            return ExitCode::from(1);
        }
        Err(error) => {
            eprintln!("failed to build governance plan: {error}");
            return ExitCode::from(1);
        }
    };
    let plan = build_plan_for_record(&candidate);

    println!("governance plan generated");
    println!(
        "  implementation_id: {}",
        candidate.fitness_report.implementation_id()
    );
    println!(
        "  implementation_skill_id: {}",
        candidate.fitness_report.implementation.skill_id
    );
    println!(
        "  implementation_executor: {}",
        candidate.fitness_report.implementation.executor
    );
    println!(
        "  implementation_mode: {}",
        candidate
            .fitness_report
            .implementation
            .strategy_mode
            .as_deref()
            .unwrap_or("<none>")
    );
    println!(
        "  implementation_max_cost: {}",
        candidate
            .fitness_report
            .implementation
            .max_cost
            .as_deref()
            .unwrap_or("<none>")
    );
    println!(
        "  implementation_max_latency_ms: {}",
        candidate
            .fitness_report
            .implementation
            .max_latency_ms
            .as_deref()
            .unwrap_or("<none>")
    );
    println!("  score: {}", candidate.fitness_report.score);
    println!(
        "  skill_refs: {}",
        joined_or_none(&candidate.fitness_report.skill_refs)
    );
    println!(
        "  tool_refs: {}",
        joined_or_none(&candidate.fitness_report.tool_refs)
    );
    println!("  decision: {}", plan.decision.as_str());
    println!("  rationale: {}", plan.rationale);
    ExitCode::SUCCESS
}

fn handle_governance_apply(args: &[String]) -> ExitCode {
    let root = option_value(args, "--root").unwrap_or(".");
    let implementation_id = option_value(args, "--implementation");
    let skill_ref = option_value(args, "--skill-ref");
    let tool_ref = option_value(args, "--tool-ref");

    let candidate = match select_governance_candidate(root, implementation_id, skill_ref, tool_ref)
    {
        Ok(Some(record)) => record,
        Ok(None) => {
            let reason =
                governance_candidate_block_reason(root, implementation_id, skill_ref, tool_ref)
                    .unwrap_or_else(|_| None)
                    .unwrap_or_else(|| "no matching fitness record".to_owned());
            let target_id = implementation_id
                .or(skill_ref)
                .or(tool_ref)
                .unwrap_or("governance-apply");
            let detail = format!(
                "implementation={} skill_ref={} tool_ref={} reason={}",
                implementation_id.unwrap_or("<none>"),
                skill_ref.unwrap_or("<none>"),
                tool_ref.unwrap_or("<none>"),
                reason
            );
            if let Err(error) = append_guardrail_block_audit(
                root,
                "governance_apply_guardrail_block",
                "governance_candidate",
                target_id,
                detail,
            ) {
                eprintln!("failed to append governance apply guardrail audit: {error}");
            }
            eprintln!("failed to apply governance plan: {reason}");
            return ExitCode::from(1);
        }
        Err(error) => {
            eprintln!("failed to apply governance plan: {error}");
            return ExitCode::from(1);
        }
    };
    let plan = build_plan_for_record(&candidate);

    let (path, record) = match update_fitness_plan(root, plan.implementation_id(), &plan) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to persist governance plan: {error}");
            return ExitCode::from(1);
        }
    };

    if let Err(error) = append_evolution_audit(
        root,
        &AuditRecord::now(
            format!("audit-{}-governance-apply", plan.implementation_id()),
            "user".to_owned(),
            "local-cli".to_owned(),
            "governance_apply".to_owned(),
            "implementation".to_owned(),
            plan.implementation_id().to_owned(),
            String::new(),
            plan.decision.as_str().to_owned(),
            format!(
                "score={} skill={} executor={} mode={} max_cost={} max_latency_ms={} skills={} tools={}",
                record.fitness_report.score,
                record.fitness_report.implementation.skill_id,
                record.fitness_report.implementation.executor,
                record
                    .fitness_report
                    .implementation
                    .strategy_mode
                    .as_deref()
                    .unwrap_or("<none>"),
                record
                    .fitness_report
                    .implementation
                    .max_cost
                    .as_deref()
                    .unwrap_or("<none>"),
                record
                    .fitness_report
                    .implementation
                    .max_latency_ms
                    .as_deref()
                    .unwrap_or("<none>"),
                joined_or_none(&record.fitness_report.skill_refs),
                joined_or_none(&record.fitness_report.tool_refs)
            ),
        ),
    ) {
        eprintln!("failed to append governance audit: {error}");
        return ExitCode::from(1);
    }

    println!("governance apply completed");
    println!("  implementation_id: {}", plan.implementation_id());
    println!(
        "  implementation_skill_id: {}",
        plan.implementation.skill_id
    );
    println!(
        "  implementation_executor: {}",
        plan.implementation.executor
    );
    println!(
        "  implementation_mode: {}",
        plan.implementation
            .strategy_mode
            .as_deref()
            .unwrap_or("<none>")
    );
    println!("  decision: {}", plan.decision.as_str());
    println!("  rationale: {}", plan.rationale);
    println!("  written_to: {}", path.display());
    ExitCode::SUCCESS
}

fn handle_registry_sync(args: &[String]) -> ExitCode {
    let root = option_value(args, "--root").unwrap_or(".");
    let sync_all = args.iter().any(|arg| arg == "--all");

    let skill_ids = if sync_all {
        let (_, skills) = match list_skills(root) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to load skills for registry sync: {error}");
                return ExitCode::from(1);
            }
        };
        skills
            .into_iter()
            .map(|skill| skill.skill_id)
            .collect::<Vec<_>>()
    } else {
        vec![
            option_value(args, "--skill-id")
                .unwrap_or("xhs_publish")
                .to_owned(),
        ]
    };

    let mut synced = Vec::new();
    let mut skipped = Vec::new();

    for skill_id in skill_ids {
        let (_, skill_record) = match load_skill(root, &skill_id) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to load skill for registry sync: {error}");
                return ExitCode::from(1);
            }
        };
        if let Err(error) = validate_skill_implementation_refs(root, &skill_record) {
            eprintln!("failed to validate skill implementations for registry sync: {error}");
            return ExitCode::from(1);
        }

        let (_, lineage_records) = match list_fitness_runs(root) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to load lineage for registry sync: {error}");
                return ExitCode::from(1);
            }
        };
        let matching_lineage = lineage_records
            .into_iter()
            .filter(|record| {
                record
                    .fitness_report
                    .skill_refs
                    .iter()
                    .any(|skill| skill == &skill_id)
            })
            .collect::<Vec<_>>();
        let skipped_for_risk = matching_lineage
            .iter()
            .filter_map(|record| {
                governance_candidate_should_skip(&record.fitness_report.implementation)
            })
            .collect::<Vec<_>>();
        let candidate = match select_registry_sync_candidate(root, &skill_id) {
            Ok(Some(record)) => record,
            Ok(None) => {
                let reason = if matching_lineage.is_empty() {
                    "no lineage candidate".to_owned()
                } else if !skipped_for_risk.is_empty() {
                    format!(
                        "all lineage candidates blocked by guardrails: {}",
                        skipped_for_risk.join(", ")
                    )
                } else {
                    "no eligible lineage candidate".to_owned()
                };
                if reason.contains("guardrails") {
                    let detail = format!(
                        "skill={} skipped_reasons={}",
                        skill_id,
                        skipped_for_risk.join(", ")
                    );
                    if let Err(error) = append_guardrail_block_audit(
                        root,
                        "registry_sync_guardrail_block",
                        "skill",
                        &skill_id,
                        detail,
                    ) {
                        eprintln!("failed to append registry sync guardrail audit: {error}");
                    }
                }
                skipped.push((skill_id, reason));
                continue;
            }
            Err(error) => {
                eprintln!("failed to sync registry: {error}");
                return ExitCode::from(1);
            }
        };
        let (_, candidate_implementation) =
            match load_implementation(root, candidate.fitness_report.implementation_id()) {
                Ok(value) => value,
                Err(error) => {
                    eprintln!("failed to load registry sync implementation candidate: {error}");
                    return ExitCode::from(1);
                }
            };
        if candidate_implementation.skill_id != skill_id {
            skipped.push((
                skill_id,
                format!(
                    "candidate implementation belongs to different skill {}",
                    candidate_implementation.skill_id
                ),
            ));
            continue;
        }

        let timestamp = crate::core::current_timestamp();
        let (path, skill) = match update_skill(root, &skill_id, |skill| {
            skill.recommended_implementation_id =
                Some(candidate.fitness_report.implementation_id().to_owned());
            skill.governance_decision = Some(candidate.evolution_plan.decision);
            skill.last_synced_at = Some(timestamp.clone());
            Ok(())
        }) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to update skill registry: {error}");
                return ExitCode::from(1);
            }
        };

        if let Err(error) = append_evolution_audit(
            root,
            &AuditRecord::new(
                format!("audit-{}-registry-sync", skill.skill_id),
                timestamp,
                "user".to_owned(),
                "local-cli".to_owned(),
                "registry_sync".to_owned(),
                "skill".to_owned(),
                skill.skill_id.clone(),
                String::new(),
                skill
                    .governance_decision
                    .map(|decision| decision.as_str().to_owned())
                    .unwrap_or_else(|| "unknown".to_owned()),
                format!(
                    "recommended_implementation={} source_score={} flags={}",
                    skill
                        .recommended_implementation_id
                        .clone()
                        .unwrap_or_else(|| "<none>".to_owned()),
                    candidate.fitness_report.score,
                    {
                        let flags = implementation_governance_flags(
                            &candidate.fitness_report.implementation,
                        );
                        if flags.is_empty() {
                            "<none>".to_owned()
                        } else {
                            flags.join(", ")
                        }
                    }
                ),
            ),
        ) {
            eprintln!("failed to append registry sync audit: {error}");
            return ExitCode::from(1);
        }

        synced.push((path, skill));
    }

    println!("registry sync completed");
    println!("  mode: {}", if sync_all { "all" } else { "single" });
    println!("  synced_count: {}", synced.len());
    for (path, skill) in synced {
        println!(
            "  synced: skill={} recommended_implementation_id={} governance_decision={} last_synced_at={} written_to={}",
            skill.skill_id,
            skill
                .recommended_implementation_id
                .as_deref()
                .unwrap_or("<none>"),
            skill
                .governance_decision
                .map(|decision| decision.as_str())
                .unwrap_or("<none>"),
            skill.last_synced_at.as_deref().unwrap_or("<none>"),
            path.display()
        );
    }
    println!("  skipped_count: {}", skipped.len());
    for (skill_id, reason) in skipped {
        println!("  skipped: skill={} reason={}", skill_id, reason);
    }
    ExitCode::SUCCESS
}

fn handle_registry_overview(args: &[String]) -> ExitCode {
    let root = option_value(args, "--root").unwrap_or(".");
    let with_details = args.iter().any(|arg| arg == "--with-details");
    let with_gaps = args.iter().any(|arg| arg == "--with-gaps");
    let with_policy = args.iter().any(|arg| arg == "--with-policy");
    let exclude_legacy = args.iter().any(|arg| arg == "--exclude-legacy");
    let as_json = args.iter().any(|arg| arg == "--json");
    let governance_defaults =
        summarize_governance_defaults_for_overview(load_governance_defaults(root).ok());

    let (skills_dir, skills) = match list_skills(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load skills for registry overview: {error}");
            return ExitCode::from(1);
        }
    };
    let (implementations_dir, implementations) = match list_implementations(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load implementations for registry overview: {error}");
            return ExitCode::from(1);
        }
    };
    let (tools_dir, tools) = match list_tools(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load tools for registry overview: {error}");
            return ExitCode::from(1);
        }
    };
    let (fitness_dir, fitness_runs) = match list_fitness_runs(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load fitness runs for registry overview: {error}");
            return ExitCode::from(1);
        }
    };
    let (tasks_dir, all_tasks) = match list_task_submissions(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load tasks for registry overview: {error}");
            return ExitCode::from(1);
        }
    };
    let tasks = all_tasks
        .into_iter()
        .filter(|task| !exclude_legacy || !is_legacy_demo_task(task))
        .collect::<Vec<_>>();
    for skill in &skills {
        if let Err(error) = validate_skill_implementation_refs(root, skill) {
            eprintln!(
                "failed to validate skill implementations for registry overview skill {}: {error}",
                skill.skill_id
            );
            return ExitCode::from(1);
        }
    }

    let recommended_skill_count = skills
        .iter()
        .filter(|skill| skill.recommended_implementation_id.is_some())
        .count();
    let promoted_count = fitness_runs
        .iter()
        .filter(|record| record.evolution_plan.decision == GovernanceDecision::Promote)
        .count();
    let hold_count = fitness_runs
        .iter()
        .filter(|record| record.evolution_plan.decision == GovernanceDecision::Hold)
        .count();
    let observe_count = fitness_runs
        .iter()
        .filter(|record| record.evolution_plan.decision == GovernanceDecision::Observe)
        .count();
    let deprecated_count = fitness_runs
        .iter()
        .filter(|record| record.evolution_plan.decision == GovernanceDecision::Deprecate)
        .count();

    let task_count = tasks.len();
    let completed_task_count = tasks
        .iter()
        .filter(|task| task.task_runtime.status.as_str() == "completed")
        .count();
    let implementation_bound_task_count = tasks
        .iter()
        .filter(|task| runtime_task_implementation_id(task).is_some())
        .count();

    let mut assignment_count = 0usize;
    let mut completed_assignment_count = 0usize;
    let mut implementation_usage = std::collections::BTreeMap::<String, usize>::new();
    let mut skill_usage = std::collections::BTreeMap::<String, usize>::new();
    let mut tool_usage = std::collections::BTreeMap::<String, usize>::new();
    for task in &tasks {
        if let Some(implementation_ref) = runtime_task_implementation_id(task) {
            *implementation_usage
                .entry(implementation_ref.to_owned())
                .or_insert(0) += 1;
        }
        for skill_ref in &task.task_spec.skill_refs {
            *skill_usage.entry(skill_ref.clone()).or_insert(0) += 1;
        }
        for tool_ref in &task.task_spec.tool_refs {
            *tool_usage.entry(tool_ref.clone()).or_insert(0) += 1;
        }
        let (_, assignments) = match load_task_assignments(root, &task.task_spec.task_id) {
            Ok(value) => value,
            Err(error) => {
                eprintln!(
                    "failed to load assignments for registry overview task {}: {error}",
                    task.task_spec.task_id
                );
                return ExitCode::from(1);
            }
        };
        assignment_count += assignments.len();
        completed_assignment_count += assignments
            .iter()
            .filter(|assignment| assignment.status.as_str() == "completed")
            .count();
    }

    let policy = if with_policy {
        let (_, approval_requests) = match list_shell_approval_requests(root) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to load shell approval requests for registry overview: {error}");
                return ExitCode::from(1);
            }
        };
        let (_, alert_acks) = match list_policy_alert_acks(root) {
            Ok(value) => value,
            Err(error) => {
                eprintln!(
                    "failed to load policy alert acknowledgements for registry overview: {error}"
                );
                return ExitCode::from(1);
            }
        };
        let acked_ids = alert_acks
            .into_iter()
            .map(|ack| ack.alert_id)
            .collect::<std::collections::BTreeSet<_>>();
        let shell_tools = tools
            .iter()
            .filter(|tool| tool.entrypoint.starts_with("shell://"))
            .collect::<Vec<_>>();
        let shell_tools_allowed = shell_tools.iter().filter(|tool| tool.allow_shell).count();
        let shell_tools_blocked = shell_tools.len().saturating_sub(shell_tools_allowed);
        let shell_tools_pending = shell_tools
            .iter()
            .filter(|tool| tool.shell_approval_pending)
            .count();
        let shell_tool_rows = shell_tools
            .iter()
            .map(|tool| {
                let trust_tier = match tool.owner.as_str() {
                    "system" => "system",
                    "tenant-local" => "trusted_local",
                    _ => "tenant",
                };
                RegistryOverviewPolicyShellToolJson {
                    tool_id: tool.tool_id.clone(),
                    owner: tool.owner.clone(),
                    scheme: "shell".to_owned(),
                    trust_tier: trust_tier.to_owned(),
                    allow_shell: tool.allow_shell,
                    pending: tool.shell_approval_pending,
                }
            })
            .collect::<Vec<_>>();

        let pending_requests = approval_requests
            .iter()
            .filter(|request| request.status.as_str() == "pending")
            .collect::<Vec<_>>();
        let overdue_requests = pending_requests
            .iter()
            .copied()
            .filter(|request| {
                approval_request_age_ms(request).is_some_and(|age| age >= 60 * 60 * 1000)
            })
            .collect::<Vec<_>>();
        let blocked_unacked = shell_tools
            .iter()
            .filter(|tool| !tool.allow_shell)
            .filter(|tool| !acked_ids.contains(&blocked_tool_alert_id(&tool.tool_id)))
            .collect::<Vec<_>>();
        let overdue_unacked = overdue_requests
            .iter()
            .copied()
            .filter(|request| !acked_ids.contains(&overdue_request_alert_id(&request.request_id)))
            .collect::<Vec<_>>();

        let mut inbox_by_owner = std::collections::BTreeMap::<String, usize>::new();
        for tool in &blocked_unacked {
            *inbox_by_owner.entry(tool.owner.clone()).or_insert(0) += 1;
        }
        for request in &overdue_unacked {
            *inbox_by_owner.entry(request.owner.clone()).or_insert(0) += 1;
        }
        let inbox_owner_rows = inbox_by_owner
            .into_iter()
            .map(|(owner, count)| RegistryOverviewPolicyOwnerJson { owner, count })
            .collect::<Vec<_>>();

        let mut pending_request_by_owner = std::collections::BTreeMap::<String, usize>::new();
        let mut pending_request_by_age_bucket = std::collections::BTreeMap::<String, usize>::new();
        for request in &pending_requests {
            *pending_request_by_owner
                .entry(request.owner.clone())
                .or_insert(0) += 1;
            *pending_request_by_age_bucket
                .entry(approval_age_bucket(approval_request_age_ms(request)).to_owned())
                .or_insert(0) += 1;
        }
        let pending_request_owner_rows = pending_request_by_owner
            .into_iter()
            .map(|(owner, count)| RegistryOverviewPolicyOwnerJson { owner, count })
            .collect::<Vec<_>>();
        let pending_request_age_rows = pending_request_by_age_bucket
            .into_iter()
            .map(|(bucket, count)| RegistryOverviewPolicyAgeBucketJson { bucket, count })
            .collect::<Vec<_>>();
        let pending_request_rows = pending_requests
            .iter()
            .map(|request| {
                let age_ms = approval_request_age_ms(request);
                RegistryOverviewPolicyRequestJson {
                    request_id: request.request_id.clone(),
                    tool_id: request.tool_id.clone(),
                    owner: request.owner.clone(),
                    requested_by: request.requested_by.clone(),
                    requested_at: request.requested_at.clone(),
                    age_ms,
                    age_bucket: approval_age_bucket(age_ms).to_owned(),
                }
            })
            .collect::<Vec<_>>();
        let mut alert_status_rows = shell_tools
            .iter()
            .filter(|tool| !tool.allow_shell)
            .map(|tool| RegistryOverviewPolicyAlertStatusJson {
                kind: "blocked_tool".to_owned(),
                target: tool.tool_id.clone(),
                acked: acked_ids.contains(&blocked_tool_alert_id(&tool.tool_id)),
            })
            .collect::<Vec<_>>();
        alert_status_rows.extend(overdue_requests.iter().map(|request| {
            RegistryOverviewPolicyAlertStatusJson {
                kind: "overdue_request".to_owned(),
                target: request.request_id.clone(),
                acked: acked_ids.contains(&overdue_request_alert_id(&request.request_id)),
            }
        }));
        let mut inbox_alert_rows = blocked_unacked
            .iter()
            .map(|tool| RegistryOverviewPolicyInboxJson {
                kind: "blocked_tool".to_owned(),
                target: tool.tool_id.clone(),
                owner: tool.owner.clone(),
                requested_by: None,
            })
            .collect::<Vec<_>>();
        inbox_alert_rows.extend(overdue_unacked.iter().map(|request| {
            RegistryOverviewPolicyInboxJson {
                kind: "overdue_request".to_owned(),
                target: request.request_id.clone(),
                owner: request.owner.clone(),
                requested_by: Some(request.requested_by.clone()),
            }
        }));

        let (_, audits) = match load_evolution_audits(root) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("failed to load policy audits for registry overview: {error}");
                return ExitCode::from(1);
            }
        };
        let mut tool_policy_audits = audits
            .into_iter()
            .filter(|audit| {
                matches!(
                    audit.action.as_str(),
                    "tool_register"
                        | "tool_request_shell"
                        | "tool_authorize_shell"
                        | "tool_revoke_shell"
                )
            })
            .collect::<Vec<_>>();
        if tool_policy_audits.len() > 5 {
            let start = tool_policy_audits.len() - 5;
            tool_policy_audits = tool_policy_audits.split_off(start);
        }
        let recent_change_rows = tool_policy_audits
            .into_iter()
            .map(|audit| RegistryOverviewPolicyRecentChangeJson {
                timestamp: audit.timestamp,
                action: audit.action,
                tool_id: audit.target_id,
                result: audit.result,
                detail: audit.payload,
            })
            .collect::<Vec<_>>();

        Some(RegistryOverviewPolicyJson {
            tool_count: tools.len(),
            shell_tool_count: shell_tools.len(),
            shell_tool_allowed_count: shell_tools_allowed,
            shell_tool_blocked_count: shell_tools_blocked,
            shell_tool_pending_count: shell_tools_pending,
            shell_request_count: approval_requests.len(),
            shell_request_pending_count: pending_requests.len(),
            shell_request_overdue_count: overdue_requests.len(),
            alert_count: shell_tools.iter().filter(|tool| !tool.allow_shell).count()
                + overdue_requests.len(),
            unacked_alert_count: blocked_unacked.len() + overdue_unacked.len(),
            acked_alert_count: acked_ids.len(),
            inbox_blocked_tool_count: blocked_unacked.len(),
            inbox_overdue_request_count: overdue_unacked.len(),
            inbox_count: blocked_unacked.len() + overdue_unacked.len(),
            inbox_owner_count: inbox_owner_rows.len(),
            shell_request_pending_owner_count: pending_request_owner_rows.len(),
            shell_request_pending_age_bucket_count: pending_request_age_rows.len(),
            recent_change_count: recent_change_rows.len(),
            shell_tools: shell_tool_rows,
            inbox_owners: inbox_owner_rows,
            pending_request_owners: pending_request_owner_rows,
            pending_request_age_buckets: pending_request_age_rows,
            pending_requests: pending_request_rows,
            alert_statuses: alert_status_rows,
            inbox_alerts: inbox_alert_rows,
            recent_changes: recent_change_rows,
        })
    } else {
        None
    };

    let gaps = if with_gaps {
        let unrecommended_skills = skills
            .iter()
            .filter(|skill| skill.recommended_implementation_id.is_none())
            .map(|skill| skill.skill_id.clone())
            .collect::<Vec<_>>();
        let mut unbound_tasks_no_skill = Vec::new();
        let mut unbound_tasks_missing_recommendation = Vec::new();
        for task in &tasks {
            if runtime_task_implementation_id(task).is_some() {
                continue;
            }
            if task.task_spec.skill_refs.is_empty() {
                unbound_tasks_no_skill.push(task.task_spec.task_id.clone());
                continue;
            }
            let has_recommended_skill = task.task_spec.skill_refs.iter().any(|skill_ref| {
                skills.iter().any(|skill| {
                    skill.skill_id == *skill_ref && skill.recommended_implementation_id.is_some()
                })
            });
            if has_recommended_skill {
                continue;
            }
            unbound_tasks_missing_recommendation.push(task.task_spec.task_id.clone());
        }
        let blocked_shell_tools = tools
            .iter()
            .filter(|tool| tool.entrypoint.starts_with("shell://") && !tool.allow_shell)
            .collect::<Vec<_>>();
        let pending_shell_tools = blocked_shell_tools
            .iter()
            .filter(|tool| tool.shell_approval_pending)
            .map(|tool| tool.tool_id.clone())
            .collect::<Vec<_>>();
        let blocked_shell_tool_rows = blocked_shell_tools
            .iter()
            .map(|tool| {
                let trust_tier = match tool.owner.as_str() {
                    "system" => "system",
                    "tenant-local" => "trusted_local",
                    _ => "tenant",
                };
                RegistryOverviewGapBlockedShellToolJson {
                    tool_id: tool.tool_id.clone(),
                    owner: tool.owner.clone(),
                    scheme: "shell".to_owned(),
                    trust_tier: trust_tier.to_owned(),
                    allow_shell: false,
                    pending: tool.shell_approval_pending,
                }
            })
            .collect::<Vec<_>>();
        Some(RegistryOverviewGapsJson {
            skill_without_recommendation_count: unrecommended_skills.len(),
            skill_without_recommendation: unrecommended_skills,
            task_without_implementation_no_skill_count: unbound_tasks_no_skill.len(),
            task_without_implementation_no_skill: unbound_tasks_no_skill,
            task_without_implementation_missing_recommendation_count:
                unbound_tasks_missing_recommendation.len(),
            task_without_implementation_missing_recommendation:
                unbound_tasks_missing_recommendation,
            blocked_shell_tool_count: blocked_shell_tool_rows.len(),
            blocked_shell_tools: blocked_shell_tool_rows,
            pending_shell_tool_count: pending_shell_tools.len(),
            pending_shell_tools,
        })
    } else {
        None
    };

    let details = if with_details {
        let implementation_context = summarize_implementation_usage(root, &skills, &tasks);
        let global_policy = load_governance_defaults(root)
            .ok()
            .map(|(_, defaults)| defaults.governance_policy)
            .unwrap_or_default();
        let skill_policy = skills
            .iter()
            .map(|skill| (skill.skill_id.clone(), skill.governance_policy.clone()))
            .collect::<std::collections::BTreeMap<_, _>>();
        let implementation_guardrails = match summarize_guardrail_implementation_audits(
            root,
            None,
            Some(30 * 24 * 60 * 60 * 1000),
        ) {
            Ok(value) => value,
            Err(error) => {
                eprintln!(
                    "failed to summarize implementation guardrails for registry overview: {error}"
                );
                return ExitCode::from(1);
            }
        };
        let recommended_skills = skills
            .iter()
            .filter_map(|skill| {
                skill
                    .recommended_implementation_id
                    .as_ref()
                    .map(|implementation| RegistryOverviewRecommendedSkillJson {
                        skill_id: skill.skill_id.clone(),
                        implementation_id: implementation.clone(),
                        decision: skill
                            .governance_decision
                            .map(|decision| decision.as_str().to_owned())
                            .unwrap_or_else(|| "<none>".to_owned()),
                    })
            })
            .collect::<Vec<_>>();
        let mut implementation_signal_rows = implementations
            .iter()
            .map(|record| {
                let governed = GovernedImplementation::from_record(record);
                let flags = implementation_governance_flags(&governed);
                RegistryOverviewImplementationSignalJson {
                    implementation_id: record.implementation_id.clone(),
                    skill_id: record.skill_id.clone(),
                    executor: record.executor.clone(),
                    mode: governed
                        .strategy_mode
                        .unwrap_or_else(|| "<none>".to_owned()),
                    max_cost: governed.max_cost.unwrap_or_else(|| "<none>".to_owned()),
                    max_latency_ms: governed
                        .max_latency_ms
                        .unwrap_or_else(|| "<none>".to_owned()),
                    flags,
                }
            })
            .collect::<Vec<_>>();
        implementation_signal_rows.sort_by(|a, b| {
            b.flags
                .len()
                .cmp(&a.flags.len())
                .then_with(|| a.implementation_id.cmp(&b.implementation_id))
        });
        let mut implementation_flag_counts = std::collections::BTreeMap::<String, usize>::new();
        for row in &implementation_signal_rows {
            for flag in &row.flags {
                *implementation_flag_counts.entry(flag.clone()).or_insert(0) += 1;
            }
        }
        let implementation_flag_rows = implementation_flag_counts
            .into_iter()
            .map(|(flag, count)| RegistryOverviewImplementationFlagCountJson { flag, count })
            .collect::<Vec<_>>();
        let mut implementation_hotspot_rows = implementations
            .iter()
            .filter_map(|record| {
                let usage = implementation_context
                    .get(&record.implementation_id)
                    .cloned()
                    .unwrap_or_default();
                let guardrail = implementation_guardrails
                    .get(&record.implementation_id)
                    .cloned()
                    .unwrap_or_default();
                if guardrail.recent_guardrail_block_count == 0 {
                    return None;
                }
                if usage.recommended_by_skill_count == 0
                    && usage.active_task_count == 0
                    && usage.runtime_assignment_count == 0
                    && usage.execution_count == 0
                {
                    return None;
                }
                let governed = GovernedImplementation::from_record(record);
                let skill_policy = skill_policy.get(&record.skill_id);
                let (refresh_min_absolute_increase, refresh_min_absolute_increase_source) =
                    resolve_usize_setting_with_source(
                        &record.constraints,
                        skill_policy,
                        Some(&global_policy),
                        "review_refresh_min_absolute_increase",
                        3,
                    );
                let (refresh_min_multiplier, refresh_min_multiplier_source) =
                    resolve_f64_setting_with_source(
                        &record.constraints,
                        skill_policy,
                        Some(&global_policy),
                        "review_refresh_min_multiplier",
                        2.0,
                    );
                let (refresh_min_severity_delta, refresh_min_severity_delta_source) =
                    resolve_usize_setting_with_source(
                        &record.constraints,
                        skill_policy,
                        Some(&global_policy),
                        "review_refresh_min_severity_delta",
                        3,
                    );
                let (severity_weight_recommended_by, severity_weight_recommended_by_source) =
                    resolve_usize_setting_with_source(
                        &record.constraints,
                        skill_policy,
                        Some(&global_policy),
                        "review_severity_weight_recommended_by",
                        2,
                    );
                let (severity_weight_active_tasks, severity_weight_active_tasks_source) =
                    resolve_usize_setting_with_source(
                        &record.constraints,
                        skill_policy,
                        Some(&global_policy),
                        "review_severity_weight_active_tasks",
                        2,
                    );
                let (
                    severity_weight_runtime_assignments,
                    severity_weight_runtime_assignments_source,
                ) = resolve_usize_setting_with_source(
                    &record.constraints,
                    skill_policy,
                    Some(&global_policy),
                    "review_severity_weight_runtime_assignments",
                    1,
                );
                let (severity_weight_executions, severity_weight_executions_source) =
                    resolve_usize_setting_with_source(
                        &record.constraints,
                        skill_policy,
                        Some(&global_policy),
                        "review_severity_weight_executions",
                        1,
                    );
                let (severity_weight_severe_flags, severity_weight_severe_flags_source) =
                    resolve_usize_setting_with_source(
                        &record.constraints,
                        skill_policy,
                        Some(&global_policy),
                        "review_severity_weight_severe_flags",
                        1,
                    );
                Some(RegistryOverviewImplementationHotspotJson {
                    implementation_id: record.implementation_id.clone(),
                    skill_id: record.skill_id.clone(),
                    executor: record.executor.clone(),
                    recent_guardrail_block_count: guardrail.recent_guardrail_block_count,
                    top_reason: guardrail.top_reason.unwrap_or_else(|| "<none>".to_owned()),
                    recommended_by_skill_count: usage.recommended_by_skill_count,
                    runtime_task_count: usage.runtime_task_count,
                    active_task_count: usage.active_task_count,
                    runtime_assignment_count: usage.runtime_assignment_count,
                    execution_count: usage.execution_count,
                    flags: implementation_governance_flags(&governed),
                    refresh_min_absolute_increase,
                    refresh_min_multiplier,
                    refresh_min_severity_delta,
                    severity_weight_recommended_by,
                    severity_weight_active_tasks,
                    severity_weight_runtime_assignments,
                    severity_weight_executions,
                    severity_weight_severe_flags,
                    refresh_min_absolute_increase_source: refresh_min_absolute_increase_source
                        .to_owned(),
                    refresh_min_multiplier_source: refresh_min_multiplier_source.to_owned(),
                    refresh_min_severity_delta_source: refresh_min_severity_delta_source.to_owned(),
                    severity_weight_recommended_by_source: severity_weight_recommended_by_source
                        .to_owned(),
                    severity_weight_active_tasks_source: severity_weight_active_tasks_source
                        .to_owned(),
                    severity_weight_runtime_assignments_source:
                        severity_weight_runtime_assignments_source.to_owned(),
                    severity_weight_executions_source: severity_weight_executions_source
                        .to_owned(),
                    severity_weight_severe_flags_source: severity_weight_severe_flags_source
                        .to_owned(),
                })
            })
            .collect::<Vec<_>>();
        implementation_hotspot_rows.sort_by(|a, b| {
            b.recent_guardrail_block_count
                .cmp(&a.recent_guardrail_block_count)
                .then_with(|| b.execution_count.cmp(&a.execution_count))
                .then_with(|| b.runtime_assignment_count.cmp(&a.runtime_assignment_count))
                .then_with(|| {
                    b.recommended_by_skill_count
                        .cmp(&a.recommended_by_skill_count)
                })
                .then_with(|| b.active_task_count.cmp(&a.active_task_count))
                .then_with(|| a.implementation_id.cmp(&b.implementation_id))
        });
        let mut implementation_usage_rows = implementation_usage
            .iter()
            .map(|(id, _count)| {
                let usage = implementation_context.get(id).cloned().unwrap_or_default();
                RegistryOverviewImplementationUsageJson {
                    implementation_id: id.clone(),
                    recommended_by_skill_count: usage.recommended_by_skill_count,
                    runtime_task_count: usage.runtime_task_count,
                    active_task_count: usage.active_task_count,
                    runtime_assignment_count: usage.runtime_assignment_count,
                    execution_count: usage.execution_count,
                }
            })
            .collect::<Vec<_>>();
        implementation_usage_rows.sort_by(|a, b| {
            b.runtime_task_count
                .cmp(&a.runtime_task_count)
                .then_with(|| b.runtime_assignment_count.cmp(&a.runtime_assignment_count))
                .then_with(|| b.execution_count.cmp(&a.execution_count))
                .then_with(|| {
                    b.recommended_by_skill_count
                        .cmp(&a.recommended_by_skill_count)
                })
                .then_with(|| a.implementation_id.cmp(&b.implementation_id))
        });
        let mut skill_usage_rows = skill_usage
            .iter()
            .map(|(id, count)| RegistryOverviewCountJson {
                id: id.clone(),
                task_count: *count,
            })
            .collect::<Vec<_>>();
        skill_usage_rows.sort_by(|a, b| {
            b.task_count
                .cmp(&a.task_count)
                .then_with(|| a.id.cmp(&b.id))
        });
        let mut tool_usage_rows = tool_usage
            .iter()
            .map(|(id, count)| RegistryOverviewCountJson {
                id: id.clone(),
                task_count: *count,
            })
            .collect::<Vec<_>>();
        tool_usage_rows.sort_by(|a, b| {
            b.task_count
                .cmp(&a.task_count)
                .then_with(|| a.id.cmp(&b.id))
        });

        Some(RegistryOverviewDetailsJson {
            governance_defaults,
            recommended_skill_detail_count: recommended_skills.len(),
            recommended_skills,
            implementation_usage_detail_count: implementation_usage_rows.len(),
            implementation_usage: implementation_usage_rows,
            implementation_signal_detail_count: implementation_signal_rows.len(),
            implementation_signals: implementation_signal_rows,
            implementation_flag_count: implementation_flag_rows.len(),
            implementation_flags: implementation_flag_rows,
            implementation_hotspot_detail_count: implementation_hotspot_rows.len(),
            implementation_hotspots: implementation_hotspot_rows,
            skill_usage_detail_count: skill_usage_rows.len(),
            skill_usage: skill_usage_rows,
            tool_usage_detail_count: tool_usage_rows.len(),
            tool_usage: tool_usage_rows,
        })
    } else {
        None
    };

    if as_json {
        let output = RegistryOverviewJson {
            skills_dir: skills_dir.display().to_string(),
            implementations_dir: implementations_dir.display().to_string(),
            tools_dir: tools_dir.display().to_string(),
            fitness_dir: fitness_dir.display().to_string(),
            tasks_dir: tasks_dir.display().to_string(),
            exclude_legacy,
            skill_count: skills.len(),
            skill_with_recommendation_count: recommended_skill_count,
            implementation_count: implementations.len(),
            tool_count: tools.len(),
            fitness_count: fitness_runs.len(),
            fitness_promote_count: promoted_count,
            fitness_hold_count: hold_count,
            fitness_observe_count: observe_count,
            fitness_deprecate_count: deprecated_count,
            task_count,
            completed_task_count,
            implementation_bound_task_count,
            assignment_count,
            completed_assignment_count,
            policy,
            gaps,
            details,
        };
        match serde_json::to_string_pretty(&output) {
            Ok(json) => println!("{json}"),
            Err(error) => {
                eprintln!("failed to serialize registry overview json: {error}");
                return ExitCode::from(1);
            }
        }
        return ExitCode::SUCCESS;
    }

    println!("registry overview loaded");
    println!("  skills_dir: {}", skills_dir.display());
    println!("  implementations_dir: {}", implementations_dir.display());
    println!("  tools_dir: {}", tools_dir.display());
    println!("  fitness_dir: {}", fitness_dir.display());
    println!("  tasks_dir: {}", tasks_dir.display());
    println!(
        "  exclude_legacy: {}",
        if exclude_legacy { "true" } else { "false" }
    );
    println!("  skill_count: {}", skills.len());
    println!(
        "  skill_with_recommendation_count: {}",
        recommended_skill_count
    );
    println!("  implementation_count: {}", implementations.len());
    println!("  tool_count: {}", tools.len());
    println!("  fitness_count: {}", fitness_runs.len());
    println!("  fitness_promote_count: {}", promoted_count);
    println!("  fitness_hold_count: {}", hold_count);
    println!("  fitness_observe_count: {}", observe_count);
    println!("  fitness_deprecate_count: {}", deprecated_count);
    println!("  task_count: {}", task_count);
    println!("  completed_task_count: {}", completed_task_count);
    println!(
        "  implementation_bound_task_count: {}",
        implementation_bound_task_count
    );
    println!("  assignment_count: {}", assignment_count);
    println!(
        "  completed_assignment_count: {}",
        completed_assignment_count
    );

    if let Some(policy) = &policy {
        println!("  policy_tool_count: {}", policy.tool_count);
        println!("  policy_shell_tool_count: {}", policy.shell_tool_count);
        println!(
            "  policy_shell_tool_allowed_count: {}",
            policy.shell_tool_allowed_count
        );
        println!(
            "  policy_shell_tool_blocked_count: {}",
            policy.shell_tool_blocked_count
        );
        for tool in &policy.shell_tools {
            println!(
                "  policy_shell_tool: tool={} owner={} scheme={} trust_tier={} allow_shell={} pending={}",
                tool.tool_id,
                tool.owner,
                tool.scheme,
                tool.trust_tier,
                if tool.allow_shell { "true" } else { "false" },
                if tool.pending { "true" } else { "false" }
            );
        }
        println!(
            "  policy_shell_tool_pending_count: {}",
            policy.shell_tool_pending_count
        );
        println!(
            "  policy_shell_request_count: {}",
            policy.shell_request_count
        );
        println!(
            "  policy_shell_request_pending_count: {}",
            policy.shell_request_pending_count
        );
        println!(
            "  policy_shell_request_overdue_count: {}",
            policy.shell_request_overdue_count
        );
        println!("  policy_alert_count: {}", policy.alert_count);
        println!(
            "  policy_unacked_alert_count: {}",
            policy.unacked_alert_count
        );
        println!("  policy_acked_alert_count: {}", policy.acked_alert_count);
        println!(
            "  policy_inbox_blocked_tool_count: {}",
            policy.inbox_blocked_tool_count
        );
        println!(
            "  policy_inbox_overdue_request_count: {}",
            policy.inbox_overdue_request_count
        );
        println!("  policy_inbox_count: {}", policy.inbox_count);
        println!("  policy_inbox_owner_count: {}", policy.inbox_owner_count);
        for row in &policy.inbox_owners {
            println!(
                "  policy_inbox_owner: owner={} count={}",
                row.owner, row.count
            );
        }
        println!(
            "  policy_shell_request_pending_owner_count: {}",
            policy.shell_request_pending_owner_count
        );
        for row in &policy.pending_request_owners {
            println!(
                "  policy_shell_request_pending_owner: owner={} count={}",
                row.owner, row.count
            );
        }
        println!(
            "  policy_shell_request_pending_age_bucket_count: {}",
            policy.shell_request_pending_age_bucket_count
        );
        for row in &policy.pending_request_age_buckets {
            println!(
                "  policy_shell_request_pending_age_bucket: bucket={} count={}",
                row.bucket, row.count
            );
        }
        for row in &policy.pending_requests {
            println!(
                "  policy_shell_request_pending: request={} tool={} owner={} requested_by={} requested_at={} age_ms={} age_bucket={}",
                row.request_id,
                row.tool_id,
                row.owner,
                row.requested_by,
                row.requested_at,
                row.age_ms
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "<unknown>".to_owned()),
                row.age_bucket
            );
        }
        for row in &policy.alert_statuses {
            println!(
                "  policy_alert_status: kind={} target={} acked={}",
                row.kind,
                row.target,
                if row.acked { "true" } else { "false" }
            );
        }
        for row in &policy.inbox_alerts {
            if let Some(requested_by) = &row.requested_by {
                println!(
                    "  policy_inbox: kind={} target={} owner={} requested_by={}",
                    row.kind, row.target, row.owner, requested_by
                );
            } else {
                println!(
                    "  policy_inbox: kind={} target={} owner={}",
                    row.kind, row.target, row.owner
                );
            }
        }
        println!(
            "  policy_recent_change_count: {}",
            policy.recent_change_count
        );
        for row in &policy.recent_changes {
            println!(
                "  policy_recent_change: ts={} action={} tool={} result={} detail={}",
                row.timestamp, row.action, row.tool_id, row.result, row.detail
            );
        }
    }

    if let Some(gaps) = &gaps {
        println!(
            "  gap_skill_without_recommendation_count: {}",
            gaps.skill_without_recommendation_count
        );
        for skill_id in &gaps.skill_without_recommendation {
            println!("  gap_skill_without_recommendation: skill={skill_id}");
        }
        println!(
            "  gap_task_without_implementation_no_skill_count: {}",
            gaps.task_without_implementation_no_skill_count
        );
        for task_id in &gaps.task_without_implementation_no_skill {
            println!("  gap_task_without_implementation_no_skill: task={task_id}");
        }
        println!(
            "  gap_task_without_implementation_missing_recommendation_count: {}",
            gaps.task_without_implementation_missing_recommendation_count
        );
        for task_id in &gaps.task_without_implementation_missing_recommendation {
            println!("  gap_task_without_implementation_missing_recommendation: task={task_id}");
        }
        println!(
            "  gap_blocked_shell_tool_count: {}",
            gaps.blocked_shell_tool_count
        );
        for tool in &gaps.blocked_shell_tools {
            println!(
                "  gap_blocked_shell_tool: tool={} owner={} scheme=shell trust_tier={} allow_shell=false pending={}",
                tool.tool_id,
                tool.owner,
                tool.trust_tier,
                if tool.pending { "true" } else { "false" }
            );
        }
        println!(
            "  gap_pending_shell_tool_count: {}",
            gaps.pending_shell_tool_count
        );
        for tool in &gaps.pending_shell_tools {
            println!("  gap_pending_shell_tool: tool={tool}");
        }
    }

    if let Some(details) = &details {
        println!(
            "  recommended_skill_detail_count: {}",
            details.recommended_skill_detail_count
        );
        for row in &details.recommended_skills {
            println!(
                "  recommended_skill: skill={} implementation={} decision={}",
                row.skill_id, row.implementation_id, row.decision
            );
        }

        println!(
            "  implementation_usage_detail_count: {}",
            details.implementation_usage_detail_count
        );
        println!(
            "  governance_defaults_loaded: {}",
            details.governance_defaults.loaded
        );
        println!(
            "  governance_defaults_policy_count: {}",
            details.governance_defaults.policy_count
        );
        println!(
            "  governance_defaults_updated_at: {}",
            details
                .governance_defaults
                .updated_at
                .as_deref()
                .unwrap_or("<none>")
        );
        println!(
            "  governance_defaults_loaded_from: {}",
            details
                .governance_defaults
                .loaded_from
                .as_deref()
                .unwrap_or("<none>")
        );
        for row in &details.governance_defaults.policies {
            println!("  governance_default: {}={}", row.key, row.value);
        }
        for row in &details.implementation_usage {
            println!(
                "  implementation_usage: implementation={} recommended_by={} runtime_tasks={} active_tasks={} assignments={} executions={}",
                row.implementation_id,
                row.recommended_by_skill_count,
                row.runtime_task_count,
                row.active_task_count,
                row.runtime_assignment_count,
                row.execution_count
            );
        }
        println!(
            "  implementation_signal_detail_count: {}",
            details.implementation_signal_detail_count
        );
        for row in &details.implementation_signals {
            println!(
                "  implementation_signal: implementation={} skill={} executor={} mode={} max_cost={} max_latency_ms={} flags={}",
                row.implementation_id,
                row.skill_id,
                row.executor,
                row.mode,
                row.max_cost,
                row.max_latency_ms,
                if row.flags.is_empty() {
                    "<none>".to_owned()
                } else {
                    row.flags.join(", ")
                }
            );
        }
        println!(
            "  implementation_flag_count: {}",
            details.implementation_flag_count
        );
        for row in &details.implementation_flags {
            println!(
                "  implementation_flag: flag={} count={}",
                row.flag, row.count
            );
        }
        println!(
            "  implementation_hotspot_detail_count: {}",
            details.implementation_hotspot_detail_count
        );
        for row in &details.implementation_hotspots {
            println!(
                "  implementation_hotspot: implementation={} skill={} executor={} guardrails={} top_reason={} recommended_by={} runtime_tasks={} active_tasks={} assignments={} executions={} flags={} refresh_abs={}({}) refresh_multiplier={}({}) refresh_severity_delta={}({}) weight_recommended_by={}({}) weight_active_tasks={}({}) weight_runtime_assignments={}({}) weight_executions={}({}) weight_severe_flags={}({})",
                row.implementation_id,
                row.skill_id,
                row.executor,
                row.recent_guardrail_block_count,
                row.top_reason,
                row.recommended_by_skill_count,
                row.runtime_task_count,
                row.active_task_count,
                row.runtime_assignment_count,
                row.execution_count,
                if row.flags.is_empty() {
                    "<none>".to_owned()
                } else {
                    row.flags.join(", ")
                },
                row.refresh_min_absolute_increase,
                row.refresh_min_absolute_increase_source,
                row.refresh_min_multiplier,
                row.refresh_min_multiplier_source,
                row.refresh_min_severity_delta,
                row.refresh_min_severity_delta_source,
                row.severity_weight_recommended_by,
                row.severity_weight_recommended_by_source,
                row.severity_weight_active_tasks,
                row.severity_weight_active_tasks_source,
                row.severity_weight_runtime_assignments,
                row.severity_weight_runtime_assignments_source,
                row.severity_weight_executions,
                row.severity_weight_executions_source,
                row.severity_weight_severe_flags,
                row.severity_weight_severe_flags_source
            );
        }
        println!(
            "  skill_usage_detail_count: {}",
            details.skill_usage_detail_count
        );
        for row in &details.skill_usage {
            println!(
                "  skill_usage: skill={} task_count={}",
                row.id, row.task_count
            );
        }
        println!(
            "  tool_usage_detail_count: {}",
            details.tool_usage_detail_count
        );
        for row in &details.tool_usage {
            println!(
                "  tool_usage: tool={} task_count={}",
                row.id, row.task_count
            );
        }
    }

    ExitCode::SUCCESS
}
