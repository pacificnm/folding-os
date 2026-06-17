//! Shared FAH metric fields collected from logs, WebSocket, and client.db.

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FahLogState {
    pub project: Option<String>,
    pub run: Option<f64>,
    pub clone: Option<f64>,
    pub gen: Option<f64>,
    pub progress: Option<f64>,
    pub ppd: Option<f64>,
    pub tpf: Option<String>,
    pub recent_errors: Vec<String>,
}

impl FahLogState {
    pub fn empty() -> Self {
        Self::default()
    }
}
