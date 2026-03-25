use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
struct HiveFrontmatter {
    interface: Option<HiveInterfacePaths>,
}

#[derive(Debug, Deserialize)]
struct HiveInterfacePaths {
    input_schema: PathBuf,
    output_schema: PathBuf,
}

#[derive(Debug, Deserialize)]
struct ImplementationPaths {
    components: Option<ImplementationComponents>,
}

#[derive(Debug, Deserialize)]
struct ImplementationComponents {
    prompt: Option<PathBuf>,
    config: Option<PathBuf>,
    script: Option<PathBuf>,
    binary: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ValidationIssue {
    pub level: ValidationLevel,
    pub path: PathBuf,
    pub message: String,
}

#[derive(Debug, Clone, Copy)]
pub enum ValidationLevel {
    Error,
    Warning,
}

#[derive(Debug, Clone, Default)]
pub struct ValidationReport {
    pub issues: Vec<ValidationIssue>,
}

impl ValidationReport {
    pub fn is_ok(&self) -> bool {
        self.issues
            .iter()
            .all(|issue| matches!(issue.level, ValidationLevel::Warning))
    }

    pub fn push(
        &mut self,
        level: ValidationLevel,
        path: impl Into<PathBuf>,
        message: impl Into<String>,
    ) {
        self.issues.push(ValidationIssue {
            level,
            path: path.into(),
            message: message.into(),
        });
    }
}

pub fn validate_path(path: &Path) -> Result<ValidationReport> {
    if path.is_dir() {
        validate_hive_dir(path)
    } else {
        validate_markdown_file(path)
    }
}

fn validate_markdown_file(path: &Path) -> Result<ValidationReport> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read markdown file: {}", path.display()))?;
    let content = strip_bom(&content);

    let mut report = ValidationReport::default();

    if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
        report.push(ValidationLevel::Error, path, "expected a markdown file");
    }

    validate_hive_markdown(path, content, &mut report);
    validate_hive_frontmatter(path.parent().unwrap_or_else(|| Path::new(".")), path, content, &mut report)?;
    Ok(report)
}

fn validate_hive_dir(path: &Path) -> Result<ValidationReport> {
    let mut report = ValidationReport::default();

    let hive_md = path.join("hive.md");
    let implementation_json = path.join("implementation.json");
    let genome_json = path.join("genome.json");

    require_file(&hive_md, &mut report);
    require_file(&implementation_json, &mut report);
    require_file(&genome_json, &mut report);

    if hive_md.exists() {
        let content = fs::read_to_string(&hive_md)
            .with_context(|| format!("failed to read {}", hive_md.display()))?;
        let content = strip_bom(&content);
        validate_hive_markdown(&hive_md, content, &mut report);
        validate_hive_frontmatter(path, &hive_md, content, &mut report)?;
    }

    if implementation_json.exists() {
        validate_json_file(&implementation_json, &mut report)?;
        validate_implementation_paths(path, &implementation_json, &mut report)?;
    }

    if genome_json.exists() {
        validate_json_file(&genome_json, &mut report)?;
    }

    let practices_dir = path.join("practices");
    if practices_dir.exists() && practices_dir.is_dir() {
        for entry in fs::read_dir(&practices_dir)
            .with_context(|| format!("failed to read {}", practices_dir.display()))?
        {
            let entry = entry?;
            let entry_path = entry.path();
            if entry_path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                validate_json_file(&entry_path, &mut report)?;
            }
        }
    }

    Ok(report)
}

fn require_file(path: &Path, report: &mut ValidationReport) {
    if !path.exists() {
        report.push(ValidationLevel::Error, path, "required file is missing");
    }
}

fn validate_json_file(path: &Path, report: &mut ValidationReport) -> Result<()> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read json file: {}", path.display()))?;
    let content = strip_bom(&content);

    match serde_json::from_str::<Value>(content) {
        Ok(_) => Ok(()),
        Err(error) => {
            report.push(
                ValidationLevel::Error,
                path,
                format!("invalid json: {error}"),
            );
            Ok(())
        }
    }
}

fn validate_hive_markdown(path: &Path, content: &str, report: &mut ValidationReport) {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        report.push(ValidationLevel::Error, path, "markdown file is empty");
        return;
    }

    if !trimmed.starts_with("---") {
        report.push(
            ValidationLevel::Warning,
            path,
            "missing YAML frontmatter start marker",
        );
    }

    if !trimmed.contains("capability:") {
        report.push(
            ValidationLevel::Warning,
            path,
            "missing capability field in frontmatter or body",
        );
    }

    for heading in ["# Purpose", "# Rules"] {
        if !content.contains(heading) {
            report.push(
                ValidationLevel::Warning,
                path,
                format!("recommended section not found: {heading}"),
            );
        }
    }
}

fn validate_hive_frontmatter(
    root: &Path,
    path: &Path,
    content: &str,
    report: &mut ValidationReport,
) -> Result<()> {
    let Some(frontmatter) = extract_frontmatter(content) else {
        return Ok(());
    };

    match serde_yaml::from_str::<HiveFrontmatter>(frontmatter) {
        Ok(frontmatter) => {
            if let Some(interface) = frontmatter.interface {
                require_relative_file(root, &interface.input_schema, report);
                require_relative_file(root, &interface.output_schema, report);
            } else {
                report.push(
                    ValidationLevel::Warning,
                    path,
                    "frontmatter does not define interface paths",
                );
            }
        }
        Err(error) => {
            report.push(
                ValidationLevel::Error,
                path,
                format!("invalid YAML frontmatter: {error}"),
            );
        }
    }

    Ok(())
}

fn validate_implementation_paths(
    root: &Path,
    path: &Path,
    report: &mut ValidationReport,
) -> Result<()> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read json file: {}", path.display()))?;
    let content = strip_bom(&content);

    match serde_json::from_str::<ImplementationPaths>(content) {
        Ok(implementation) => {
            if let Some(components) = implementation.components {
                for relative in [
                    components.prompt,
                    components.config,
                    components.script,
                    components.binary,
                ]
                .into_iter()
                .flatten()
                {
                    require_relative_file(root, &relative, report);
                }
            }
        }
        Err(error) => {
            report.push(
                ValidationLevel::Error,
                path,
                format!("invalid implementation structure: {error}"),
            );
        }
    }

    Ok(())
}

fn extract_frontmatter(content: &str) -> Option<&str> {
    let content = content.strip_prefix("---\n")?;
    let end = content.find("\n---")?;
    Some(&content[..end])
}

fn require_relative_file(root: &Path, relative: &Path, report: &mut ValidationReport) {
    let full = root.join(relative);
    if !full.exists() {
        report.push(
            ValidationLevel::Error,
            full,
            format!("referenced file does not exist: {}", relative.display()),
        );
    }
}

fn strip_bom(content: &str) -> &str {
    content.trim_start_matches('\u{feff}')
}
