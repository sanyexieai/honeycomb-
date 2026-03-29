pub(crate) fn joined_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "<none>".to_owned()
    } else {
        values.join(", ")
    }
}

pub(crate) fn entrypoint_scheme(entrypoint: &str) -> &'static str {
    if entrypoint.starts_with("shell://") {
        "shell"
    } else if entrypoint.starts_with("tool://") {
        "tool"
    } else {
        "custom"
    }
}

pub(crate) fn owner_trust_tier(owner: &str) -> &'static str {
    match owner {
        "system" => "system",
        "tenant-local" => "trusted_local",
        _ => "tenant",
    }
}
