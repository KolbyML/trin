use ethportal_api::{types::portal_wire::OfferTrace, Enr, OverlayContentKey, StateContentKey};
use tracing::{debug, enabled, info, Level};

/// Global report for outcomes of offering state content keys from long-running state bridge
#[derive(Default)]
pub struct GlobalOfferReport {
    success: usize,
    failed: usize,
    declined: usize,
}

impl GlobalOfferReport {
    pub fn update(&mut self, trace: &OfferTrace) {
        match trace {
            OfferTrace::Success(_) => self.success += 1,
            OfferTrace::Failed => self.failed += 1,
            OfferTrace::Declined => self.declined += 1,
        }
    }

    pub fn report(&self) {
        let total = self.success + self.failed + self.declined;
        if total == 0 {
            return;
        }
        info!(
            "State offer report: Total Offers: {}. Successful: {}% ({}). Declined: {}% ({}). Failed: {}% ({}).",
            total,
            100 * self.success / total,
            self.success,
            100 * self.declined / total,
            self.declined,
            100 * self.failed / total,
            self.failed,
        );
    }
}

/// Individual report for outcomes of offering a state content key
pub struct OfferReport {
    content_key: StateContentKey,
    /// total number of enrs interested in the content key
    total: usize,
    success: Vec<Enr>,
    failed: Vec<Enr>,
    declined: Vec<Enr>,
}

impl OfferReport {
    pub fn new(content_key: StateContentKey, total: usize) -> Self {
        Self {
            content_key,
            total,
            success: Vec::new(),
            failed: Vec::new(),
            declined: Vec::new(),
        }
    }

    pub fn update(&mut self, enr: &Enr, trace: &OfferTrace) {
        match trace {
            // since the state bridge only offers one content key at a time,
            // we can assume that a successful offer means the lone content key
            // was successfully offered
            OfferTrace::Success(_) => self.success.push(enr.clone()),
            OfferTrace::Failed => self.failed.push(enr.clone()),
            OfferTrace::Declined => self.declined.push(enr.clone()),
        }
        if self.total == self.success.len() + self.failed.len() + self.declined.len() {
            self.report();
        }
    }

    pub fn report(&self) {
        if enabled!(Level::DEBUG) {
            debug!(
                "Successfully offered to {}/{} peers. Content key: {}. Declined: {:?}. Failed: {:?}",
                self.success.len(),
                self.total,
                self.content_key.to_hex(),
                self.declined,
                self.failed,
            );
        } else {
            info!(
                "Successfully offered to {}/{} peers. Content key: {}. Declined: {}. Failed: {}.",
                self.success.len(),
                self.total,
                self.content_key.to_hex(),
                self.declined.len(),
                self.failed.len(),
            );
        }
    }
}

/// Individual report for outcomes of offering a state content key
pub struct GranularOfferReport {
    content_key: StateContentKey,
    /// we will only offer to this total if we have any successful offers
    base_total: usize,
    /// we will keep trying to offer to peers we have a successful offer or reach this total
    total: usize,
    success: Vec<Enr>,
    failed: Vec<Enr>,
    declined: Vec<Enr>,
}

impl GranularOfferReport {
    pub fn new(content_key: StateContentKey, base_total: usize, total: usize) -> Self {
        Self {
            content_key,
            base_total,
            total,
            success: Vec::new(),
            failed: Vec::new(),
            declined: Vec::new(),
        }
    }

    pub fn update(&mut self, enr: &Enr, trace: &OfferTrace) -> bool {
        match trace {
            // since the state bridge only offers one content key at a time,
            // we can assume that a successful offer means the lone content key
            // was successfully offered
            OfferTrace::Success(_) => self.success.push(enr.clone()),
            OfferTrace::Failed => self.failed.push(enr.clone()),
            OfferTrace::Declined => self.declined.push(enr.clone()),
        }

        // if we have no successful offers and we have reached the base total, we should continue
        if self.success.is_empty() && self.len() >= self.base_total && self.len() < self.total {
            return true;
        }

        // if we have reached the base total and have successful offers, we should stop
        if self.len() >= self.base_total && !self.success.is_empty() {
            self.report();
            return false;
        }

        // if we have reached the total, we should stop
        if self.total == self.len() {
            self.report();
            return false;
        }

        return false;
    }

    pub fn len(&self) -> usize {
        self.success.len() + self.failed.len() + self.declined.len()
    }

    pub fn report(&self) {
        info!(
            "Successfully offered to {}/{} peers. Declined: {}. Failed: {}.",
            self.success.len(),
            self.len(),
            self.declined.len(),
            self.failed.len(),
        );
    }
}
