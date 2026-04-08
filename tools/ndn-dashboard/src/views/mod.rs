pub mod config;
pub mod onboarding;
pub mod cs;
pub mod faces;
pub mod fleet;
pub mod logs;
pub mod overview;
pub mod radio;
pub mod routes;
pub mod security;
pub mod session;
pub mod strategy;
pub mod traffic;

/// Which panel is currently visible in the content area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Overview,
    Faces,
    Routes,
    ContentStore,
    Strategy,
    Traffic,
    Logs,
    Config,
    Session,
    Security,
    Fleet,
    Radio,
}

impl View {
    pub fn label(self) -> &'static str {
        match self {
            View::Overview     => "Overview",
            View::Faces        => "Faces",
            View::Routes       => "Routes",
            View::ContentStore => "Content Store",
            View::Strategy     => "Strategy",
            View::Traffic      => "Traffic",
            View::Logs         => "Logs",
            View::Config       => "Config",
            View::Session      => "Session",
            View::Security     => "Security",
            View::Fleet        => "Fleet",
            View::Radio        => "Radio",
        }
    }

    pub const ALL: &'static [View] = &[
        View::Overview,
        View::Faces,
        View::Routes,
        View::ContentStore,
        View::Strategy,
        View::Traffic,
        View::Logs,
        View::Config,
        View::Session,
        View::Security,
        View::Fleet,
        View::Radio,
    ];
}
