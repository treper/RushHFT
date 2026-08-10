use crossbeam_queue::ArrayQueue;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

pub struct ObjectPool<T: Default + Clone + Send + Sync> {
    queue: Arc<ArrayQueue<T>>,
}

pub struct PoolGuard<T: Default + Clone + Send + Sync> {
    queue: Arc<ArrayQueue<T>>,
    item: Option<T>,
}

impl<T: Default + Clone + Send + Sync> ObjectPool<T> {
    pub fn new(capacity: usize) -> Self {
        let queue = Arc::new(ArrayQueue::new(capacity));
        for _ in 0..capacity {
            let _ = queue.push(T::default());
        }
        Self { queue }
    }

    pub fn get(&self) -> PoolGuard<T> {
        let item = self.queue.pop().unwrap_or_else(T::default);
        PoolGuard {
            queue: self.queue.clone(),
            item: Some(item),
        }
    }

    pub fn try_get(&self) -> Option<PoolGuard<T>> {
        self.queue.pop().map(|item| PoolGuard {
            queue: self.queue.clone(),
            item: Some(item),
        })
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }
}

impl<T: Default + Clone + Send + Sync> PoolGuard<T> {
    pub fn get(&self) -> &T {
        self.item.as_ref().unwrap()
    }

    pub fn get_mut(&mut self) -> &mut T {
        self.item.as_mut().unwrap()
    }

    pub fn into_inner(mut self) -> T {
        self.item.take().unwrap()
    }
}

impl<T: Default + Clone + Send + Sync> Deref for PoolGuard<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.item.as_ref().unwrap()
    }
}

impl<T: Default + Clone + Send + Sync> DerefMut for PoolGuard<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.item.as_mut().unwrap()
    }
}

impl<T: Default + Clone + Send + Sync> Drop for PoolGuard<T> {
    fn drop(&mut self) {
        if let Some(item) = self.item.take() {
            let _ = self.queue.push(item);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Default, Clone, PartialEq)]
    struct TestItem {
        value: i32,
    }

    #[test]
    fn get_returns_default_from_pool() {
        let pool: ObjectPool<TestItem> = ObjectPool::new(2);
        let g = pool.get();
        assert_eq!(*g, TestItem { value: 0 });
    }

    #[test]
    fn guard_returns_item_to_pool_on_drop() {
        let pool: ObjectPool<TestItem> = ObjectPool::new(1);
        assert_eq!(pool.len(), 1);
        {
            let _g = pool.get();
            assert_eq!(pool.len(), 0);
        }
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn into_inner_does_not_return_to_pool() {
        let pool: ObjectPool<TestItem> = ObjectPool::new(1);
        let g = pool.get();
        let item = g.into_inner();
        assert_eq!(item, TestItem { value: 0 });
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn modified_item_is_returned_to_pool() {
        let pool: ObjectPool<TestItem> = ObjectPool::new(1);
        {
            let mut g = pool.get();
            g.value = 42;
        }
        let g2 = pool.get();
        assert_eq!(g2.value, 42);
    }

    #[test]
    fn try_get_returns_none_when_empty() {
        let pool: ObjectPool<TestItem> = ObjectPool::new(1);
        let _g = pool.get();
        assert!(pool.try_get().is_none());
    }
}
