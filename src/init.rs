use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

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
    println!("claude-tmux setup\n");

    let choice = prompt_detection_method()?;

    match choice {
        DetectionChoice::Hooks => {
            let script_path = create_status_script()?;
            let added = add_claude_hooks(&script_path)?;
            if added.is_empty() {
                println!("All claude-tmux hooks already configured.");
            } else {
                println!("Added hooks to ~/.claude/settings.json:");
                for event in &added {
                    println!("  {}", event);
                }
            }
            set_detection_method("hooks")?;
        }
        DetectionChoice::Sidecar => {
            set_detection_method("sidecar")?;
            println!("Sidecar detection enabled (experimental).");
            println!("The daemon will automatically start pipe-pane sidecars for Claude panes.");
        }
        DetectionChoice::Skip => {
            println!("Keeping default process-tree detection (no additional setup needed).");
        }
    }

    println!();
    install_daemon()?;

    Ok(())
}

enum DetectionChoice {
    Hooks,
    Sidecar,
    Skip,
}

fn prompt_detection_method() -> Result<DetectionChoice> {
    println!("Detection method:");
    println!("  [1] Hooks — Claude Code hooks write status on each event");
    println!("  [2] Sidecar (experimental) — Real-time stream analysis via pipe-pane");
    println!("  [3] Skip — Keep default process-tree detection (no setup needed)");
    print!("\nChoice [1/2/3]: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    match input.trim() {
        "1" => Ok(DetectionChoice::Hooks),
        "2" => Ok(DetectionChoice::Sidecar),
        _ => Ok(DetectionChoice::Skip),
    }
}

fn set_detection_method(method: &str) -> Result<()> {
    let settings_dir = dirs::home_dir()
        .context("could not determine home directory")?
        .join(".claude-tmux");
    std::fs::create_dir_all(&settings_dir)?;
    let settings_path = settings_dir.join("settings.json");

    let mut settings: Value = if settings_path.exists() {
        let content = std::fs::read_to_string(&settings_path)?;
        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    settings
        .as_object_mut()
        .context("settings.json root is not an object")?
        .insert(
            "detection_method".to_string(),
            serde_json::Value::String(method.to_string()),
        );

    let formatted = serde_json::to_string_pretty(&settings)?;
    std::fs::write(&settings_path, formatted)?;
    println!("Set detection_method=\"{}\" in {}", method, settings_path.display());

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

// =========================================================================
// Daemon service installation
// =========================================================================

fn install_daemon() -> Result<()> {
    let binary = std::env::current_exe().context("could not determine binary path")?;

    if cfg!(target_os = "macos") {
        install_launchd(&binary)
    } else if cfg!(target_os = "linux") {
        install_systemd(&binary)
    } else {
        println!("Unsupported OS for daemon installation. Start manually:");
        println!("  {} --headless &", binary.display());
        Ok(())
    }
}

fn install_launchd(binary: &PathBuf) -> Result<()> {
    let label = "com.claude-tmux.daemon";
    let plist_dir = dirs::home_dir()
        .context("could not determine home directory")?
        .join("Library")
        .join("LaunchAgents");
    std::fs::create_dir_all(&plist_dir)?;
    let plist_path = plist_dir.join(format!("{}.plist", label));

    let path_env = build_path_env();

    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{binary}</string>
        <string>--headless</string>
    </array>
    <key>EnvironmentVariables</key>
    <dict>
        <key>PATH</key>
        <string>{path}</string>{linear_key}
    </dict>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/claude-tmux-daemon.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/claude-tmux-daemon.log</string>
</dict>
</plist>
"#,
        label = label,
        binary = binary.display(),
        path = path_env,
        linear_key = linear_key_plist_fragment(),
    );

    // Unload existing service if present (ignore errors — might not be loaded)
    if plist_path.exists() {
        let _ = Command::new("launchctl")
            .args(["unload", &plist_path.to_string_lossy()])
            .output();
    }

    std::fs::write(&plist_path, plist)?;

    let status = Command::new("launchctl")
        .args(["load", &plist_path.to_string_lossy()])
        .status()
        .context("failed to run launchctl load")?;

    if status.success() {
        println!("Daemon installed and started (launchd: {})", label);
    } else {
        anyhow::bail!("launchctl load failed");
    }

    Ok(())
}

fn install_systemd(binary: &PathBuf) -> Result<()> {
    let unit_dir = dirs::home_dir()
        .context("could not determine home directory")?
        .join(".config")
        .join("systemd")
        .join("user");
    std::fs::create_dir_all(&unit_dir)?;
    let unit_path = unit_dir.join("claude-tmux.service");

    let path_env = build_path_env();

    let mut env_lines = format!("Environment=PATH={}", path_env);
    if let Ok(key) = std::env::var("LINEAR_API_KEY") {
        env_lines.push_str(&format!("\nEnvironment=LINEAR_API_KEY={}", key));
    }

    let unit = format!(
        r#"[Unit]
Description=claude-tmux status daemon
After=default.target

[Service]
Type=simple
ExecStart={binary} --headless
Restart=on-failure
RestartSec=5
{env}

[Install]
WantedBy=default.target
"#,
        binary = binary.display(),
        env = env_lines,
    );

    std::fs::write(&unit_path, unit)?;

    let _ = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status();

    let status = Command::new("systemctl")
        .args(["--user", "enable", "--now", "claude-tmux"])
        .status()
        .context("failed to run systemctl")?;

    if status.success() {
        println!("Daemon installed and started (systemd: claude-tmux.service)");
    } else {
        anyhow::bail!("systemctl enable --now failed");
    }

    Ok(())
}

fn build_path_env() -> String {
    std::env::var("PATH").unwrap_or_else(|_| "/usr/local/bin:/usr/bin:/bin".to_string())
}

fn linear_key_plist_fragment() -> String {
    match std::env::var("LINEAR_API_KEY") {
        Ok(key) => format!(
            "\n        <key>LINEAR_API_KEY</key>\n        <string>{}</string>",
            key
        ),
        Err(_) => String::new(),
    }
}
