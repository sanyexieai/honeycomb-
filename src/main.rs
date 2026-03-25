use std::path::PathBuf;

use clap::{Parser, Subcommand};
use honeycomb::core::{
    LifecycleState, Scheduler, TaskConstraints, TaskHiveSession, TaskRuntime, TaskSpec, TaskStatus,
    TaskTopology, WorkerRequest,
};
use honeycomb::executors::ProcessExecutor;
use honeycomb::scheduler::MemoryScheduler;
use honeycomb::store::FsRepository;
use honeycomb::{validate_path, ValidationLevel};
use serde_json::json;

#[derive(Debug, Parser)]
#[command(name = "honeycomb")]
#[command(about = "Markdown-driven hive runtime skeleton", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Validate {
        #[arg(value_name = "PATH")]
        path: PathBuf,
    },
    Run {
        #[arg(value_name = "PATH")]
        path: PathBuf,
        #[arg(long)]
        task: Option<String>,
    },
    Inspect {
        #[arg(value_name = "RUN_ID")]
        run_id: String,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let repo = FsRepository::new(".");
    let scheduler = MemoryScheduler::new();
    let executor = ProcessExecutor::new();

    match cli.command {
        Command::Validate { path } => match validate_path(&path) {
            Ok(report) => {
                println!("validate requested: {}", path.display());
                println!("repository root: {}", repo.root().display());

                if report.issues.is_empty() {
                    println!("validation passed with no issues");
                    return;
                }

                for issue in &report.issues {
                    let level = match issue.level {
                        ValidationLevel::Error => "error",
                        ValidationLevel::Warning => "warning",
                    };
                    println!("[{level}] {}: {}", issue.path.display(), issue.message);
                }

                if report.is_ok() {
                    println!("validation passed with warnings");
                } else {
                    println!("validation failed");
                    std::process::exit(1);
                }
            }
            Err(error) => {
                eprintln!("validation error: {error:#}");
                std::process::exit(1);
            }
        },
        Command::Run { path, task } => {
            let task_id = format!("task_{}", std::process::id());
            let task_text = task.unwrap_or_else(|| "demo task".to_string());
            let spec = TaskSpec {
                task_id: task_id.clone(),
                task_type: "adhoc".to_string(),
                input: json!({
                    "path": path,
                    "task": task_text,
                }),
                context: json!({
                    "entry_path": path,
                    "repository_root": repo.root(),
                }),
                topology: TaskTopology::Singleton,
                constraints: TaskConstraints::default(),
            };

            let submitted_task_id = match scheduler.submit(spec).await {
                Ok(value) => value,
                Err(error) => {
                    eprintln!("scheduler submit error: {error:#}");
                    std::process::exit(1);
                }
            };

            if let Err(error) = persist_runtime(&repo, &scheduler, &submitted_task_id).await {
                eprintln!("runtime persistence error: {error:#}");
                std::process::exit(1);
            }

            if let Err(error) = scheduler
                .update_task_status(&submitted_task_id, TaskStatus::Running)
                .await
            {
                eprintln!("scheduler task update error: {error:#}");
                std::process::exit(1);
            }

            if let Err(error) = persist_runtime(&repo, &scheduler, &submitted_task_id).await {
                eprintln!("runtime persistence error: {error:#}");
                std::process::exit(1);
            }

            let implementation = match repo.load_implementation_from_dir(&path) {
                Ok(value) => value,
                Err(error) => {
                    eprintln!("failed to load implementation: {error:#}");
                    std::process::exit(1);
                }
            };

            let session_id = format!("sess_{}", std::process::id());
            let session = TaskHiveSession {
                session_id: session_id.clone(),
                task_id: submitted_task_id.clone(),
                hive_id: implementation.hive_id.clone(),
                selected_impl: implementation.impl_id.clone(),
                selected_practice: None,
                lifecycle: LifecycleState::Created,
                input: json!({
                    "source_text": task_text,
                }),
                context: json!({
                    "entry_path": path,
                }),
                overrides: json!({}),
                local_state: json!({}),
                artifacts: Vec::new(),
            };

            if let Err(error) = scheduler.add_session(&submitted_task_id, session).await {
                eprintln!("scheduler add session error: {error:#}");
                std::process::exit(1);
            }

            if let Err(error) = scheduler
                .update_session_lifecycle(&submitted_task_id, &session_id, LifecycleState::Running)
                .await
            {
                eprintln!("scheduler session update error: {error:#}");
                std::process::exit(1);
            }

            if let Err(error) = persist_runtime(&repo, &scheduler, &submitted_task_id).await {
                eprintln!("runtime persistence error: {error:#}");
                std::process::exit(1);
            }

            let runtime = match scheduler.poll(&submitted_task_id).await {
                Ok(value) => value,
                Err(error) => {
                    eprintln!("scheduler poll error: {error:#}");
                    std::process::exit(1);
                }
            };

            let request = WorkerRequest {
                task_id: runtime.task_id.clone(),
                session_id: session_id.clone(),
                hive_id: implementation.hive_id.clone(),
                impl_id: implementation.impl_id.clone(),
                input: json!({
                    "source_text": task_text,
                }),
                context: runtime.shared_context.clone(),
                overrides: json!({}),
            };

            match executor.run(&path, &implementation, &request) {
                Ok(output) => {
                    if let Err(error) = scheduler
                        .attach_session_output(&submitted_task_id, &session_id, &output)
                        .await
                    {
                        eprintln!("scheduler attach output error: {error:#}");
                        std::process::exit(1);
                    }

                    if let Err(error) = scheduler
                        .update_session_lifecycle(
                            &submitted_task_id,
                            &session_id,
                            LifecycleState::Completed,
                        )
                        .await
                    {
                        eprintln!("scheduler session completion error: {error:#}");
                        std::process::exit(1);
                    }

                    if let Err(error) = scheduler
                        .update_task_status(&submitted_task_id, TaskStatus::Completed)
                        .await
                    {
                        eprintln!("scheduler task completion error: {error:#}");
                        std::process::exit(1);
                    }

                    let final_runtime = match scheduler.poll(&submitted_task_id).await {
                        Ok(value) => value,
                        Err(error) => {
                            eprintln!("scheduler final poll error: {error:#}");
                            std::process::exit(1);
                        }
                    };

                    let persisted_path = match repo.save_task_runtime(&final_runtime) {
                        Ok(path) => path,
                        Err(error) => {
                            eprintln!("failed to persist runtime: {error:#}");
                            std::process::exit(1);
                        }
                    };

                    println!("run requested: {}", path.display());
                    println!("task id: {}", final_runtime.task_id);
                    println!("task status: {:?}", final_runtime.status);
                    println!("sessions: {}", final_runtime.sessions.len());
                    if let Some(session) = final_runtime.sessions.first() {
                        println!("session id: {}", session.session_id);
                        println!("session lifecycle: {:?}", session.lifecycle);
                    }
                    println!("worker success: {}", output.success);
                    println!("payload: {}", output.payload);
                    println!("task artifacts: {}", final_runtime.artifacts.len());
                    println!("output metrics: {}", output.metrics.len());
                    println!("persisted runtime: {}", persisted_path.display());
                }
                Err(error) => {
                    let _ = scheduler
                        .update_session_lifecycle(&submitted_task_id, &session_id, LifecycleState::Failed)
                        .await;
                    let _ = scheduler
                        .update_task_status(&submitted_task_id, TaskStatus::Failed)
                        .await;
                    let _ = persist_runtime(&repo, &scheduler, &submitted_task_id).await;
                    eprintln!("process execution error: {error:#}");
                    std::process::exit(1);
                }
            }
        }
        Command::Inspect { run_id } => match repo.load_task_runtime(&run_id) {
            Ok(runtime) => print_runtime(&repo, &runtime),
            Err(error) => {
                eprintln!("inspect error: {error:#}");
                std::process::exit(1);
            }
        },
    }
}

async fn persist_runtime(
    repo: &FsRepository,
    scheduler: &MemoryScheduler,
    task_id: &str,
) -> anyhow::Result<()> {
    let runtime = scheduler.poll(task_id).await?;
    repo.save_task_runtime(&runtime)?;
    Ok(())
}

fn print_runtime(repo: &FsRepository, runtime: &TaskRuntime) {
    println!("task id: {}", runtime.task_id);
    println!("task status: {:?}", runtime.status);
    println!("sessions: {}", runtime.sessions.len());
    for session in &runtime.sessions {
        println!("session id: {}", session.session_id);
        println!("session hive: {}", session.hive_id);
        println!("session impl: {}", session.selected_impl);
        println!("session lifecycle: {:?}", session.lifecycle);
        println!("session local_state: {}", session.local_state);
    }
    println!("task artifacts: {}", runtime.artifacts.len());
    println!(
        "runtime path: {}",
        repo.task_runtime_path(&runtime.task_id).display()
    );
}
