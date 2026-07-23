use std::collections::{BTreeMap, BTreeSet};

use ae_sdd_domain::GateKeyDigest;

use crate::SelectorId;

/// Reverse dependency index used to invalidate cached Gate results precisely.
#[derive(Clone, Debug, Default)]
pub struct SelectorCacheIndex {
    by_selector: BTreeMap<SelectorId, BTreeSet<GateKeyDigest>>,
    by_key: BTreeMap<GateKeyDigest, BTreeSet<SelectorId>>,
}

impl SelectorCacheIndex {
    pub fn register(
        &mut self,
        key: GateKeyDigest,
        selectors: impl IntoIterator<Item = SelectorId>,
    ) {
        self.remove_key(key);
        let selectors: BTreeSet<_> = selectors.into_iter().collect();
        for selector in &selectors {
            self.by_selector
                .entry(selector.clone())
                .or_default()
                .insert(key);
        }
        self.by_key.insert(key, selectors);
    }

    pub fn invalidate(
        &mut self,
        selectors: impl IntoIterator<Item = SelectorId>,
    ) -> BTreeSet<GateKeyDigest> {
        let mut invalidated = BTreeSet::new();
        for selector in selectors {
            if let Some(keys) = self.by_selector.remove(&selector) {
                invalidated.extend(keys);
            }
        }
        for key in invalidated.iter().copied().collect::<Vec<_>>() {
            self.remove_key(key);
        }
        invalidated
    }

    pub fn clear(&mut self) -> BTreeSet<GateKeyDigest> {
        let keys = self.by_key.keys().copied().collect();
        self.by_key.clear();
        self.by_selector.clear();
        keys
    }

    pub fn len(&self) -> usize {
        self.by_key.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_key.is_empty()
    }

    fn remove_key(&mut self, key: GateKeyDigest) {
        let Some(selectors) = self.by_key.remove(&key) else {
            return;
        };
        for selector in selectors {
            let remove_selector = self.by_selector.get_mut(&selector).is_some_and(|keys| {
                keys.remove(&key);
                keys.is_empty()
            });
            if remove_selector {
                self.by_selector.remove(&selector);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalidating_one_selector_removes_each_dependent_key_everywhere() {
        let source = SelectorId::new("source").expect("valid selector");
        let config = SelectorId::new("config").expect("valid selector");
        let key = GateKeyDigest::digest(b"gate-key");
        let mut index = SelectorCacheIndex::default();
        index.register(key, [source.clone(), config.clone()]);

        assert_eq!(index.invalidate([source]), BTreeSet::from([key]));
        assert!(index.invalidate([config]).is_empty());
        assert!(index.is_empty());
    }
}
