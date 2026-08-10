use crate::plugin::{PluginContext, PluginError};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

type BoxFuture<'a> = Pin<Box<dyn Future<Output = Result<(), PluginError>> + Send + 'a>>;

pub struct BaseDataRetriever {
    is_reconnecting: AtomicBool,
    attempt_count: AtomicU32,
    max_attempts: u32,
    base_delay: Duration,
    max_delay: Duration,
}

impl BaseDataRetriever {
    pub fn new(max_attempts: u32, base_delay: Duration, max_delay: Duration) -> Self {
        Self {
            is_reconnecting: AtomicBool::new(false),
            attempt_count: AtomicU32::new(0),
            max_attempts,
            base_delay,
            max_delay,
        }
    }

    pub fn new_default() -> Self {
        Self::new(5, Duration::from_millis(500), Duration::from_secs(30))
    }

    pub fn is_reconnecting(&self) -> bool {
        self.is_reconnecting.load(Ordering::Relaxed)
    }

    pub fn attempt_count(&self) -> u32 {
        self.attempt_count.load(Ordering::Relaxed)
    }

    pub async fn start_with_reconnect<F>(
        &self,
        _ctx: Arc<dyn PluginContext>,
        internal_start: F,
    ) -> Result<(), PluginError>
    where
        F: Fn() -> BoxFuture<'static>,
    {
        if self
            .is_reconnecting
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            tracing::warn!("reconnection already in progress, skipping");
            return Ok(());
        }

        self.attempt_count.store(0, Ordering::Relaxed);
        let result = self.reconnect_loop(internal_start).await;

        self.is_reconnecting.store(false, Ordering::Relaxed);
        result
    }

    async fn reconnect_loop<F>(&self, internal_start: F) -> Result<(), PluginError>
    where
        F: Fn() -> BoxFuture<'static>,
    {
        loop {
            let attempt = self.attempt_count.fetch_add(1, Ordering::Relaxed) + 1;

            let result = internal_start().await;

            match result {
                Ok(()) => {
                    self.attempt_count.store(0, Ordering::Relaxed);
                    return Ok(());
                }
                Err(e) => {
                    if attempt >= self.max_attempts {
                        tracing::error!(
                            attempt,
                            max = self.max_attempts,
                            error = %e,
                            "reconnection exhausted"
                        );
                        return Err(PluginError::StartFailed(format!(
                            "after {} attempts: {}",
                            attempt, e
                        )));
                    }

                    let delay = self.backoff_delay(attempt);
                    tracing::warn!(
                        attempt,
                        delay_ms = delay.as_millis(),
                        error = %e,
                        "reconnect attempt failed, backing off"
                    );
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    fn backoff_delay(&self, attempt: u32) -> Duration {
        let exp = 2u32.saturating_pow(attempt);
        let base = self.base_delay.as_millis() as u64 * exp as u64;
        let capped = base.min(self.max_delay.as_millis() as u64);
        let jitter = (capped / 10).max(1);
        let total = capped + jitter;
        Duration::from_millis(total)
    }
}

impl Default for BaseDataRetriever {
    fn default() -> Self {
        Self::new_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::{OrderBookHub, ProviderHub, TradeHub};
    use crate::model::order_book::OrderBook;
    use crate::model::provider::Provider;
    use crate::model::trade::Trade;
    use async_trait::async_trait;
    use rust_decimal::Decimal;
    use std::sync::atomic::AtomicU32;
    use time::OffsetDateTime;

    fn make_internal_start(
        fail_times: u32,
        counter: Arc<AtomicU32>,
    ) -> impl Fn() -> BoxFuture<'static> {
        move || {
            let c = counter.clone();
            Box::pin(async move {
                let n = c.fetch_add(1, Ordering::Relaxed) + 1;
                if n <= fail_times {
                    Err(PluginError::Generic(format!("fail {}", n)))
                } else {
                    Ok(())
                }
            })
        }
    }

    struct MockCtx;

    #[async_trait]
    impl crate::plugin::PluginContext for MockCtx {
        async fn publish_order_book(&self, _: OrderBook) {}
        async fn publish_trade(&self, _: Trade) {}
        async fn publish_provider(&self, _: Provider) {}
        async fn register_metric(
            &self,
            _: &str,
            _: &str,
            _: &str,
            _: &str,
            _: Decimal,
            _: OffsetDateTime,
        ) {
        }
        fn order_book_hub(&self) -> Arc<OrderBookHub> {
            Arc::new(OrderBookHub::new())
        }
        fn trade_hub(&self) -> Arc<TradeHub> {
            Arc::new(TradeHub::new())
        }
        fn provider_hub(&self) -> Arc<ProviderHub> {
            Arc::new(ProviderHub::new())
        }
    }

    #[tokio::test]
    async fn succeeds_on_first_try() {
        let retriever =
            BaseDataRetriever::new(5, Duration::from_millis(1), Duration::from_millis(10));
        let counter = Arc::new(AtomicU32::new(0));
        let f = make_internal_start(0, counter.clone());
        retriever
            .start_with_reconnect(Arc::new(MockCtx), f)
            .await
            .unwrap();
        assert_eq!(counter.load(Ordering::Relaxed), 1); // called once, succeeded
        assert_eq!(retriever.attempt_count(), 0); // reset on success
    }

    #[tokio::test]
    async fn retries_then_succeeds() {
        let retriever =
            BaseDataRetriever::new(5, Duration::from_millis(1), Duration::from_millis(10));
        let counter = Arc::new(AtomicU32::new(0));
        let f = make_internal_start(2, counter);
        retriever
            .start_with_reconnect(Arc::new(MockCtx), f)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn fails_after_max_attempts() {
        let retriever =
            BaseDataRetriever::new(3, Duration::from_millis(1), Duration::from_millis(10));
        let counter = Arc::new(AtomicU32::new(0));
        let f = make_internal_start(99, counter);
        let result = retriever.start_with_reconnect(Arc::new(MockCtx), f).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn concurrent_reconnect_is_skipped() {
        let retriever = Arc::new(BaseDataRetriever::new(
            5,
            Duration::from_millis(1),
            Duration::from_millis(10),
        ));
        retriever.is_reconnecting.store(true, Ordering::Relaxed);
        let counter = Arc::new(AtomicU32::new(0));
        let f = make_internal_start(0, counter.clone());
        retriever
            .start_with_reconnect(Arc::new(MockCtx), f)
            .await
            .unwrap();
        assert_eq!(counter.load(Ordering::Relaxed), 0);
    }
}
