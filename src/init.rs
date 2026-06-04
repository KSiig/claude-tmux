use std::path::PathBuf;

use anyhow::{Context, Result};
use serde_json::Value;

const STATUS_SCRIPT: &str = r#"#!/bin/bash
mkdir -p /tmp/claude-tmux-hooks
printf '%s %s\n' "$1" "$(date +%s)" > "/tmp/claude-tmux-hooks/$TMUX_PANE"
"#;

const HOOK_EVENTS: &[(&str, &str)] = &[
    ("UserPromptSubmit", "working"),
    ("Stop", "idle"),
    ("StopFailure", "error"),
    ("PermissionRequest", "waiting_input"),
    ("Elicitation", "waiting_input"),
];

pub fn run_init() -> Result<()> {
    let script_path = create_status_script()?;
    let added = add_claude_hooks(&script_path)?;

    if added.is_empty() {
        println!("All claude-tmux hooks already configured — nothing to do.");
    } else {
        println!("Added hooks to ~/.claude/settings.json:");
        for event in &added {
            println!("  {}", event);
        }
    }

    Ok(())
}

fn create_status_script() -> Result<PathBuf> {
    let dir = dirs::home_dir()
        .context("could not determine home directory")?
        .join(".claude-tmux")
        .join("hooks");
    std::fs::create_dir_all(&dir)?;

    let path = dir.join("status.sh");
    std::fs::write(&path, STATUS_SCRIPT)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
    }

    println!("Created {}", path.display());
    Ok(path)
}

fn add_claude_hooks(script_path: &PathBuf) -> Result<Vec<String>> {
    let settings_path = dirs::home_dir()
        .context("could not determine home directory")?
        .join(".claude")
        .join("settings.json");

    let mut settings: Value = if settings_path.exists() {
        let content = std::fs::read_to_string(&settings_path)?;
        serde_json::from_str(&content).context("failed to parse ~/.claude/settings.json")?
    } else {
        serde_json::json!({})
    };

    let hooks = settings
        .as_object_mut()
        .context("settings.json root is not an object")?
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}));

    let hooks_obj = hooks
        .as_object_mut()
        .context("hooks field is not an object")?;

    let script_str = script_path.display().to_string();
    let mut added = Vec::new();

    for (event, status_arg) in HOOK_EVENTS {
        let command = format!("{} {}", script_str, status_arg);

        let event_array = hooks_obj
            .entry(*event)
            .or_insert_with(|| serde_json::json!([]));

        let arr = event_array
            .as_array_mut()
            .context(format!("hooks.{} is not an array", event))?;

        let already_exists = arr.iter().any(|entry| {
            entry
                .get("hooks")
                .and_then(|h| h.as_array())
                .map(|h| {
                    h.iter().any(|hook| {
                        hook.get("command")
                            .and_then(|c| c.as_str())
                            .is_some_and(|c| c.contains("claude-tmux") && c.contains("status.sh"))
                    })
                })
                .unwrap_or(false)
        });

        if already_exists {
            continue;
        }

        let entry = serde_json::json!({
            "matcher": "",
            "hooks": [{
                "type": "command",
                "command": command,
            }]
        });

        arr.push(entry);
        added.push(format!("{} → {}", event, status_arg));
    }

    if !added.is_empty() {
        let formatted = serde_json::to_string_pretty(&settings)?;
        std::fs::write(&settings_path, formatted)?;
    }

    Ok(added)
}
