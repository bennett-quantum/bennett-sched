//! Lexical environments share immutable resolved bindings. Rebinding a name
//! changes only that environment; captured wire/region/allocation identities
//! and large descriptor lists keep their original values.
use crate::expansion::Binding;
use std::{borrow::Borrow, collections::BTreeMap, ops::Index, sync::Arc};
#[derive(Clone, Debug, Default)]
pub struct Env(BTreeMap<String, Arc<Binding>>);
impl Env {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn get<Q: Ord + ?Sized>(&self, key: &Q) -> Option<&Binding>
    where
        String: Borrow<Q>,
    {
        self.0.get(key).map(Arc::as_ref)
    }
    pub fn contains_key<Q: Ord + ?Sized>(&self, key: &Q) -> bool
    where
        String: Borrow<Q>,
    {
        self.0.contains_key(key)
    }
    pub fn insert(&mut self, key: String, value: Binding) -> Option<Binding> {
        self.0.insert(key, Arc::new(value)).map(owned)
    }
    pub fn remove<Q: Ord + ?Sized>(&mut self, key: &Q) -> Option<Binding>
    where
        String: Borrow<Q>,
    {
        self.0.remove(key).map(owned)
    }
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Binding)> {
        self.0.iter().map(|(n, b)| (n, b.as_ref()))
    }
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.0.keys()
    }
    pub fn values(&self) -> impl Iterator<Item = &Binding> {
        self.0.values().map(Arc::as_ref)
    }
}
fn owned(value: Arc<Binding>) -> Binding {
    Arc::try_unwrap(value).unwrap_or_else(|value| value.as_ref().clone())
}
impl<Q: Ord + ?Sized> Index<&Q> for Env
where
    String: Borrow<Q>,
{
    type Output = Binding;
    fn index(&self, key: &Q) -> &Binding {
        self.get(key).expect("resolved lexical binding")
    }
}
impl<const N: usize> From<[(String, Binding); N]> for Env {
    fn from(values: [(String, Binding); N]) -> Self {
        values.into_iter().collect()
    }
}
impl FromIterator<(String, Binding)> for Env {
    fn from_iter<T: IntoIterator<Item = (String, Binding)>>(values: T) -> Self {
        Self(values.into_iter().map(|(n, b)| (n, Arc::new(b))).collect())
    }
}
impl Extend<(String, Binding)> for Env {
    fn extend<T: IntoIterator<Item = (String, Binding)>>(&mut self, values: T) {
        self.0
            .extend(values.into_iter().map(|(n, b)| (n, Arc::new(b))))
    }
}
impl IntoIterator for Env {
    type Item = (String, Binding);
    type IntoIter = std::vec::IntoIter<Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        self.0
            .into_iter()
            .map(|(n, b)| (n, owned(b)))
            .collect::<Vec<_>>()
            .into_iter()
    }
}
