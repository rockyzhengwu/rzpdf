use crate::error::{PdfError, PdfResult};
use crate::objects::PdfObject;
use std::collections::HashMap;

type BudgetEntry = (
    String,
    PdfObject,
    crate::budget::MemoryLease,
    crate::budget::MemoryLease,
);

/// Temporary ownership of individually charged keys/values makes replacement
/// release only the displaced value. The final table contains no leases.
pub(crate) struct BudgetedDictBuilder {
    entries: Vec<BudgetEntry>,
    table: crate::budget::MemoryLease,
    budget: crate::budget::OperationBudget,
    maximum: usize,
}

impl BudgetedDictBuilder {
    pub(crate) fn new(budget: &crate::budget::OperationBudget, maximum: usize) -> PdfResult<Self> {
        Ok(Self {
            entries: Vec::new(),
            table: budget.reserve(0)?,
            budget: budget.clone(),
            maximum,
        })
    }

    pub(crate) fn contains_key(&self, key: &str) -> bool {
        self.entries
            .binary_search_by(|entry| entry.0.as_str().cmp(key))
            .is_ok()
    }

    pub(crate) fn insert(
        &mut self,
        key: String,
        value: PdfObject,
        key_lease: crate::budget::MemoryLease,
        value_lease: crate::budget::MemoryLease,
    ) -> PdfResult<()> {
        let index = match self.entries.binary_search_by(|entry| entry.0.cmp(&key)) {
            Ok(index) => {
                let old = std::mem::replace(&mut self.entries[index].1, value);
                drop(old);
                self.entries[index].3 = value_lease;
                drop(key);
                drop(key_lease);
                return Ok(());
            }
            Err(index) => index,
        };
        if self.entries.len() >= self.maximum {
            return Err(PdfError::ResourceLimit("dictionary entry count".into()));
        }
        if self.entries.len() == self.entries.capacity() {
            let capacity = self
                .entries
                .capacity()
                .saturating_mul(2)
                .max(1)
                .min(self.maximum);
            let mut lease = self.budget.reserve(
                capacity
                    .checked_mul(std::mem::size_of::<BudgetEntry>())
                    .ok_or_else(|| PdfError::ResourceLimit("dictionary table size".into()))?,
            )?;
            let mut entries = Vec::new();
            entries
                .try_reserve_exact(capacity)
                .map_err(|_| PdfError::ResourceLimit("dictionary table allocation".into()))?;
            lease.resize(
                entries
                    .capacity()
                    .checked_mul(std::mem::size_of::<BudgetEntry>())
                    .ok_or_else(|| PdfError::ResourceLimit("dictionary table capacity".into()))?,
            )?;
            entries.append(&mut self.entries);
            self.entries = entries;
            self.table = lease;
        }
        self.entries
            .insert(index, (key, value, key_lease, value_lease));
        Ok(())
    }

    pub(crate) fn finish(self) -> PdfResult<(PdfDict, crate::budget::MemoryLease)> {
        let mut lease = self.budget.reserve(
            self.entries
                .len()
                .checked_mul(std::mem::size_of::<(String, PdfObject)>())
                .ok_or_else(|| PdfError::ResourceLimit("dictionary result table size".into()))?,
        )?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(self.entries.len())
            .map_err(|_| PdfError::ResourceLimit("dictionary result allocation".into()))?;
        lease.resize(
            entries
                .capacity()
                .checked_mul(std::mem::size_of::<(String, PdfObject)>())
                .ok_or_else(|| PdfError::ResourceLimit("dictionary result capacity".into()))?,
        )?;
        for (key, value, mut key_lease, mut value_lease) in self.entries {
            entries.push((key, value));
            lease.absorb(&mut key_lease)?;
            lease.absorb(&mut value_lease)?;
        }
        Ok((PdfDict::from_sorted_entries(entries), lease))
    }
}

#[derive(Debug, Clone, Default)]
pub struct PdfDict {
    map: HashMap<String, PdfObject>,
    // Budget construction uses a measurable contiguous table, without relying
    // on private HashMap bucket/control-byte allocation details.
    entries: Option<Vec<(String, PdfObject)>>,
}

impl PartialEq for PdfDict {
    fn eq(&self, other: &Self) -> bool {
        self.into_iter().len() == other.into_iter().len()
            && self
                .into_iter()
                .all(|(key, value)| other.get(key) == Some(value))
    }
}

impl PdfDict {
    /// Build a charged replacement while the original dictionary remains live.
    /// The caller swaps only after success and retains the returned lease.
    pub(crate) fn with_replaced_entry_with_budget(
        &self,
        replacement_key: &str,
        replacement: &PdfObject,
        budget: &crate::budget::OperationBudget,
        limits: crate::parser::options::Limits,
    ) -> PdfResult<(Self, crate::budget::MemoryLease)> {
        if limits.max_depth == 0 || self.into_iter().len() > limits.max_objects {
            return Err(PdfError::ResourceLimit(
                "dictionary replacement limits".into(),
            ));
        }
        if replacement_key.len() > limits.max_token_bytes {
            return Err(PdfError::ResourceLimit(
                "dictionary replacement key bytes".into(),
            ));
        }
        let mut builder = BudgetedDictBuilder::new(budget, limits.max_objects)?;
        for (key, value) in self {
            if key == replacement_key {
                continue;
            }
            if key.len() > limits.max_token_bytes {
                return Err(PdfError::ResourceLimit("dictionary clone key bytes".into()));
            }
            let (key, key_lease) = crate::budget::copy_string(key, budget)?;
            let (value, value_lease) =
                crate::objects::clone_object_with_budget(value, budget, limits, 1)?;
            builder.insert(key, value, key_lease, value_lease)?;
        }
        let (key, key_lease) = crate::budget::copy_string(replacement_key, budget)?;
        let (value, value_lease) =
            crate::objects::clone_object_with_budget(replacement, budget, limits, 1)?;
        builder.insert(key, value, key_lease, value_lease)?;
        builder.finish()
    }

    pub fn new(map: HashMap<String, PdfObject>) -> Self {
        Self { map, entries: None }
    }
    pub(crate) fn from_sorted_entries(entries: Vec<(String, PdfObject)>) -> Self {
        debug_assert!(entries.windows(2).all(|pair| pair[0].0 < pair[1].0));
        Self {
            map: HashMap::new(),
            entries: Some(entries),
        }
    }
    pub fn insert(&mut self, key: String, value: PdfObject) {
        if let Some(entries) = &mut self.entries {
            match entries.binary_search_by(|(entry, _)| entry.cmp(&key)) {
                Ok(index) => entries[index].1 = value,
                Err(index) => entries.insert(index, (key, value)),
            }
            return;
        }
        self.map.insert(key, value);
    }

    /// Replace an existing entry without allocating a key or growing the table.
    pub(crate) fn replace_existing(&mut self, key: &str, value: PdfObject) -> PdfResult<()> {
        let target = if let Some(entries) = &mut self.entries {
            entries
                .binary_search_by(|(entry, _)| entry.as_str().cmp(key))
                .ok()
                .map(|index| &mut entries[index].1)
        } else {
            self.map.get_mut(key)
        };
        let target = target
            .ok_or_else(|| PdfError::ObjectError("dictionary replacement key missing".into()))?;
        *target = value;
        Ok(())
    }
    pub fn try_insert(&mut self, key: String, value: PdfObject) -> PdfResult<()> {
        if let Some(entries) = &mut self.entries {
            match entries.binary_search_by(|(entry, _)| entry.cmp(&key)) {
                Ok(index) => entries[index].1 = value,
                Err(index) => {
                    entries.try_reserve(1).map_err(|_| {
                        PdfError::ResourceLimit("dictionary entry allocation".into())
                    })?;
                    entries.insert(index, (key, value));
                }
            }
            return Ok(());
        }
        if !self.map.contains_key(&key) {
            self.map
                .try_reserve(1)
                .map_err(|_| PdfError::ResourceLimit("dictionary entry allocation".into()))?;
        }
        self.map.insert(key, value);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&PdfObject> {
        if let Some(entries) = &self.entries {
            return entries
                .binary_search_by(|(entry, _)| entry.as_str().cmp(key))
                .ok()
                .map(|index| &entries[index].1);
        }
        self.map.get(key)
    }

    pub fn remove(&mut self, key: &str) -> Option<PdfObject> {
        if let Some(entries) = &mut self.entries {
            return entries
                .binary_search_by(|(entry, _)| entry.as_str().cmp(key))
                .ok()
                .map(|index| entries.remove(index).1);
        }
        self.map.remove(key)
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.get(key).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn budgeted_replacement_preserves_input_and_checks_dictionary_limits() {
        use crate::{budget::OperationBudget, parser::options::Limits};
        let pool = OperationBudget::new(65536);
        let mut original = PdfDict::default();
        original.insert("A".into(), PdfObject::PdfNull);
        for limits in [
            Limits {
                max_depth: 0,
                ..Default::default()
            },
            Limits {
                max_depth: 1,
                ..Default::default()
            },
            Limits {
                max_objects: 0,
                ..Default::default()
            },
            Limits {
                max_objects: 1,
                ..Default::default()
            },
            Limits {
                max_token_bytes: 0,
                ..Default::default()
            },
        ] {
            assert!(matches!(
                original.with_replaced_entry_with_budget("B", &PdfObject::PdfNull, &pool, limits),
                Err(PdfError::ResourceLimit(_))
            ));
            assert_eq!(pool.live_bytes(), 0);
            assert_eq!(original.into_iter().len(), 1);
            assert_eq!(original.get("A"), Some(&PdfObject::PdfNull));
            assert!(original.get("B").is_none());
        }
        let limits = Limits {
            max_depth: 2,
            max_objects: 1,
            max_token_bytes: 1,
            ..Default::default()
        };
        let (replacement, lease) = original
            .with_replaced_entry_with_budget("A", &PdfObject::PdfNull, &pool, limits)
            .unwrap();
        assert_eq!(replacement, original);
        assert!(lease.bytes() > 0);
        assert_eq!(pool.live_bytes(), lease.bytes());
        drop((replacement, lease));
        assert_eq!(pool.live_bytes(), 0);
        original.insert("Long".into(), PdfObject::PdfNull);
        assert!(original
            .with_replaced_entry_with_budget(
                "B",
                &PdfObject::PdfNull,
                &pool,
                Limits {
                    max_token_bytes: 1,
                    ..Default::default()
                }
            )
            .is_err());
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn replace_existing_preserves_storage_and_rejects_missing_keys() {
        let mut map = PdfDict::default();
        map.insert("Contents".into(), PdfObject::PdfNull);
        let sorted = PdfDict::from_sorted_entries(vec![("Contents".into(), PdfObject::PdfNull)]);
        for mut dict in [map, sorted] {
            let capacity = dict
                .entries
                .as_ref()
                .map(|entries| entries.capacity())
                .unwrap_or(dict.map.capacity());
            dict.replace_existing(
                "Contents",
                PdfObject::PdfBool(crate::objects::pdf_bool::PdfBool::new(b"true")),
            )
            .unwrap();
            assert_eq!(
                dict.get("Contents"),
                Some(&PdfObject::PdfBool(crate::objects::pdf_bool::PdfBool::new(
                    b"true"
                )))
            );
            assert!(dict
                .replace_existing("Missing", PdfObject::PdfNull)
                .is_err());
            assert!(!dict.contains_key("Missing"));
            assert_eq!(
                capacity,
                dict.entries
                    .as_ref()
                    .map(|entries| entries.capacity())
                    .unwrap_or(dict.map.capacity())
            );
            assert_eq!(dict.into_iter().len(), 1);
        }
    }
    #[test]
    fn budgeted_builder_growth_accounts_old_and_new_tables() {
        let size = std::mem::size_of::<BudgetEntry>();
        let budget = crate::budget::OperationBudget::new(6 * size + 3);
        let neighbor = budget.reserve(1).unwrap();
        let mut builder = BudgetedDictBuilder::new(&budget, 8).unwrap();
        let append = |builder: &mut BudgetedDictBuilder, key: &str| {
            let mut key_lease = budget.reserve(key.len())?;
            let key = key.to_owned();
            key_lease.resize(key.capacity())?;
            builder.insert(key, PdfObject::PdfNull, key_lease, budget.reserve(0)?)
        };
        append(&mut builder, "B").unwrap();
        append(&mut builder, "A").unwrap();
        assert_eq!(budget.live_bytes(), 2 * size + 2 + 1);
        assert!(append(&mut builder, "C").is_err());
        assert_eq!(budget.live_bytes(), 2 * size + 2 + 1);
        assert!(!builder.contains_key("C"));
        assert!(builder.contains_key("A"));
        drop(neighbor);
        append(&mut builder, "C").unwrap();
        assert_eq!(budget.live_bytes(), 4 * size + 3);
        assert_eq!(builder.entries.len(), 3);
        drop(builder);
        assert_eq!(budget.live_bytes(), 0);
    }

    #[test]
    fn sorted_storage_matches_hash_storage_for_dictionary_operations() {
        let mut sorted = PdfDict::from_sorted_entries(vec![]);
        let mut hashed = PdfDict::default();
        for key in ["B", "A", "Z", "B"] {
            let value = PdfObject::PdfArray(crate::objects::pdf_array::PdfArray::default());
            sorted.try_insert(key.into(), value.clone()).unwrap();
            hashed.try_insert(key.into(), value).unwrap();
            assert_eq!(sorted, hashed);
            assert_eq!(hashed, sorted);
        }
        assert_eq!(
            sorted
                .into_iter()
                .map(|(key, _)| key.as_str())
                .collect::<Vec<_>>(),
            ["A", "B", "Z"]
        );
        sorted.insert("B".into(), PdfObject::PdfNull);
        hashed.insert("B".into(), PdfObject::PdfNull);
        assert_eq!(sorted, hashed);
        assert_eq!(sorted.clone(), hashed.clone());
        let mut iter = (&sorted).into_iter();
        assert_eq!(iter.len(), 3);
        assert!(iter.next().is_some());
        assert_eq!(iter.len(), 2);
        for key in ["B", "missing", "A", "Z"] {
            assert_eq!(sorted.remove(key), hashed.remove(key));
            assert_eq!(sorted, hashed);
            assert_eq!(sorted.contains_key(key), hashed.contains_key(key));
        }
        assert_eq!(sorted.into_iter().len(), 0);
    }

    #[test]
    fn fallible_insert_preserves_replacement_semantics() {
        let mut dict = PdfDict::default();
        dict.try_insert("A".into(), PdfObject::PdfNull).unwrap();
        let value = PdfObject::PdfDict(PdfDict::default());
        dict.try_insert("A".into(), value.clone()).unwrap();
        assert_eq!(dict.get("A"), Some(&value));
        assert_eq!((&dict).into_iter().count(), 1);
        dict.try_insert("B".into(), PdfObject::PdfNull).unwrap();
        assert_eq!((&dict).into_iter().count(), 2);
    }
}
impl<'a> IntoIterator for &'a PdfDict {
    type Item = (&'a String, &'a PdfObject);
    type IntoIter = PdfDictIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        match &self.entries {
            Some(entries) => PdfDictIterator::Entries(entries.iter()),
            None => PdfDictIterator::Map(self.map.iter()),
        }
    }
}

#[derive(Clone, Debug)]
pub enum PdfDictIterator<'a> {
    Map(std::collections::hash_map::Iter<'a, String, PdfObject>),
    Entries(std::slice::Iter<'a, (String, PdfObject)>),
}

impl<'a> Iterator for PdfDictIterator<'a> {
    type Item = (&'a String, &'a PdfObject);
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Map(iter) => iter.next(),
            Self::Entries(iter) => iter.next().map(|(key, value)| (key, value)),
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::Map(iter) => iter.size_hint(),
            Self::Entries(iter) => iter.size_hint(),
        }
    }
}
impl ExactSizeIterator for PdfDictIterator<'_> {}
impl std::iter::FusedIterator for PdfDictIterator<'_> {}
