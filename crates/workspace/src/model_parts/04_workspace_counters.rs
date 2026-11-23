impl Workspace {
    /// IDs actuales, para tests y persistencia.
    pub fn counters(&self) -> &IdCounters {
        &self.ids
    }
}
