//! Explicit session state machine (Phase 3). Rust is the sole owner of
//! transitions; the UI only mirrors snapshots. No boolean-flag states.

/// The seven Neko session states (master §9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    Disconnected,
    Idle,
    Working,
    ToolRunning,
    WaitingPermission,
    Completed,
    Error,
}

impl SessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SessionStatus::Disconnected => "disconnected",
            SessionStatus::Idle => "idle",
            SessionStatus::Working => "working",
            SessionStatus::ToolRunning => "tool_running",
            SessionStatus::WaitingPermission => "waiting_permission",
            SessionStatus::Completed => "completed",
            SessionStatus::Error => "error",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "disconnected" => Some(SessionStatus::Disconnected),
            "idle" => Some(SessionStatus::Idle),
            "working" => Some(SessionStatus::Working),
            "tool_running" => Some(SessionStatus::ToolRunning),
            "waiting_permission" => Some(SessionStatus::WaitingPermission),
            "completed" => Some(SessionStatus::Completed),
            "error" => Some(SessionStatus::Error),
            _ => None,
        }
    }
}

/// Neko-level event kinds fed by the manager (adapted from OpenCode hooks).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    Created,
    Status(SessionStatus),
    Completed,
    Error,
    ToolStarted,
    ToolCompleted,
    PermissionRequested,
    PermissionResolved,
    Activity, // diff / file.edited / todo.updated: activity only
}

/// Outcome of one transition attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Valid event, state kept. Manager still bumps activity.
    Unchanged,
    /// Valid event, state moves.
    Changed(SessionStatus),
    /// Invalid for this state. Logged, never panics, activity untouched.
    Rejected,
}

use EventKind as E;
use Outcome as O;
use SessionStatus as S;

/// Explicit transition table. Unknown combinations are Rejected, never guessed.
pub fn transition(from: S, ev: E) -> O {
    match (from, ev) {
        (_, E::Status(s)) => O::Changed(s),
        (_, E::Error) => O::Changed(S::Error),
        (_, E::Activity) => O::Unchanged,

        (S::Disconnected | S::Idle | S::Completed | S::Error, E::Created) => O::Changed(S::Idle),
        (S::Disconnected, E::Completed) => O::Rejected,
        (_, E::Completed) => O::Changed(S::Completed),

        (S::Working | S::Idle, E::ToolStarted) => O::Changed(S::ToolRunning),
        (S::ToolRunning, E::ToolStarted) => O::Unchanged,
        (S::ToolRunning, E::ToolCompleted) => O::Changed(S::Working),
        (_, E::ToolStarted) => O::Rejected,
        (_, E::ToolCompleted) => O::Unchanged,

        (S::Working | S::ToolRunning | S::Idle, E::PermissionRequested) => {
            O::Changed(S::WaitingPermission)
        }
        (S::WaitingPermission, E::PermissionRequested) => O::Unchanged,
        (S::WaitingPermission, E::PermissionResolved) => O::Changed(S::Working),
        (_, E::PermissionRequested) => O::Rejected,
        (_, E::PermissionResolved) => O::Unchanged,

        (_, E::Created) => O::Rejected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_working_to_completed() {
        assert_eq!(
            transition(S::Idle, E::Status(S::Working)),
            O::Changed(S::Working)
        );
        assert_eq!(
            transition(S::Working, E::ToolStarted),
            O::Changed(S::ToolRunning)
        );
        assert_eq!(
            transition(S::ToolRunning, E::ToolCompleted),
            O::Changed(S::Working)
        );
        assert_eq!(
            transition(S::Working, E::Completed),
            O::Changed(S::Completed)
        );
        assert_eq!(transition(S::Completed, E::Created), O::Changed(S::Idle));
    }

    #[test]
    fn permission_cycle() {
        assert_eq!(
            transition(S::Working, E::PermissionRequested),
            O::Changed(S::WaitingPermission)
        );
        assert_eq!(
            transition(S::WaitingPermission, E::PermissionResolved),
            O::Changed(S::Working)
        );
    }

    #[test]
    fn error_from_anywhere_and_recovery() {
        for from in [S::Idle, S::Working, S::ToolRunning, S::WaitingPermission] {
            assert_eq!(transition(from, E::Error), O::Changed(S::Error));
        }
        assert_eq!(transition(S::Error, E::Created), O::Changed(S::Idle));
    }

    #[test]
    fn invalid_is_rejected_not_panicked() {
        assert_eq!(transition(S::Completed, E::ToolStarted), O::Rejected);
        assert_eq!(transition(S::Disconnected, E::Completed), O::Rejected);
        assert_eq!(transition(S::Idle, E::ToolCompleted), O::Unchanged);
        assert_eq!(transition(S::Working, E::Activity), O::Unchanged);
    }
}
