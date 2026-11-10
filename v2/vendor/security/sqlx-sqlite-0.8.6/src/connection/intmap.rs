// Bad casts in this module SHOULD NOT result in a SQL injection
// https://github.com/launchbadge/sqlx/issues/3440
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]
use std::cmp::Ordering;
use std::{fmt::Debug, hash::Hash};

/// Simplistic map implementation built on a Vec of Options (index = key)
#[derive(Debug, Clone, Eq)]
pub(crate) struct IntMap<V>(Vec<Option<V>>);

impl<V> Default for IntMap<V> {
    fn default() -> Self {
        IntMap(Vec::new())
    }
}

impl<V> IntMap<V> {
    pub(crate) fn new() -> Self {
        Self(Vec::new())
    }

    pub(crate) fn expand(&mut self, size: i64) -> usize {
        let idx = size.try_into().expect("negative column index unsupported");
        while self.0.len() <= idx {
            self.0.push(None);
        }
        idx
    }

    pub(crate) fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
        self.0.iter_mut().filter_map(Option::as_mut)
    }

    pub(crate) fn values(&self) -> impl Iterator<Item = &V> {
        self.0.iter().filter_map(Option::as_ref)
    }

    pub(crate) fn get(&self, idx: &i64) -> Option<&V> {
        let idx: usize = (*idx)
            .try_into()
            .expect("negative column index unsupported");

        match self.0.get(idx) {
            Some(Some(v)) => Some(v),
            _ => None,
        }
    }

    pub(crate) fn get_mut(&mut self, idx: &i64) -> Option<&mut V> {
        let idx: usize = (*idx)
            .try_into()
            .expect("negative column index unsupported");
        match self.0.get_mut(idx) {
            Some(Some(v)) => Some(v),
            _ => None,
        }
    }

    pub(crate) fn insert(&mut self, idx: i64, value: V) -> Option<V> {
        let idx: usize = self.expand(idx);

        std::mem::replace(&mut self.0[idx], Some(value))
    }

    pub(crate) fn remove(&mut self, idx: &i64) -> Option<V> {
        let idx: usize = (*idx)
            .try_into()
            .expect("negative column index unsupported");

        let item = self.0.get_mut(idx);
        match item {
            Some(content) => content.take(),
            None => None,
        }
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = Option<&V>> {
        self.0.iter().map(Option::as_ref)
    }

    pub(crate) fn iter_entries(&self) -> impl Iterator<Item = (i64, &V)> {
        self.0
            .iter()
            .enumerate()
            .filter_map(|(i, v)| v.as_ref().map(|v: &V| (i as i64, v)))
    }

    pub(crate) fn last_index(&self) -> Option<i64> {
        self.0.iter().rposition(|v| v.is_some()).map(|i| i as i64)
    }
}

impl<V: Default> IntMap<V> {
    pub(crate) fn get_mut_or_default<'a>(&'a mut self, idx: &i64) -> &'a mut V {
        let idx: usize = self.expand(*idx);

        let item: &mut Option<V> = &mut self.0[idx];
        if item.is_none() {
            *item = Some(V::default());
        }

        return self.0[idx].as_mut().unwrap();
    }
}

impl<V: Clone> IntMap<V> {
    pub(crate) fn from_elem(elem: V, len: usize) -> Self {
        Self(vec![Some(elem); len])
    }
    pub(crate) fn from_dense_record(record: &[V]) -> Self {
        Self(record.iter().cloned().map(Some).collect())
    }
}

impl<V: Eq> IntMap<V> {
    /// get the additions to this intmap compared to the prev intmap
    pub(crate) fn diff<'a, 'b, 'c>(
        &'a self,
        prev: &'b Self,
    ) -> impl Iterator<Item = (usize, Option<&'c V>)>
    where
        'a: 'c,
        'b: 'c,
    {
        let self_pad = if prev.0.len() > self.0.len() {
            prev.0.len() - self.0.len()
        } else {
            0
        };
        self.iter()
            .chain(std::iter::repeat(None).take(self_pad))
            .zip(prev.iter().chain(std::iter::repeat(None)))
            .enumerate()
            .filter(|(_i, (n, p))| n != p)
            .map(|(i, (n, _p))| (i, n))
    }
}

impl<V: Hash> Hash for IntMap<V> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        for value in self.values() {
            value.hash(state);
        }
    }
}

impl<V: PartialEq> PartialEq for IntMap<V> {
    fn eq(&self, other: &Self) -> bool {
        match self.0.len().cmp(&other.0.len()) {
            Ordering::Greater => {
                self.0[..other.0.len()] == other.0
                    && self.0[other.0.len()..].iter().all(Option::is_none)
            }
            Ordering::Less => {
                other.0[..self.0.len()] == self.0
                    && other.0[self.0.len()..].iter().all(Option::is_none)
            }
            Ordering::Equal => self.0 == other.0,
        }
    }
}

impl<V: Debug> FromIterator<(i64, V)> for IntMap<V> {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = (i64, V)>,
    {
        let mut result = Self(Vec::new());
        for (idx, val) in iter {
            let idx = result.expand(idx);
            result.0[idx] = Some(val);
        }
        result
    }
}
