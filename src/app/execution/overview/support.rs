use super::super::common_support::*;
use super::super::control::*;
use super::super::*;

#[derive(Serialize)]
pub(crate) struct AlertBlockedToolView {
    pub(crate) tool_id: String,
    pub(crate) owner: String,
    pub(crate) policy: String,
}

#[derive(Serialize)]
pub(crate) struct AlertOverdueRequestView {
    pub(crate) request_id: String,
    pub(crate) tool_id: String,
    pub(crate) owner: String,
    pub(crate) requested_by: String,
    pub(crate) age_ms: Option<u128>,
    pub(crate) age_bucket: String,
}

#[derive(Serialize)]
pub(crate) struct AlertOwnerSummary {
    pub(crate) owner: String,
    pub(crate) count: usize,
}

#[derive(Serialize)]
pub(crate) struct ApprovalAlertsJson {
    pub(crate) tool_id: Option<String>,
    pub(crate) owner: Option<String>,
    pub(crate) threshold_minutes: u128,
    pub(crate) include_acked: bool,
    pub(crate) blocked_shell_tool_count: usize,
    pub(crate) overdue_request_count: usize,
    pub(crate) alert_count: usize,
    pub(crate) inbox_owners: Vec<AlertOwnerSummary>,
    pub(crate) blocked_tools: Vec<AlertBlockedToolView>,
    pub(crate) overdue_requests: Vec<AlertOverdueRequestView>,
}

#[derive(Serialize)]
pub(crate) struct RuntimeOverviewUsageJson {
    pub(crate) id: String,
    pub(crate) count: usize,
}

#[derive(Serialize)]
pub(crate) struct RuntimeOverviewImplementationUsageJson {
    pub(crate) implementation_id: String,
    pub(crate) runtime_task_count: usize,
    pub(crate) active_task_count: usize,
    pub(crate) runtime_assignment_count: usize,
    pub(crate) execution_count: usize,
}

#[derive(Clone, Serialize)]
pub(crate) struct RuntimeOverviewActiveTaskJson {
    pub(crate) task_id: String,
    pub(crate) status: String,
    pub(crate) reason: String,
    pub(crate) skills: String,
    pub(crate) tools: String,
    pub(crate) assignment_total: usize,
    pub(crate) assignment_active: usize,
    pub(crate) resident_total: usize,
    pub(crate) resident_running: usize,
    pub(crate) trigger_total: usize,
    pub(crate) trigger_active: usize,
}

#[derive(Serialize)]
pub(crate) struct RuntimeOverviewPolicyShellToolJson {
    pub(crate) tool_id: String,
    pub(crate) owner: String,
    pub(crate) policy: String,
}

#[derive(Serialize)]
pub(crate) struct RuntimeOverviewPolicyRecentChangeJson {
    pub(crate) timestamp: String,
    pub(crate) action: String,
    pub(crate) tool_id: String,
    pub(crate) result: String,
    pub(crate) detail: String,
}

#[derive(Serialize)]
pub(crate) struct RuntimeOverviewGapTaskJson {
    pub(crate) task_id: String,
    pub(crate) status: String,
    pub(crate) goal: String,
    pub(crate) skills: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct RuntimeOverviewPolicyJson {
    pub(crate) tool_count: usize,
    pub(crate) shell_tool_count: usize,
    pub(crate) shell_tool_allowed_count: usize,
    pub(crate) shell_tool_pending_count: usize,
    pub(crate) shell_tool_blocked_count: usize,
    pub(crate) shell_request_count: usize,
    pub(crate) shell_request_pending_count: usize,
    pub(crate) recent_change_count: usize,
    pub(crate) shell_tools: Vec<RuntimeOverviewPolicyShellToolJson>,
    pub(crate) recent_changes: Vec<RuntimeOverviewPolicyRecentChangeJson>,
}

#[derive(Serialize)]
pub(crate) struct RuntimeOverviewGapsJson {
    pub(crate) task_without_implementation_no_skill_count: usize,
    pub(crate) task_without_implementation_no_skill: Vec<RuntimeOverviewGapTaskJson>,
    pub(crate) task_without_implementation_missing_recommendation_count: usize,
    pub(crate) task_without_implementation_missing_recommendation: Vec<RuntimeOverviewGapTaskJson>,
    pub(crate) active_task_count: usize,
    pub(crate) active_task_reason_count: usize,
    pub(crate) active_task_reasons: Vec<RuntimeOverviewUsageJson>,
    pub(crate) active_tasks: Vec<RuntimeOverviewActiveTaskJson>,
    pub(crate) blocked_shell_tool_count: usize,
    pub(crate) blocked_shell_tools: Vec<RuntimeOverviewPolicyShellToolJson>,
    pub(crate) pending_shell_tool_count: usize,
    pub(crate) pending_shell_tools: Vec<RuntimeOverviewPolicyShellToolJson>,
    pub(crate) trigger_waiting_consumption_count: usize,
    pub(crate) trigger_waiting_consumption: Vec<RuntimeOverviewUsageJson>,
}

#[derive(Serialize)]
pub(crate) struct RuntimeOverviewDetailsJson {
    pub(crate) implementation_usage_detail_count: usize,
    pub(crate) implementation_usage: Vec<RuntimeOverviewImplementationUsageJson>,
    pub(crate) task_status_detail_count: usize,
    pub(crate) task_statuses: Vec<RuntimeOverviewUsageJson>,
    pub(crate) active_task_reason_detail_count: usize,
    pub(crate) active_task_reasons: Vec<RuntimeOverviewUsageJson>,
    pub(crate) assignment_status_detail_count: usize,
    pub(crate) assignment_statuses: Vec<RuntimeOverviewUsageJson>,
    pub(crate) resident_status_detail_count: usize,
    pub(crate) resident_statuses: Vec<RuntimeOverviewUsageJson>,
    pub(crate) trigger_status_detail_count: usize,
    pub(crate) trigger_statuses: Vec<RuntimeOverviewUsageJson>,
    pub(crate) trigger_consumption_detail_count: usize,
    pub(crate) trigger_consumption: Vec<RuntimeOverviewUsageJson>,
}

#[derive(Serialize)]
pub(crate) struct RuntimeOverviewJson {
    pub(crate) tasks_dir: String,
    pub(crate) exclude_legacy: bool,
    pub(crate) task_count: usize,
    pub(crate) completed_task_count: usize,
    pub(crate) implementation_bound_task_count: usize,
    pub(crate) assignment_count: usize,
    pub(crate) resident_count: usize,
    pub(crate) trigger_count: usize,
    pub(crate) audit_count: usize,
    pub(crate) trace_count: usize,
    pub(crate) policy: Option<RuntimeOverviewPolicyJson>,
    pub(crate) gaps: Option<RuntimeOverviewGapsJson>,
    pub(crate) details: Option<RuntimeOverviewDetailsJson>,
}

#[derive(Serialize)]
pub(crate) struct SystemOverviewJson {
    pub(crate) root: String,
    pub(crate) owner: Option<String>,
    pub(crate) sort: String,
    pub(crate) limit: Option<usize>,
    pub(crate) summary_only: bool,
    pub(crate) include_acked_policy: bool,
    pub(crate) exclude_legacy: bool,
    pub(crate) registry_skill_count: usize,
    pub(crate) registry_skill_with_recommendation_count: usize,
    pub(crate) registry_tool_count: usize,
    pub(crate) registry_fitness_count: usize,
    pub(crate) runtime_task_count: usize,
    pub(crate) runtime_completed_task_count: usize,
    pub(crate) runtime_implementation_bound_task_count: usize,
    pub(crate) runtime_active_task_count: usize,
    pub(crate) runtime_assignment_count: usize,
    pub(crate) runtime_resident_count: usize,
    pub(crate) runtime_trigger_count: usize,
    pub(crate) runtime_audit_count: usize,
    pub(crate) runtime_trace_count: usize,
    pub(crate) rerun_plan_count: usize,
    pub(crate) rerun_plan_task_count: usize,
    pub(crate) policy_alert_count: usize,
    pub(crate) policy_inbox_count: usize,
    pub(crate) policy_shell_tool_count: usize,
    pub(crate) policy_shell_tool_allowed_count: usize,
    pub(crate) policy_shell_tool_blocked_count: usize,
    pub(crate) alert_summaries: Option<SystemOverviewAlertSummariesJson>,
    pub(crate) runtime_health: Option<SystemOverviewRuntimeHealthJson>,
    pub(crate) gaps: Option<SystemOverviewGapsJson>,
    pub(crate) details: Option<SystemOverviewDetailsJson>,
    pub(crate) policy: Option<SystemOverviewPolicyJson>,
}

#[derive(Serialize)]
pub(crate) struct SystemOverviewGapsJson {
    pub(crate) skill_without_recommendation_count: usize,
    pub(crate) task_without_implementation_no_skill_count: usize,
    pub(crate) task_without_implementation_missing_recommendation_count: usize,
    pub(crate) blocked_shell_tool_count: usize,
    pub(crate) active_task_reason_count: usize,
    pub(crate) active_task_reasons: Vec<RuntimeOverviewUsageJson>,
}

#[derive(Serialize)]
pub(crate) struct SystemOverviewDetailsJson {
    pub(crate) recommended_skill_count: usize,
    pub(crate) recommended_skills: Vec<RegistryOverviewRecommendedSkillJson>,
    pub(crate) implementation_usage_count: usize,
    pub(crate) implementation_usage: Vec<RuntimeOverviewImplementationUsageJson>,
    pub(crate) active_task_count: usize,
    pub(crate) active_tasks: Vec<RuntimeOverviewActiveTaskJson>,
}

#[derive(Serialize)]
pub(crate) struct SystemOverviewPolicyJson {
    pub(crate) shell_request_count: usize,
    pub(crate) shell_request_pending_count: usize,
    pub(crate) shell_request_overdue_count: usize,
    pub(crate) unacked_alert_count: usize,
    pub(crate) acked_alert_count: usize,
    pub(crate) inbox_owner_count: usize,
    pub(crate) shell_tools: Vec<RuntimeOverviewPolicyShellToolJson>,
    pub(crate) inbox_owners: Vec<AlertOwnerSummary>,
    pub(crate) recent_changes: Vec<RuntimeOverviewPolicyRecentChangeJson>,
}

#[derive(Serialize)]
pub(crate) struct SystemOverviewRuntimeHealthJson {
    pub(crate) active_task_count: usize,
    pub(crate) active_task_reason_count: usize,
    pub(crate) active_task_reasons: Vec<RuntimeOverviewUsageJson>,
    pub(crate) trigger_waiting_consumption_count: usize,
    pub(crate) trigger_waiting_consumption: Vec<RuntimeOverviewUsageJson>,
    pub(crate) rerun_plan_task_count: usize,
    pub(crate) rerun_plan_tasks: Vec<RuntimeOverviewUsageJson>,
    pub(crate) shell_request_pending_count: usize,
    pub(crate) shell_request_overdue_count: usize,
    pub(crate) blocked_shell_tool_count: usize,
    pub(crate) unacked_alert_count: usize,
    pub(crate) health_severity: String,
}

#[derive(Serialize)]
pub(crate) struct SystemOverviewAlertSummariesJson {
    pub(crate) by_kind: Vec<SystemAlertSummaryJson>,
    pub(crate) by_owner: Vec<SystemAlertSummaryJson>,
    pub(crate) by_severity: Vec<SystemAlertSummaryJson>,
}

#[derive(Serialize)]
pub(crate) struct SystemAlertJson {
    pub(crate) kind: String,
    pub(crate) severity: String,
    pub(crate) owner: Option<String>,
    pub(crate) target: String,
    pub(crate) detail: String,
}

#[derive(Serialize)]
pub(crate) struct SystemAlertSummaryJson {
    pub(crate) key: String,
    pub(crate) count: usize,
    pub(crate) highest_severity: String,
}

#[derive(Serialize)]
pub(crate) struct SystemAlertsJson {
    pub(crate) root: String,
    pub(crate) kind: Option<String>,
    pub(crate) owner: Option<String>,
    pub(crate) severity: Option<String>,
    pub(crate) summary_by: Option<String>,
    pub(crate) sort: String,
    pub(crate) limit: Option<usize>,
    pub(crate) summary_only: bool,
    pub(crate) include_acked_policy: bool,
    pub(crate) exclude_legacy: bool,
    pub(crate) rerun_plan_count: usize,
    pub(crate) alert_count: usize,
    pub(crate) highest_severity: String,
    pub(crate) summaries: Vec<SystemAlertSummaryJson>,
    pub(crate) alerts: Vec<SystemAlertJson>,
}

#[derive(Serialize)]
pub(crate) struct RegistryOverviewRecommendedSkillJson {
    pub(crate) skill_id: String,
    pub(crate) implementation_id: String,
    pub(crate) decision: String,
}

pub(crate) fn alert_kind_matches(kind_filter: Option<&str>, alert_kind: &str) -> bool {
    kind_filter.is_none_or(|value| value == alert_kind)
}

pub(crate) fn alert_severity_matches(severity_filter: Option<&str>, alert_severity: &str) -> bool {
    severity_filter.is_none_or(|value| value == alert_severity)
}

pub(crate) fn alert_severity_rank(severity: &str) -> usize {
    match severity {
        "warning" => 0,
        "attention" => 1,
        "healthy" => 2,
        _ => 3,
    }
}

pub(crate) fn overview_count_sort(rows: &mut [RuntimeOverviewUsageJson]) {
    rows.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.id.cmp(&b.id)));
}

pub(crate) fn overview_target_sort(rows: &mut [RuntimeOverviewUsageJson]) {
    rows.sort_by(|a, b| a.id.cmp(&b.id).then_with(|| b.count.cmp(&a.count)));
}

pub(crate) fn sort_active_task_rows(rows: &mut [RuntimeOverviewActiveTaskJson], sort: &str) {
    if sort == "target" {
        rows.sort_by(|a, b| a.task_id.cmp(&b.task_id));
    } else {
        rows.sort_by(|a, b| {
            a.reason
                .cmp(&b.reason)
                .then_with(|| a.task_id.cmp(&b.task_id))
        });
    }
}

pub(crate) fn sort_alert_owner_summaries(rows: &mut [AlertOwnerSummary], sort: &str) {
    if sort == "target" {
        rows.sort_by(|a, b| a.owner.cmp(&b.owner).then_with(|| b.count.cmp(&a.count)));
    } else {
        rows.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.owner.cmp(&b.owner)));
    }
}

pub(crate) fn sort_policy_shell_tool_rows(
    rows: &mut [RuntimeOverviewPolicyShellToolJson],
    sort: &str,
) {
    if sort == "target" {
        rows.sort_by(|a, b| {
            a.tool_id
                .cmp(&b.tool_id)
                .then_with(|| a.owner.cmp(&b.owner))
        });
    } else {
        rows.sort_by(|a, b| {
            a.owner
                .cmp(&b.owner)
                .then_with(|| a.tool_id.cmp(&b.tool_id))
        });
    }
}

pub(crate) fn sort_policy_recent_changes(
    rows: &mut [RuntimeOverviewPolicyRecentChangeJson],
    sort: &str,
) {
    if sort == "target" {
        rows.sort_by(|a, b| {
            a.tool_id
                .cmp(&b.tool_id)
                .then_with(|| b.timestamp.cmp(&a.timestamp))
        });
    } else {
        rows.sort_by(|a, b| {
            b.timestamp
                .cmp(&a.timestamp)
                .then_with(|| a.tool_id.cmp(&b.tool_id))
        });
    }
}

pub(crate) fn system_alert_summary_key(alert: &SystemAlertJson, summary_by: &str) -> String {
    match summary_by {
        "kind" => alert.kind.clone(),
        "owner" => alert.owner.clone().unwrap_or_else(|| "<none>".to_owned()),
        "severity" => alert.severity.clone(),
        _ => alert.kind.clone(),
    }
}

pub(crate) fn build_system_alert_summaries(
    alerts: &[SystemAlertJson],
    summary_by: &str,
    sort: &str,
) -> Vec<SystemAlertSummaryJson> {
    let mut grouped = std::collections::BTreeMap::<String, (usize, &'static str)>::new();
    for alert in alerts {
        let key = system_alert_summary_key(alert, summary_by);
        let entry = grouped.entry(key).or_insert((0, "healthy"));
        entry.0 += 1;
        if alert_severity_rank(&alert.severity) < alert_severity_rank(entry.1) {
            entry.1 = match alert.severity.as_str() {
                "warning" => "warning",
                "attention" => "attention",
                _ => "healthy",
            };
        }
    }
    let mut rows = grouped
        .into_iter()
        .map(|(key, (count, severity))| SystemAlertSummaryJson {
            key,
            count,
            highest_severity: severity.to_owned(),
        })
        .collect::<Vec<_>>();
    match sort {
        "target" => rows.sort_by(|a, b| a.key.cmp(&b.key).then_with(|| b.count.cmp(&a.count))),
        _ => rows.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.key.cmp(&b.key))),
    }
    rows
}

pub(crate) fn collect_system_alerts(
    root: &str,
    tasks: &[crate::runtime::TaskRecord],
    tools: &[ToolRecord],
    approval_requests: &[ShellApprovalRequest],
    acked_ids: &std::collections::BTreeSet<String>,
    skill_owners: &std::collections::BTreeMap<String, String>,
    tool_owners: &std::collections::BTreeMap<String, String>,
    owner_filter: Option<&str>,
    kind_filter: Option<&str>,
    severity_filter: Option<&str>,
    include_acked_policy: bool,
) -> std::io::Result<Vec<SystemAlertJson>> {
    let mut alerts = Vec::<SystemAlertJson>::new();

    for task in tasks {
        if !task_matches_owner_filter(task, owner_filter, skill_owners, tool_owners) {
            continue;
        }
        let (_, assignments) = load_task_assignments(root, &task.task_spec.task_id)?;
        let (_, residents) = list_residents(root, &task.task_spec.task_id)?;
        let (_, triggers) = list_triggers(root, &task.task_spec.task_id)?;
        let ready_trigger_count = triggers
            .iter()
            .filter(|trigger| trigger.has_unconsumed_fire())
            .count();
        if ready_trigger_count > 0
            && alert_kind_matches(kind_filter, "trigger_waiting_consumption")
            && alert_severity_matches(severity_filter, "attention")
        {
            alerts.push(SystemAlertJson {
                kind: "trigger_waiting_consumption".to_owned(),
                severity: "attention".to_owned(),
                owner: Some(task.task_spec.tenant_id.clone()),
                target: task.task_spec.task_id.clone(),
                detail: format!(
                    "status={} ready_trigger_count={} skills={} tools={}",
                    task.task_runtime.status.as_str(),
                    ready_trigger_count,
                    joined_or_none(&task.task_spec.skill_refs),
                    joined_or_none(&task.task_spec.tool_refs)
                ),
            });
        }
        if let Some(reason) = classify_active_task(task, &assignments, &residents, &triggers) {
            if !alert_kind_matches(kind_filter, "active_task") {
                continue;
            }
            if !alert_severity_matches(severity_filter, "attention") {
                continue;
            }
            alerts.push(SystemAlertJson {
                kind: "active_task".to_owned(),
                severity: "attention".to_owned(),
                owner: Some(task.task_spec.tenant_id.clone()),
                target: task.task_spec.task_id.clone(),
                detail: format!(
                    "status={} reason={} skills={} tools={}",
                    task.task_runtime.status.as_str(),
                    reason.as_str(),
                    joined_or_none(&task.task_spec.skill_refs),
                    joined_or_none(&task.task_spec.tool_refs)
                ),
            });
        }
    }

    for tool in tools
        .iter()
        .filter(|tool| is_shell_tool(tool) && !tool.allow_shell)
    {
        if owner_filter.is_some_and(|owner| tool.owner != owner) {
            continue;
        }
        if !alert_kind_matches(kind_filter, "blocked_tool") {
            continue;
        }
        if !alert_severity_matches(severity_filter, "warning") {
            continue;
        }
        let alert_id = blocked_tool_alert_id(&tool.tool_id);
        if !include_acked_policy && acked_ids.contains(&alert_id) {
            continue;
        }
        alerts.push(SystemAlertJson {
            kind: "blocked_tool".to_owned(),
            severity: "warning".to_owned(),
            owner: Some(tool.owner.clone()),
            target: tool.tool_id.clone(),
            detail: format!("owner={} policy={}", tool.owner, tool_policy_summary(tool)),
        });
    }

    for request in approval_requests
        .iter()
        .filter(|request| request.status == ApprovalRequestStatus::Pending)
        .filter(|request| approval_request_age_ms(request).is_some_and(|age| age >= 60 * 60 * 1000))
    {
        if owner_filter.is_some_and(|owner| request.owner != owner) {
            continue;
        }
        if !alert_kind_matches(kind_filter, "overdue_request") {
            continue;
        }
        if !alert_severity_matches(severity_filter, "warning") {
            continue;
        }
        let alert_id = overdue_request_alert_id(&request.request_id);
        if !include_acked_policy && acked_ids.contains(&alert_id) {
            continue;
        }
        alerts.push(SystemAlertJson {
            kind: "overdue_request".to_owned(),
            severity: "warning".to_owned(),
            owner: Some(request.owner.clone()),
            target: request.request_id.clone(),
            detail: format!(
                "tool={} owner={} requested_by={} age_bucket={}",
                request.tool_id,
                request.owner,
                request.requested_by,
                approval_age_bucket(approval_request_age_ms(request))
            ),
        });
    }

    Ok(alerts)
}

pub(crate) fn task_matches_owner_filter(
    task: &crate::runtime::TaskRecord,
    owner_filter: Option<&str>,
    skill_owners: &std::collections::BTreeMap<String, String>,
    tool_owners: &std::collections::BTreeMap<String, String>,
) -> bool {
    let Some(owner_filter) = owner_filter else {
        return true;
    };
    if task.task_spec.tenant_id == owner_filter {
        return true;
    }
    if task.task_spec.skill_refs.iter().any(|skill_ref| {
        skill_owners
            .get(skill_ref)
            .is_some_and(|owner| owner == owner_filter)
    }) {
        return true;
    }
    task.task_spec.tool_refs.iter().any(|tool_ref| {
        tool_owners
            .get(tool_ref)
            .is_some_and(|owner| owner == owner_filter)
    })
}

pub(crate) fn blocked_tool_alert_id(tool_id: &str) -> String {
    format!("blocked-tool-{tool_id}")
}

pub(crate) fn overdue_request_alert_id(request_id: &str) -> String {
    format!("overdue-request-{request_id}")
}

pub(crate) fn is_legacy_demo_task(record: &crate::runtime::TaskRecord) -> bool {
    record.task_spec.skill_refs.is_empty()
        && (record.task_spec.task_id.starts_with("task-demo-flow-")
            || record.task_spec.task_id == "task-xhs-demo")
}
