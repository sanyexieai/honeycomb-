use super::*;

pub(crate) fn tool_policy_summary(tool: &ToolRecord) -> String {
    format!(
        "scheme={} trust_tier={} allow_shell={} pending={}",
        super::common_support::entrypoint_scheme(&tool.entrypoint),
        super::common_support::owner_trust_tier(&tool.owner),
        if tool.allow_shell { "true" } else { "false" },
        if tool.shell_approval_pending {
            "true"
        } else {
            "false"
        }
    )
}

pub(crate) fn recent_tool_policy_audits(
    root: &str,
    limit: usize,
) -> std::io::Result<Vec<AuditRecord>> {
    let (_, audits) = load_evolution_audits(root)?;
    let mut filtered = audits
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
    if filtered.len() > limit {
        let start = filtered.len() - limit;
        filtered = filtered.split_off(start);
    }
    Ok(filtered)
}

pub(crate) fn is_shell_tool(tool: &ToolRecord) -> bool {
    super::common_support::entrypoint_scheme(&tool.entrypoint) == "shell"
}

pub(crate) fn ensure_shell_execution_allowed(tool: &ToolRecord) -> std::io::Result<()> {
    if tool.entrypoint.starts_with("shell://") && !tool.allow_shell {
        return Err(std::io::Error::other("tool_shell_execution_not_allowed"));
    }

    Ok(())
}

pub(crate) fn shell_request_status(status: &str) -> Option<ApprovalRequestStatus> {
    match status {
        "pending" => Some(ApprovalRequestStatus::Pending),
        "approved" => Some(ApprovalRequestStatus::Approved),
        "rejected" => Some(ApprovalRequestStatus::Rejected),
        _ => None,
    }
}

pub(crate) fn approval_request_age_ms(request: &ShellApprovalRequest) -> Option<u128> {
    let now = crate::core::parse_unix_ms_timestamp(&crate::core::current_timestamp())?;
    let requested_at = crate::core::parse_unix_ms_timestamp(&request.requested_at)?;
    Some(now.saturating_sub(requested_at))
}

pub(crate) fn approval_age_bucket(age_ms: Option<u128>) -> &'static str {
    match age_ms {
        Some(age) if age < 5 * 60 * 1000 => "fresh",
        Some(age) if age < 60 * 60 * 1000 => "stale",
        Some(_) => "overdue",
        None => "unknown",
    }
}

pub(crate) fn overdue_threshold_ms(args: &[String]) -> u128 {
    option_value(args, "--threshold-minutes")
        .and_then(|value| value.parse::<u128>().ok())
        .unwrap_or(60)
        .saturating_mul(60_000)
}
