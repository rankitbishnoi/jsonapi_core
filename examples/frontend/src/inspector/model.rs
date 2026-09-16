//! Pure, framework-free exchange log. No Leptos, no wasm — unit-testable.
// consumed by the reactive store, client, and panel in later tasks
#![allow(dead_code)]

/// One captured HTTP header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    pub name: String,
    pub value: String,
}

/// A fully captured request/response exchange.
#[derive(Clone, Debug, PartialEq)]
pub struct Exchange {
    pub id: usize,
    pub method: String,
    pub url: String,
    pub req_headers: Vec<Header>,
    pub req_body: Option<String>,
    pub status: u16,
    pub resp_headers: Vec<Header>,
    pub resp_body: Option<String>,
    pub duration_ms: f64,
    pub request_id: Option<String>,
}

/// Everything needed to record an exchange except its assigned `id`.
#[derive(Clone, Debug, PartialEq)]
pub struct ExchangeInit {
    pub method: String,
    pub url: String,
    pub req_headers: Vec<Header>,
    pub req_body: Option<String>,
    pub status: u16,
    pub resp_headers: Vec<Header>,
    pub resp_body: Option<String>,
    pub duration_ms: f64,
    pub request_id: Option<String>,
}

/// Ordered log of exchanges, newest first, with a selection cursor.
#[derive(Clone, Debug, Default)]
pub struct ExchangeLog {
    entries: Vec<Exchange>,
    selected: Option<usize>,
    next_id: usize,
}

impl ExchangeLog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a new exchange at the front (newest first), auto-select it,
    /// and return its assigned id.
    pub fn record(&mut self, init: ExchangeInit) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        let ex = Exchange {
            id,
            method: init.method,
            url: init.url,
            req_headers: init.req_headers,
            req_body: init.req_body,
            status: init.status,
            resp_headers: init.resp_headers,
            resp_body: init.resp_body,
            duration_ms: init.duration_ms,
            request_id: init.request_id,
        };
        self.entries.insert(0, ex);
        self.selected = Some(id);
        id
    }

    pub fn entries(&self) -> &[Exchange] {
        &self.entries
    }

    pub fn selected_id(&self) -> Option<usize> {
        self.selected
    }

    pub fn selected(&self) -> Option<&Exchange> {
        let id = self.selected?;
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn select(&mut self, id: usize) {
        if self.entries.iter().any(|e| e.id == id) {
            self.selected = Some(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init(method: &str, status: u16) -> ExchangeInit {
        ExchangeInit {
            method: method.into(),
            url: "http://127.0.0.1:8080/articles".into(),
            req_headers: vec![],
            req_body: None,
            status,
            resp_headers: vec![],
            resp_body: None,
            duration_ms: 1.0,
            request_id: None,
        }
    }

    #[test]
    fn records_newest_first_and_autoselects() {
        let mut log = ExchangeLog::new();
        let first = log.record(init("GET", 200));
        let second = log.record(init("POST", 201));
        assert_eq!(log.entries()[0].id, second, "newest is at the front");
        assert_eq!(log.entries()[1].id, first);
        assert_eq!(log.selected_id(), Some(second), "latest auto-selected");
    }

    #[test]
    fn ids_are_monotonic() {
        let mut log = ExchangeLog::new();
        assert_eq!(log.record(init("GET", 200)), 0);
        assert_eq!(log.record(init("GET", 200)), 1);
    }

    #[test]
    fn select_existing_changes_cursor_but_ignores_unknown() {
        let mut log = ExchangeLog::new();
        let a = log.record(init("GET", 200));
        let _b = log.record(init("GET", 200));
        log.select(a);
        assert_eq!(log.selected_id(), Some(a));
        log.select(999);
        assert_eq!(log.selected_id(), Some(a), "unknown id ignored");
    }
}
