use std::io::{self, BufRead, Write};
use std::process::ExitCode;

use crate::app::cli::{has_flag, option_value};
use crate::app::execution::capability::handle_skill_execute;
use crate::app::execution::history::append_event;
use crate::core::current_timestamp;

/// Bee 的静态画像（后面可以从配置/文档加载）
#[derive(Debug, Clone)]
pub struct BeeProfile {
    pub bee_id: String,
    pub display_name: String,
    pub skill_id: String,
    pub max_short_turns: usize,
}

impl BeeProfile {
    pub fn from_env_and_args(args: &[String]) -> Self {
        let bee_id =
            std::env::var("HONEYCOMB_BEE_ID").unwrap_or_else(|_| "code_assistant".to_owned());
        let display_name =
            std::env::var("HONEYCOMB_BEE_NAME").unwrap_or_else(|_| "Code Bee".to_owned());

        let skill_id = option_value(args, "--skill-id")
            .map(str::to_owned)
            .or_else(|| std::env::var("HONEYCOMB_CODE_SKILL").ok())
            .unwrap_or_else(|| "code-assistant".to_owned());

        Self {
            bee_id,
            display_name,
            skill_id,
            max_short_turns: 6,
        }
    }
}

/// 单次会话中的 Bee，持有短期记忆 / 会话 ID 等
pub struct BeeSession {
    pub profile: BeeProfile,
    pub root: String,
    pub session_id: String,
    pub use_recommended_impl: bool,
    pub run_tools: bool,
    user_history: Vec<String>,
}

impl BeeSession {
    pub fn new(profile: BeeProfile, root: String, args: &[String]) -> Self {
        let session_id =
            format!("{}-{}", profile.bee_id, current_timestamp().replace(':', "_"));
        let use_recommended_impl = has_flag(args, "--use-recommended-impl");
        let run_tools = has_flag(args, "--run-tools");

        Self {
            profile,
            root,
            session_id,
            use_recommended_impl,
            run_tools,
            user_history: Vec::new(),
        }
    }

    fn merged_input(&self, current: &str) -> String {
        let mut merged = String::new();
        let start = self
            .user_history
            .len()
            .saturating_sub(self.profile.max_short_turns);
        for past in &self.user_history[start..] {
            merged.push_str("user: ");
            merged.push_str(past);
            merged.push('\n');
        }
        merged.push_str("user: ");
        merged.push_str(current);
        merged
    }

    pub fn print_banner(&self) {
        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| ".".to_owned());

        println!("Honeycomb Code · bee runtime");
        println!("  bee: {} ({})", self.profile.bee_id, self.profile.display_name);
        println!("  workspace: {cwd}");
        println!(
            "  skill: {}  (override with --skill-id or HONEYCOMB_CODE_SKILL)",
            self.profile.skill_id
        );
        if self.profile.skill_id == "code-assistant"
            && std::env::var("HONEYCOMB_LLM_API_KEY").is_err()
            && std::env::var("MINIMAX_API_KEY").is_err()
        {
            eprintln!(
                "  (set HONEYCOMB_LLM_API_KEY for real LLM; default impl uses minimax_chat)"
            );
        }
        println!("  exit | :q  — quit   ·   /help — hints");
        println!();
    }

    pub fn repl_loop(&mut self) -> ExitCode {
        let stdin = io::stdin();
        let mut reader = stdin.lock();

        loop {
            print!("> ");
            if io::stdout().flush().is_err() {
                break;
            }
            let mut line = String::new();
            if reader.read_line(&mut line).is_err() {
                break;
            }
            let trimmed = line.trim_end();
            if trimmed.is_empty() {
                continue;
            }
            let t = trimmed.trim();
            if t == "exit" || t == ":q" || t == "/exit" {
                break;
            }
            if t == "/help" {
                println!("  Each line runs `skill execute` via this Bee (default: MiniMax-M2.5 when HONEYCOMB_LLM_PROVIDER=minimax and key is set).");
                println!("  Flags from CLI apply to each turn: --use-recommended-impl --run-tools");
                println!("  exit | :q  quit");
                continue;
            }

            if let Err(err) = append_event(&self.root, &self.session_id, "user", t) {
                eprintln!("  (failed to append conversation history: {err})");
            }

            let merged_input = self.merged_input(t);
            let mut exec_args = vec![
                "skill".to_string(),
                "execute".to_string(),
                "--skill-id".to_string(),
                self.profile.skill_id.clone(),
                "--input".to_string(),
                merged_input,
                "--root".to_string(),
                self.root.clone(),
            ];
            if self.use_recommended_impl {
                exec_args.push("--use-recommended-impl".to_string());
            }
            if self.run_tools {
                exec_args.push("--run-tools".to_string());
            }

            let code = handle_skill_execute(&exec_args);
            if code != ExitCode::SUCCESS {
                eprintln!("  (turn failed; fix errors above and try again, or :q to quit)");
            }
            self.user_history.push(t.to_string());
            println!();
        }

        ExitCode::SUCCESS
    }
}

