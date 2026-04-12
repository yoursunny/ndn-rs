pub mod config;
pub mod cs;
pub mod dashboard_config;
pub mod faces;
pub mod fleet;
pub mod logs;
pub mod modals;
pub mod onboarding;
pub mod overview;
pub mod radio;
pub mod routes;
pub mod routing;
pub mod security;
pub mod session;
pub mod strategy;
pub mod tools;
pub mod traffic;

/// Which panel is currently visible in the content area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Overview,
    Strategy,
    Logs,
    Session,
    Security,
    Fleet,
    Routing,
    Radio,
    Tools,
    DashboardConfig,
    RouterConfig,
}

impl View {
    pub fn label(self) -> &'static str {
        match self {
            View::Overview => "Overview",
            View::Strategy => "Strategy",
            View::Logs => "Logs",
            View::Session => "Session",
            View::Security => "Security",
            View::Fleet => "Fleet",
            View::Routing => "Routing",
            View::Radio => "Radio",
            View::Tools => "Tools",
            View::DashboardConfig => "Dashboard Config",
            View::RouterConfig => "Router Config",
        }
    }

    pub const NAV: &'static [View] = &[
        View::Overview,
        View::Strategy,
        View::Logs,
        View::Session,
        View::Security,
        View::Fleet,
        View::Routing,
        View::Radio,
        View::Tools,
    ];
}
