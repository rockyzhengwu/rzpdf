//! Small revision-scoped LRU for disposable derived data. Byte weights are
//! conservative retained-storage estimates, not an allocator or an RSS limit.
#[derive(Debug)]
pub(crate) struct DerivedCache<K, V> {
    revision: Option<u64>,
    entries: Vec<(K, V, usize)>,
    bytes: usize,
    maximum_bytes: usize,
    maximum_entries: usize,
    pub hits: u64,
    pub misses: u64,
}
impl<K: PartialEq, V: Clone> DerivedCache<K, V> {
    pub fn new(maximum_bytes: usize, maximum_entries: usize) -> Self {
        Self {
            revision: None,
            entries: Vec::new(),
            bytes: 0,
            maximum_bytes,
            maximum_entries,
            hits: 0,
            misses: 0,
        }
    }
    fn revision(&mut self, revision: u64) {
        if self.revision != Some(revision) {
            self.clear();
            self.revision = Some(revision);
        }
    }
    pub fn get(&mut self, revision: u64, key: &K) -> Option<V> {
        self.revision(revision);
        if let Some(index) = self.entries.iter().position(|entry| &entry.0 == key) {
            self.hits += 1;
            let entry = self.entries.remove(index);
            let result = entry.1.clone();
            self.entries.push(entry);
            Some(result)
        } else {
            self.misses += 1;
            None
        }
    }
    pub fn insert(&mut self, revision: u64, key: K, value: V, bytes: usize) {
        self.revision(revision);
        if bytes > self.maximum_bytes || self.maximum_entries == 0 {
            return;
        }
        if let Some(index) = self.entries.iter().position(|entry| entry.0 == key) {
            self.bytes -= self.entries.remove(index).2;
        }
        while !self.entries.is_empty()
            && (self.bytes.saturating_add(bytes) > self.maximum_bytes
                || self.entries.len() >= self.maximum_entries)
        {
            self.bytes -= self.entries.remove(0).2;
        }
        // Failure to retain an optimization must not fail an otherwise valid operation.
        if self.entries.try_reserve(1).is_err() {
            return;
        }
        self.entries.push((key, value, bytes));
        self.bytes += bytes;
    }
    pub fn clear(&mut self) {
        self.entries.clear();
        self.bytes = 0;
    }
    pub fn bytes(&self) -> usize {
        self.bytes
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lru_revision_and_oversized_entries() {
        let mut cache = DerivedCache::new(8, 2);
        cache.insert(1, 1, 10, 4);
        cache.insert(1, 2, 20, 4);
        assert_eq!(cache.get(1, &1), Some(10));
        cache.insert(1, 3, 30, 4);
        assert_eq!(cache.get(1, &2), None);
        cache.insert(1, 4, 40, 9);
        assert_eq!((cache.bytes(), cache.len()), (8, 2));
        assert_eq!(cache.get(2, &1), None);
        assert_eq!((cache.bytes(), cache.len()), (0, 0));
        cache.insert(2, 1, 50, 4);
        cache.insert(2, 1, 60, 6);
        assert_eq!((cache.bytes(), cache.len()), (6, 1));
        cache.clear();
        assert_eq!(cache.bytes(), 0);
    }
}
