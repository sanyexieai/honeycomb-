use std::io::{self, Read};

use serde_json::json;

fn main() {
    let mut raw = String::new();
    if io::stdin().read_to_string(&mut raw).is_err() {
        eprintln!("failed to read stdin");
        std::process::exit(1);
    }

    let request: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("invalid request json: {error}");
            std::process::exit(1);
        }
    };

    let text = request
        .get("input")
        .and_then(|input| input.get("source_text"))
        .and_then(|value| value.as_str())
        .unwrap_or_default();

    let summary = text
        .split_whitespace()
        .take(12)
        .collect::<Vec<_>>()
        .join(" ");

    let response = json!({
        "success": true,
        "payload": {
            "summary": summary
        },
        "metrics": [
            { "name": "word_count", "value": text.split_whitespace().count() as f64 }
        ],
        "artifacts": []
    });

    println!("{}", response);
}
