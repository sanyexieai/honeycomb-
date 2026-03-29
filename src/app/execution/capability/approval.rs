use super::super::control::*;
use super::super::overview::support::*;
use super::super::*;

pub(crate) fn handle_tool_approval_inspect(args: &[String]) -> ExitCode {
    let request_id = option_value(args, "--request-id").unwrap_or("shell-approval-demo");
    let root = option_value(args, "--root").unwrap_or(".");

    let (path, request) = match load_shell_approval_request(root, request_id) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to inspect shell approval request: {error}");
            return ExitCode::from(1);
        }
    };

    println!("tool approval-inspect loaded");
    println!("  request_id: {}", request.request_id);
    println!("  tool_id: {}", request.tool_id);
    println!("  owner: {}", request.owner);
    println!("  entrypoint: {}", request.entrypoint);
    println!("  requested_by: {}", request.requested_by);
    println!("  status: {}", request.status.as_str());
    println!("  requested_at: {}", request.requested_at);
    println!(
        "  resolved_at: {}",
        request.resolved_at.as_deref().unwrap_or("<none>")
    );
    println!(
        "  resolved_by: {}",
        request.resolved_by.as_deref().unwrap_or("<none>")
    );
    println!(
        "  resolution_note: {}",
        request.resolution_note.as_deref().unwrap_or("<none>")
    );
    println!("  read_from: {}", path.display());
    ExitCode::SUCCESS
}

pub(crate) fn handle_tool_approval_list(args: &[String]) -> ExitCode {
    let tool_id = option_value(args, "--tool-id");
    let status = option_value(args, "--status");
    let root = option_value(args, "--root").unwrap_or(".");

    let (dir, requests) = match list_shell_approval_requests(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to list shell approval requests: {error}");
            return ExitCode::from(1);
        }
    };
    let status_filter = status.and_then(shell_request_status);
    let filtered = requests
        .into_iter()
        .filter(|request| tool_id.is_none_or(|value| request.tool_id == value))
        .filter(|request| {
            status_filter
                .as_ref()
                .is_none_or(|value| &request.status == value)
        })
        .collect::<Vec<_>>();

    println!("tool approval-list loaded");
    println!("  read_from: {}", dir.display());
    println!("  tool_id: {}", tool_id.unwrap_or("<none>"));
    println!("  status: {}", status.unwrap_or("<none>"));
    println!("  request_count: {}", filtered.len());
    for request in filtered {
        println!(
            "  - {} tool={} requested_by={} status={} requested_at={} resolved_at={} resolved_by={}",
            request.request_id,
            request.tool_id,
            request.requested_by,
            request.status.as_str(),
            request.requested_at,
            request.resolved_at.as_deref().unwrap_or("<none>"),
            request.resolved_by.as_deref().unwrap_or("<none>")
        );
    }
    ExitCode::SUCCESS
}

pub(crate) fn handle_tool_approval_queue(args: &[String]) -> ExitCode {
    let tool_id = option_value(args, "--tool-id");
    let owner = option_value(args, "--owner");
    let root = option_value(args, "--root").unwrap_or(".");

    let (dir, requests) = match list_shell_approval_requests(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load shell approval queue: {error}");
            return ExitCode::from(1);
        }
    };
    let filtered = requests
        .into_iter()
        .filter(|request| request.status == ApprovalRequestStatus::Pending)
        .filter(|request| tool_id.is_none_or(|value| request.tool_id == value))
        .filter(|request| owner.is_none_or(|value| request.owner == value))
        .collect::<Vec<_>>();

    println!("tool approval-queue loaded");
    println!("  read_from: {}", dir.display());
    println!("  tool_id: {}", tool_id.unwrap_or("<none>"));
    println!("  owner: {}", owner.unwrap_or("<none>"));
    println!("  pending_request_count: {}", filtered.len());
    let mut pending_by_owner = std::collections::BTreeMap::<String, usize>::new();
    let mut pending_by_age_bucket = std::collections::BTreeMap::<String, usize>::new();
    for request in &filtered {
        *pending_by_owner.entry(request.owner.clone()).or_insert(0) += 1;
        *pending_by_age_bucket
            .entry(approval_age_bucket(approval_request_age_ms(request)).to_owned())
            .or_insert(0) += 1;
    }
    println!("  pending_owner_count: {}", pending_by_owner.len());
    for (owner_id, count) in pending_by_owner {
        println!("  pending_owner: owner={} count={}", owner_id, count);
    }
    println!(
        "  pending_age_bucket_count: {}",
        pending_by_age_bucket.len()
    );
    for (bucket, count) in pending_by_age_bucket {
        println!("  pending_age_bucket: bucket={} count={}", bucket, count);
    }
    for request in filtered {
        let age_ms = approval_request_age_ms(&request);
        println!(
            "  - {} tool={} owner={} requested_by={} requested_at={} age_ms={} age_bucket={} entrypoint={}",
            request.request_id,
            request.tool_id,
            request.owner,
            request.requested_by,
            request.requested_at,
            age_ms
                .map(|value| value.to_string())
                .unwrap_or_else(|| "<unknown>".to_owned()),
            approval_age_bucket(age_ms),
            request.entrypoint
        );
    }
    ExitCode::SUCCESS
}

pub(crate) fn handle_tool_approval_overdue(args: &[String]) -> ExitCode {
    let tool_id = option_value(args, "--tool-id");
    let owner = option_value(args, "--owner");
    let root = option_value(args, "--root").unwrap_or(".");
    let threshold_ms = overdue_threshold_ms(args);

    let (dir, requests) = match list_shell_approval_requests(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load overdue shell approvals: {error}");
            return ExitCode::from(1);
        }
    };
    let filtered = requests
        .into_iter()
        .filter(|request| request.status == ApprovalRequestStatus::Pending)
        .filter(|request| tool_id.is_none_or(|value| request.tool_id == value))
        .filter(|request| owner.is_none_or(|value| request.owner == value))
        .filter(|request| approval_request_age_ms(request).is_some_and(|age| age >= threshold_ms))
        .collect::<Vec<_>>();

    println!("tool approval-overdue loaded");
    println!("  read_from: {}", dir.display());
    println!("  tool_id: {}", tool_id.unwrap_or("<none>"));
    println!("  owner: {}", owner.unwrap_or("<none>"));
    println!("  threshold_minutes: {}", threshold_ms / 60_000);
    println!("  overdue_request_count: {}", filtered.len());
    for request in filtered {
        let age_ms = approval_request_age_ms(&request);
        println!(
            "  - {} tool={} owner={} requested_by={} age_ms={} age_bucket={}",
            request.request_id,
            request.tool_id,
            request.owner,
            request.requested_by,
            age_ms
                .map(|value| value.to_string())
                .unwrap_or_else(|| "<unknown>".to_owned()),
            approval_age_bucket(age_ms)
        );
    }
    ExitCode::SUCCESS
}

pub(crate) fn handle_tool_approval_alerts(args: &[String]) -> ExitCode {
    let tool_id = option_value(args, "--tool-id");
    let owner = option_value(args, "--owner");
    let root = option_value(args, "--root").unwrap_or(".");
    let threshold_ms = overdue_threshold_ms(args);
    let include_acked = has_flag(args, "--include-acked");
    let as_json = has_flag(args, "--json");

    let (_, requests) = match list_shell_approval_requests(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load shell approval alerts: {error}");
            return ExitCode::from(1);
        }
    };
    let (_, tools) = match list_tools(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load tools for shell approval alerts: {error}");
            return ExitCode::from(1);
        }
    };
    let (_, alert_acks) = match list_policy_alert_acks(root) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to load policy alert acknowledgements: {error}");
            return ExitCode::from(1);
        }
    };
    let acked_ids = alert_acks
        .into_iter()
        .map(|ack| ack.alert_id)
        .collect::<std::collections::BTreeSet<_>>();

    let blocked_shell_tools = tools
        .into_iter()
        .filter(|tool| is_shell_tool(tool))
        .filter(|tool| tool_id.is_none_or(|value| tool.tool_id == value))
        .filter(|tool| owner.is_none_or(|value| tool.owner == value))
        .filter(|tool| !tool.allow_shell)
        .filter(|tool| include_acked || !acked_ids.contains(&blocked_tool_alert_id(&tool.tool_id)))
        .collect::<Vec<_>>();
    let overdue_requests = requests
        .into_iter()
        .filter(|request| request.status == ApprovalRequestStatus::Pending)
        .filter(|request| tool_id.is_none_or(|value| request.tool_id == value))
        .filter(|request| owner.is_none_or(|value| request.owner == value))
        .filter(|request| approval_request_age_ms(request).is_some_and(|age| age >= threshold_ms))
        .filter(|request| {
            include_acked || !acked_ids.contains(&overdue_request_alert_id(&request.request_id))
        })
        .collect::<Vec<_>>();

    let mut inbox_by_owner = std::collections::BTreeMap::<String, usize>::new();
    for tool in &blocked_shell_tools {
        *inbox_by_owner.entry(tool.owner.clone()).or_insert(0) += 1;
    }
    for request in &overdue_requests {
        *inbox_by_owner.entry(request.owner.clone()).or_insert(0) += 1;
    }
    if as_json {
        let payload = ApprovalAlertsJson {
            tool_id: tool_id.map(str::to_owned),
            owner: owner.map(str::to_owned),
            threshold_minutes: threshold_ms / 60_000,
            include_acked,
            blocked_shell_tool_count: blocked_shell_tools.len(),
            overdue_request_count: overdue_requests.len(),
            alert_count: blocked_shell_tools.len() + overdue_requests.len(),
            inbox_owners: inbox_by_owner
                .iter()
                .map(|(owner, count)| AlertOwnerSummary {
                    owner: owner.clone(),
                    count: *count,
                })
                .collect(),
            blocked_tools: blocked_shell_tools
                .iter()
                .map(|tool| AlertBlockedToolView {
                    tool_id: tool.tool_id.clone(),
                    owner: tool.owner.clone(),
                    policy: tool_policy_summary(tool),
                })
                .collect(),
            overdue_requests: overdue_requests
                .iter()
                .map(|request| {
                    let age_ms = approval_request_age_ms(request);
                    AlertOverdueRequestView {
                        request_id: request.request_id.clone(),
                        tool_id: request.tool_id.clone(),
                        owner: request.owner.clone(),
                        requested_by: request.requested_by.clone(),
                        age_ms,
                        age_bucket: approval_age_bucket(age_ms).to_owned(),
                    }
                })
                .collect(),
        };
        match serde_json::to_string_pretty(&payload) {
            Ok(body) => {
                println!("{body}");
                return ExitCode::SUCCESS;
            }
            Err(error) => {
                eprintln!("failed to render approval alerts json: {error}");
                return ExitCode::from(1);
            }
        }
    }
    println!("tool approval-alerts loaded");
    println!("  tool_id: {}", tool_id.unwrap_or("<none>"));
    println!("  owner: {}", owner.unwrap_or("<none>"));
    println!("  threshold_minutes: {}", threshold_ms / 60_000);
    println!(
        "  include_acked: {}",
        if include_acked { "true" } else { "false" }
    );
    println!("  blocked_shell_tool_count: {}", blocked_shell_tools.len());
    println!("  overdue_request_count: {}", overdue_requests.len());
    println!(
        "  alert_count: {}",
        blocked_shell_tools.len() + overdue_requests.len()
    );
    println!("  inbox_owner_count: {}", inbox_by_owner.len());
    for (owner_id, count) in inbox_by_owner {
        println!("  inbox_owner: owner={} count={}", owner_id, count);
    }
    for tool in blocked_shell_tools {
        println!(
            "  alert_blocked_tool: tool={} owner={} policy={}",
            tool.tool_id,
            tool.owner,
            tool_policy_summary(&tool)
        );
    }
    for request in overdue_requests {
        let age_ms = approval_request_age_ms(&request);
        println!(
            "  alert_overdue_request: request={} tool={} owner={} requested_by={} age_ms={} age_bucket={}",
            request.request_id,
            request.tool_id,
            request.owner,
            request.requested_by,
            age_ms
                .map(|value| value.to_string())
                .unwrap_or_else(|| "<unknown>".to_owned()),
            approval_age_bucket(age_ms)
        );
    }
    ExitCode::SUCCESS
}

pub(crate) fn handle_tool_approval_inbox(args: &[String]) -> ExitCode {
    handle_tool_approval_alerts(args)
}
