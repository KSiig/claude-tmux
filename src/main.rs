mod app;
mod completion;
mod detection;
mod git;
mod init;
mod input;
mod linear;
mod monitor;
mod scroll_state;
mod session;
mod settings;
mod tmux;
mod ui;

use std::io::{self, stdout};

use anyhow::Result;
use crossterm::{
    event::{self, Event},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::prelude::*;

use crate::app::App;
use crate::settings::Settings;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "init") {
        return init::run_init();
    }

    if args.iter().any(|a| a == "monitor") {
        let pane_id = args
            .iter()
            .position(|a| a == "--pane")
            .and_then(|i| args.get(i + 1))
            .map(|s| s.as_str())
            .unwrap_or_else(|| {
                eprintln!("Usage: claude-tmux monitor --pane <pane_id>");
                std::process::exit(1);
            });
        return monitor::run_monitor(pane_id);
    }

    let headless = args.iter().any(|a| a == "--headless" || a == "-d");

    if headless {
        return run_headless();
    }

    // Set up terminal
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    // Run the app
    let result = run(&mut terminal);

    // Restore terminal
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    result
}

fn run_headless() -> Result<()> {
    let settings = Settings::load();
    let sleep_interval = settings.status_interval;

    let linear_config = settings.task_integration.as_ref().and_then(|t| {
        if t.provider == "linear" {
            Some((t.poll_interval, t.issue_prefix.clone()))
        } else {
            None
        }
    });

    let mut app = App::new(true)?;
    let mut linear_poller = linear_config.as_ref().map(|_| linear::LinearPoller::new());

    loop {
        if let Err(e) = app.refresh_for_daemon() {
            eprintln!("claude-tmux daemon: refresh failed: {}", e);
        }
        app.tick_status();

        if let (Some(poller), Some((interval, ref prefix))) =
            (linear_poller.as_mut(), &linear_config)
        {
            let names: Vec<String> = app.session_names();
            let ids = linear::extract_identifiers(&names, prefix.as_deref());
            poller.poll_if_due(*interval, &ids);
        }

        std::thread::sleep(sleep_interval);
    }
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let mut app = App::new(false)?;

    loop {
        // Draw the UI
        terminal.draw(|frame| ui::render(frame, &mut app))?;

        // Check if we should quit
        if app.should_quit {
            break;
        }

        // Handle events
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                input::handle_key(&mut app, key);
            }
        }

        // Refresh Claude status via configured detection backend (self-throttled)
        app.tick_status();

        // Check for completed background tasks (fork session, etc.)
        app.check_fork_result();
    }

    Ok(())
}
