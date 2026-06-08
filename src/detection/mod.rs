pub mod content;
pub mod hooks;
mod process;
pub mod sidecar;

use crate::session::ClaudeCodeStatus;

pub use hooks::HooksBackend;
pub use process::ProcessBackend;
pub use sidecar::SidecarBackend;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionMethod {
    Process,
    Hooks,
    Sidecar,
}

impl DetectionMethod {
    pub fn from_str(s: &str) -> Self {
        match s {
            "hooks" => Self::Hooks,
            "sidecar" => Self::Sidecar,
            _ => Self::Process,
        }
    }

    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Process => "process",
            Self::Hooks => "hooks",
            Self::Sidecar => "sidecar",
        }
    }
}

pub struct DetectionContext<'a> {
    pub pane_pid: Option<u32>,
    pub pane_content: Option<&'a str>,
}

pub trait DetectionBackend {
    fn detect(&mut self, pane_id: &str, ctx: &DetectionContext) -> ClaudeCodeStatus;

    fn tick_start(&mut self) {}

    fn needs_content(&self) -> bool {
        false
    }
}

pub fn create_backend(method: DetectionMethod, staleness_secs: u64) -> Box<dyn DetectionBackend> {
    match method {
        DetectionMethod::Process => Box::new(ProcessBackend::new()),
        DetectionMethod::Hooks => Box::new(HooksBackend::new(staleness_secs)),
        DetectionMethod::Sidecar => Box::new(SidecarBackend::new(staleness_secs)),
    }
}
