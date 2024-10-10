use super::syncer::Syncer;

pub struct SyncService {
    syncer: Syncer,
}

impl SyncService {
    pub fn new(syncer: Syncer) -> Self {
        Self { syncer }
    }
}
