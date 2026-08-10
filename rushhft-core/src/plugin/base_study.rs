use crate::model::enums::AggregationLevel;
use crate::model::study::BaseStudyModel;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct AggregatedCollection {
    level: AggregationLevel,
    items: VecDeque<(i64, BaseStudyModel)>, // (bucket_epoch_secs, item)
}

impl AggregatedCollection {
    pub fn new(level: AggregationLevel) -> Self {
        Self {
            level,
            items: VecDeque::new(),
        }
    }

    pub fn push(&mut self, item: BaseStudyModel) {
        let bucket = self.bucket_start(item.timestamp);
        match self.items.back() {
            Some((last_bucket, _)) if *last_bucket == bucket => {
                if let Some(back) = self.items.back_mut() {
                    back.1 = item;
                }
            }
            _ => {
                self.items.push_back((bucket, item));
                while self.items.len() > 1000 {
                    self.items.pop_front();
                }
            }
        }
    }

    pub fn last(&self) -> Option<&BaseStudyModel> {
        self.items.back().map(|(_, i)| i)
    }

    fn bucket_start(&self, ts: time::OffsetDateTime) -> i64 {
        let unix = ts.unix_timestamp();
        match self.level {
            AggregationLevel::None
            | AggregationLevel::Ms1
            | AggregationLevel::Ms10
            | AggregationLevel::Ms100
            | AggregationLevel::Ms500
            | AggregationLevel::S1 => unix,
            AggregationLevel::S3 => unix / 3 * 3,
            AggregationLevel::S5 => unix / 5 * 5,
            AggregationLevel::D1 => unix / 86400 * 86400,
        }
    }
}

pub struct BaseStudy {
    tx: tokio::sync::mpsc::UnboundedSender<BaseStudyModel>,
    rx: Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<BaseStudyModel>>>,
    agg_level: AggregationLevel,
}

impl BaseStudy {
    pub fn new(agg_level: AggregationLevel) -> Self {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        Self {
            tx,
            rx: Mutex::new(Some(rx)),
            agg_level,
        }
    }

    pub fn add_calculation(&self, e: BaseStudyModel) {
        let _ = self.tx.send(e);
    }

    pub async fn start_consumer<F>(&self, on_calculated: F)
    where
        F: Fn(&BaseStudyModel) + Send + Sync + 'static,
    {
        let on_calculated = Arc::new(on_calculated);
        let mut rx_guard = self.rx.lock().await;
        let mut rx = rx_guard.take().expect("consumer already started");
        let mut agg = AggregatedCollection::new(self.agg_level);
        while let Some(item) = rx.recv().await {
            agg.push(item.clone());
            if let Some(last) = agg.last() {
                on_calculated(last);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::Duration;

    #[test]
    fn aggregated_collection_replaces_within_same_second() {
        let mut agg = AggregatedCollection::new(AggregationLevel::S1);
        let ts = time::OffsetDateTime::from_unix_timestamp(1000).unwrap();

        agg.push(BaseStudyModel {
            value: dec!(0.1),
            format: "0.00".into(),
            timestamp: ts,
            market_mid_price: dec!(100),
            value_color: "".into(),
            tooltip: "".into(),
            has_error: false,
            is_stale: false,
        });
        agg.push(BaseStudyModel {
            value: dec!(0.2),
            format: "0.00".into(),
            timestamp: ts,
            market_mid_price: dec!(100),
            value_color: "".into(),
            tooltip: "".into(),
            has_error: false,
            is_stale: false,
        });

        assert_eq!(agg.items.len(), 1);
        assert_eq!(agg.last().unwrap().value, dec!(0.2));
    }

    #[test]
    fn aggregated_collection_pushes_new_bucket() {
        let mut agg = AggregatedCollection::new(AggregationLevel::S1);
        let ts1 = time::OffsetDateTime::from_unix_timestamp(1000).unwrap();
        let ts2 = time::OffsetDateTime::from_unix_timestamp(1001).unwrap();

        agg.push(BaseStudyModel {
            value: dec!(0.1),
            format: "0.00".into(),
            timestamp: ts1,
            market_mid_price: dec!(100),
            value_color: "".into(),
            tooltip: "".into(),
            has_error: false,
            is_stale: false,
        });
        agg.push(BaseStudyModel {
            value: dec!(0.2),
            format: "0.00".into(),
            timestamp: ts2,
            market_mid_price: dec!(100),
            value_color: "".into(),
            tooltip: "".into(),
            has_error: false,
            is_stale: false,
        });

        assert_eq!(agg.items.len(), 2);
    }

    #[tokio::test]
    async fn base_study_consumer_receives_items() {
        let study = Arc::new(BaseStudy::new(AggregationLevel::S1));
        let received = Arc::new(AtomicU32::new(0));
        let r = received.clone();

        let s = study.clone();
        tokio::spawn(async move {
            s.start_consumer(move |_item| {
                r.fetch_add(1, Ordering::Relaxed);
            })
            .await;
        });

        tokio::time::sleep(Duration::from_millis(10)).await;

        study.add_calculation(BaseStudyModel {
            value: dec!(0.5),
            format: "0.00".into(),
            timestamp: time::OffsetDateTime::now_utc(),
            market_mid_price: dec!(100),
            value_color: "".into(),
            tooltip: "".into(),
            has_error: false,
            is_stale: false,
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(received.load(Ordering::Relaxed) >= 1);
    }
}
