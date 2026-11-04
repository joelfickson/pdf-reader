use std::collections::{BTreeMap, VecDeque};

pub(super) struct PageCache<T, K = usize> {
    capacity: usize,
    byte_limit: usize,
    bytes: usize,
    measure: fn(&T) -> usize,
    entries: BTreeMap<K, T>,
    order: VecDeque<K>,
}

impl<T, K: Ord + Clone> PageCache<T, K> {
    pub(super) fn new(capacity: usize, byte_limit: usize, measure: fn(&T) -> usize) -> Self {
        Self {
            capacity,
            byte_limit,
            bytes: 0,
            measure,
            entries: BTreeMap::new(),
            order: VecDeque::new(),
        }
    }

    pub(super) fn contains_key(&self, page: &K) -> bool {
        self.entries.contains_key(page)
    }

    pub(super) fn peek(&self, page: &K) -> Option<&T> {
        self.entries.get(page)
    }

    pub(super) fn get(&mut self, page: &K) -> Option<&T> {
        if self.entries.contains_key(page) {
            self.order.retain(|entry| entry != page);
            self.order.push_back(page.clone());
        }
        self.entries.get(page)
    }

    pub(super) fn take(&mut self, page: &K) -> Option<T> {
        let value = self.entries.remove(page)?;
        self.bytes -= (self.measure)(&value);
        self.order.retain(|entry| entry != page);
        Some(value)
    }

    pub(super) fn insert(&mut self, page: K, value: T) -> Vec<T> {
        let mut evicted = Vec::new();
        if let Some(old) = self.take(&page) {
            evicted.push(old);
        }
        let bytes = (self.measure)(&value);
        if bytes > self.byte_limit || self.capacity == 0 {
            evicted.push(value);
            return evicted;
        }
        self.bytes += bytes;
        self.entries.insert(page.clone(), value);
        self.order.push_back(page);
        while self.order.len() > self.capacity || self.bytes > self.byte_limit {
            if let Some(oldest) = self.order.pop_front()
                && let Some(old) = self.entries.remove(&oldest)
            {
                self.bytes -= (self.measure)(&old);
                evicted.push(old);
            }
        }
        evicted
    }

    pub(super) fn remove_where(&mut self, predicate: impl Fn(&K) -> bool) -> Vec<T> {
        let keys: Vec<_> = self
            .entries
            .keys()
            .filter(|key| predicate(key))
            .cloned()
            .collect();
        keys.into_iter().filter_map(|key| self.take(&key)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_a_tab_releases_only_its_entries_and_budget() {
        let mut cache = PageCache::new(8, 20, |value: &Vec<u8>| value.len());
        cache.insert((1, 0), vec![0; 6]);
        cache.insert((2, 0), vec![1; 6]);
        cache.insert((1, 1), vec![2; 6]);
        assert_eq!(cache.remove_where(|key| key.0 == 1).len(), 2);
        assert_eq!(cache.bytes, 6);
        assert!(cache.contains_key(&(2, 0)));
        assert!(cache.insert((3, 0), vec![3; 14]).is_empty());
        assert_eq!(cache.bytes, 20);
    }

    #[test]
    fn evicts_least_recently_viewed_page() {
        let mut cache = PageCache::new(2, 100, |value: &&str| value.len());
        cache.insert(0, "first");
        cache.insert(1, "second");
        assert_eq!(cache.get(&0), Some(&"first"));
        assert_eq!(cache.insert(2, "third"), vec!["second"]);
        assert!(!cache.contains_key(&1));
        assert_eq!(cache.get(&0), Some(&"first"));
    }

    #[test]
    fn replacement_returns_old_image_without_consuming_an_extra_slot() {
        let mut cache = PageCache::new(2, 100, |value: &&str| value.len());
        cache.insert(0, "old");
        assert_eq!(cache.insert(0, "new"), vec!["old"]);
        assert!(cache.insert(1, "second").is_empty());
        assert_eq!(cache.get(&0), Some(&"new"));
        assert_eq!(cache.remove_where(|_| true), vec!["new", "second"]);
        assert!(!cache.contains_key(&0));
    }

    #[test]
    fn byte_budget_evicts_images_and_take_transfers_ownership() {
        let mut cache = PageCache::new(8, 10, |value: &Vec<u8>| value.len());
        cache.insert(0, vec![0; 6]);
        assert_eq!(cache.insert(1, vec![1; 6]), vec![vec![0; 6]]);
        assert_eq!(cache.bytes, 6);
        assert_eq!(cache.take(&1), Some(vec![1; 6]));
        assert_eq!(cache.bytes, 0);
        assert_eq!(cache.take(&1), None);
        cache.insert(2, vec![2; 4]);
        assert_eq!(cache.insert(3, vec![3; 11]), vec![vec![3; 11]]);
        assert!(cache.contains_key(&2));
        cache.remove_where(|_| true);
        assert_eq!(cache.bytes, 0);
    }
}
