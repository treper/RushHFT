//! Runtime symbol registry: the set of symbols the user has added through
//! the UI (separate from `Settings.default_symbols` which is loaded at start).
#![allow(dead_code)]

use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Default)]
pub struct UserSymbols {
    inner: Arc<RwLock<HashSet<String>>>,
}

impl UserSymbols {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    pub async fn add(&self, symbol: &str) -> bool {
        let mut g = self.inner.write().await;
        g.insert(symbol.to_string())
    }

    pub async fn remove(&self, symbol: &str) -> bool {
        let mut g = self.inner.write().await;
        g.remove(symbol)
    }

    pub async fn list(&self) -> Vec<String> {
        let g = self.inner.read().await;
        let mut v: Vec<String> = g.iter().cloned().collect();
        v.sort();
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn add_returns_true_on_new_symbol_false_on_dup() {
        let u = UserSymbols::new();
        assert!(u.add("700.HK").await);
        assert!(!u.add("700.HK").await);
        assert_eq!(u.list().await, vec!["700.HK".to_string()]);
    }

    #[tokio::test]
    async fn remove_returns_true_only_when_present() {
        let u = UserSymbols::new();
        u.add("AAPL.US").await;
        assert!(u.remove("AAPL.US").await);
        assert!(!u.remove("AAPL.US").await);
    }
}
