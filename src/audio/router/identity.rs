use super::AppIdentity;

// ── Identity matching ─────────────────────────────────────────────────────────

impl AppIdentity {
    /// Returns true if `self` (the intent identity) matches `candidate` (a live source).
    ///
    /// Match rules (in priority order):
    /// 1. If both have `app_name`, match on `app_name`.
    /// 2. If either `app_name` is None, fall back to `process_binary`.
    /// 3. If `self.process_id` is Some, also require PID match.
    pub fn matches(&self, candidate: &AppIdentity) -> bool {
        let name_matches = match (&self.app_name, &candidate.app_name) {
            (Some(a), Some(b)) => a == b,
            _ => match (&self.process_binary, &candidate.process_binary) {
                (Some(a), Some(b)) => a == b,
                _ => false,
            },
        };
        if !name_matches {
            return false;
        }
        match self.process_id {
            Some(pid) => candidate.process_id == Some(pid),
            None => true,
        }
    }

    /// Build an AppIdentity from raw stream event fields.
    pub fn from_stream(
        app_name: Option<String>,
        process_binary: Option<String>,
        process_id: Option<u32>,
    ) -> Self {
        Self {
            app_name,
            process_binary,
            process_id,
        }
    }
}
