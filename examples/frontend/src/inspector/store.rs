use leptos::prelude::*;

use super::model::{Exchange, ExchangeInit, ExchangeLog};

/// Reactive, `Copy` handle to the inspector's exchange log. Provided via context.
#[derive(Clone, Copy)]
pub struct InspectorStore {
    log: RwSignal<ExchangeLog>,
}

impl InspectorStore {
    pub fn new() -> Self {
        Self {
            log: RwSignal::new(ExchangeLog::new()),
        }
    }

    /// Record an exchange (newest first, auto-selected). Returns its id.
    // consumed by the HTTP client in a later task
    #[allow(dead_code)]
    pub fn record(&self, init: ExchangeInit) -> usize {
        let mut id = None;
        self.log.update(|l| id = Some(l.record(init)));
        id.expect("RwSignal::update runs its closure synchronously")
    }

    pub fn entries(&self) -> Vec<Exchange> {
        self.log.with(|l| l.entries().to_vec())
    }

    pub fn selected(&self) -> Option<Exchange> {
        self.log.with(|l| l.selected().cloned())
    }

    pub fn selected_id(&self) -> Option<usize> {
        self.log.with(|l| l.selected_id())
    }

    pub fn select(&self, id: usize) {
        self.log.update(|l| l.select(id));
    }
}

impl Default for InspectorStore {
    fn default() -> Self {
        Self::new()
    }
}
