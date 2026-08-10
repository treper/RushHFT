use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use time::OffsetDateTime;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricEvent {
    pub plugin: String,
    pub metric: String,
    pub exchange: String,
    pub symbol: String,
    pub value: Decimal,
    pub timestamp: OffsetDateTime,
    pub is_replay: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConditionOperator {
    Equals,
    GreaterThan,
    LessThan,
    CrossesAbove,
    CrossesBelow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeWindowUnit {
    Seconds,
    Minutes,
    Hours,
    Days,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeWindow {
    pub value: i32,
    pub unit: TimeWindowUnit,
}

impl TimeWindow {
    pub fn as_duration(&self) -> std::time::Duration {
        let secs = match self.unit {
            TimeWindowUnit::Seconds => self.value as u64,
            TimeWindowUnit::Minutes => self.value as u64 * 60,
            TimeWindowUnit::Hours => self.value as u64 * 3600,
            TimeWindowUnit::Days => self.value as u64 * 86400,
        };
        std::time::Duration::from_secs(secs)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionType {
    RestApi,
    UIAlert,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestApiConfig {
    pub url: String,
    pub method: String,
    pub headers: std::collections::HashMap<String, String>,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerCondition {
    pub condition_id: i64,
    pub plugin: String,
    pub metric: String,
    pub exchange: String,
    pub symbol: String,
    pub operator: ConditionOperator,
    pub threshold: Decimal,
    pub window: Option<TimeWindow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerAction {
    pub action_type: ActionType,
    pub cooldown_duration: i32,
    pub cooldown_unit: TimeWindowUnit,
    pub rest_api: Option<RestApiConfig>,
}

impl TriggerAction {
    pub fn cooldown(&self) -> std::time::Duration {
        let secs = match self.cooldown_unit {
            TimeWindowUnit::Seconds => self.cooldown_duration as u64,
            TimeWindowUnit::Minutes => self.cooldown_duration as u64 * 60,
            TimeWindowUnit::Hours => self.cooldown_duration as u64 * 3600,
            TimeWindowUnit::Days => self.cooldown_duration as u64 * 86400,
        };
        std::time::Duration::from_secs(secs)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerRule {
    pub rule_id: i64,
    pub name: String,
    pub is_enabled: bool,
    pub conditions: Vec<TriggerCondition>,
    pub actions: Vec<TriggerAction>,
}

#[derive(Debug, Clone)]
pub struct TriggerFiredEventArgs {
    pub rule: TriggerRule,
    pub metric_event: MetricEvent,
    pub action_index: usize,
}

fn metric_key(plugin: &str, metric: &str, exchange: &str, symbol: &str) -> String {
    format!("{}|{}|{}|{}", plugin, metric, exchange, symbol)
}

fn condition_key(rule_id: i64, condition_id: i64) -> String {
    format!("r{}|c{}", rule_id, condition_id)
}

fn action_key(rule_id: i64, action_index: usize) -> String {
    format!("r{}|a{}", rule_id, action_index)
}

pub struct TriggerEngine {
    rules: tokio::sync::RwLock<Vec<TriggerRule>>,
    last_metric_values: dashmap::DashMap<String, (Decimal, OffsetDateTime)>,
    condition_start_times: dashmap::DashMap<String, OffsetDateTime>,
    action_last_fired_times: dashmap::DashMap<String, OffsetDateTime>,
    metric_tx: mpsc::UnboundedSender<MetricEvent>,
    metric_rx: tokio::sync::Mutex<Option<mpsc::UnboundedReceiver<MetricEvent>>>,
    #[allow(clippy::type_complexity)]
    on_trigger_fired: Arc<arc_swap::ArcSwap<Vec<Arc<dyn Fn(TriggerFiredEventArgs) + Send + Sync>>>>,
}

impl TriggerEngine {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            rules: tokio::sync::RwLock::new(Vec::new()),
            last_metric_values: dashmap::DashMap::new(),
            condition_start_times: dashmap::DashMap::new(),
            action_last_fired_times: dashmap::DashMap::new(),
            metric_tx: tx,
            metric_rx: tokio::sync::Mutex::new(Some(rx)),
            on_trigger_fired: Arc::new(arc_swap::ArcSwap::from_pointee(Vec::new())),
        }
    }

    pub fn register_metric(&self, event: MetricEvent) {
        let _ = self.metric_tx.send(event);
    }

    pub async fn add_or_update_rule(&self, rule: TriggerRule) {
        let mut rules = self.rules.write().await;
        let key = rule.rule_id;
        if let Some(existing) = rules.iter_mut().find(|r| r.rule_id == key) {
            *existing = rule;
        } else {
            rules.push(rule);
        }
        drop(rules);
        self.replay_latest_metrics().await;
    }

    pub async fn remove_rule(&self, rule_id: i64) {
        let mut rules = self.rules.write().await;
        rules.retain(|r| r.rule_id != rule_id);
    }

    pub async fn get_rules(&self) -> Vec<TriggerRule> {
        self.rules.read().await.clone()
    }

    pub fn on_trigger_fired(&self, f: Arc<dyn Fn(TriggerFiredEventArgs) + Send + Sync>) {
        self.on_trigger_fired.rcu(|current| {
            let mut new_list = (**current).clone();
            new_list.push(f.clone());
            Arc::new(new_list)
        });
    }

    pub async fn start(self: Arc<Self>) {
        let mut rx_guard = self.metric_rx.lock().await;
        let mut rx = rx_guard.take().expect("engine already started");
        drop(rx_guard);
        while let Some(event) = rx.recv().await {
            self.process_metric(event).await;
        }
    }

    async fn process_metric(&self, event: MetricEvent) {
        let key = metric_key(&event.plugin, &event.metric, &event.exchange, &event.symbol);

        let prev_value = self
            .last_metric_values
            .get(&key)
            .map(|e| e.0)
            .unwrap_or(event.value);

        self.last_metric_values
            .insert(key, (event.value, event.timestamp));

        if event.is_replay {
            self.update_condition_state(&event, prev_value).await;
            return;
        }

        let rules = self.rules.read().await;
        for rule in rules.iter() {
            if !rule.is_enabled {
                continue;
            }
            let matches = rule.conditions.iter().any(|c| {
                c.plugin == event.plugin
                    && c.metric == event.metric
                    && c.exchange == event.exchange
                    && c.symbol == event.symbol
            });
            if !matches {
                continue;
            }

            let all_satisfied = self.evaluate_all_conditions(rule, &event, prev_value).await;
            if all_satisfied {
                for (idx, action) in rule.actions.iter().enumerate() {
                    let akey = action_key(rule.rule_id, idx);
                    if self.is_in_cooldown(&akey, event.timestamp, action.cooldown()) {
                        continue;
                    }
                    self.action_last_fired_times.insert(akey, event.timestamp);
                    let args = TriggerFiredEventArgs {
                        rule: rule.clone(),
                        metric_event: event.clone(),
                        action_index: idx,
                    };
                    self.fire_callbacks(args);
                }
            }
        }
    }

    async fn update_condition_state(&self, event: &MetricEvent, prev_value: Decimal) {
        let rules = self.rules.read().await;
        for rule in rules.iter() {
            if !rule.is_enabled {
                continue;
            }
            let _ = self.evaluate_all_conditions(rule, event, prev_value).await;
        }
    }

    async fn evaluate_all_conditions(
        &self,
        rule: &TriggerRule,
        event: &MetricEvent,
        prev_value: Decimal,
    ) -> bool {
        for cond in &rule.conditions {
            let key = metric_key(&cond.plugin, &cond.metric, &cond.exchange, &cond.symbol);
            let (current_val, current_ts) = match self.last_metric_values.get(&key) {
                Some(e) => (e.0, e.1),
                None => return false,
            };

            let ckey = condition_key(rule.rule_id, cond.condition_id);

            // For crosses operators, we need the prev value of THIS metric
            let cond_prev = if cond.plugin == event.plugin
                && cond.metric == event.metric
                && cond.exchange == event.exchange
                && cond.symbol == event.symbol
            {
                prev_value
            } else {
                // Use the current value as prev (no cross will fire for non-current metrics)
                current_val
            };

            let satisfied = self.evaluate_condition(cond, current_val, cond_prev);

            if satisfied {
                if let Some(window) = &cond.window {
                    let dur = window.as_duration();
                    let start = match self.condition_start_times.get(&ckey) {
                        Some(s) => *s.value(),
                        None => {
                            let _ = self.condition_start_times.insert(ckey.clone(), current_ts);
                            return false;
                        }
                    };
                    let elapsed = current_ts - start;
                    if elapsed < time::Duration::seconds(dur.as_secs() as i64) {
                        return false;
                    }
                }
            } else {
                self.condition_start_times.remove(&ckey);
                return false;
            }
        }
        true
    }

    fn evaluate_condition(&self, cond: &TriggerCondition, current: Decimal, prev: Decimal) -> bool {
        match cond.operator {
            ConditionOperator::Equals => current == cond.threshold,
            ConditionOperator::GreaterThan => current > cond.threshold,
            ConditionOperator::LessThan => current < cond.threshold,
            ConditionOperator::CrossesAbove => prev <= cond.threshold && current > cond.threshold,
            ConditionOperator::CrossesBelow => prev >= cond.threshold && current < cond.threshold,
        }
    }

    fn is_in_cooldown(
        &self,
        key: &str,
        now: OffsetDateTime,
        cooldown: std::time::Duration,
    ) -> bool {
        if let Some(last) = self.action_last_fired_times.get(key) {
            let elapsed = now - *last.value();
            let elapsed_dur = std::time::Duration::from_secs(elapsed.whole_seconds().max(0) as u64);
            return elapsed_dur < cooldown;
        }
        false
    }

    fn fire_callbacks(&self, args: TriggerFiredEventArgs) {
        let subs = self.on_trigger_fired.load();
        for sub in subs.iter() {
            let sub = sub.clone();
            let args = args.clone();
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || sub(args)));
        }
    }

    async fn replay_latest_metrics(&self) {
        let snapshots: Vec<(String, (Decimal, OffsetDateTime))> = self
            .last_metric_values
            .iter()
            .map(|e| (e.key().clone(), *e.value()))
            .collect();
        for (key, (value, ts)) in snapshots {
            let parts: Vec<&str> = key.split('|').collect();
            if parts.len() < 4 {
                continue;
            }
            let event = MetricEvent {
                plugin: parts[0].to_string(),
                metric: parts[1].to_string(),
                exchange: parts[2].to_string(),
                symbol: parts[3].to_string(),
                value,
                timestamp: ts,
                is_replay: true,
            };
            let _ = self.metric_tx.send(event);
        }
    }
}

impl Default for TriggerEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::Duration;

    fn make_event(value: Decimal, ts: i64) -> MetricEvent {
        MetricEvent {
            plugin: "VPIN".into(),
            metric: "vpin".into(),
            exchange: "LongPort".into(),
            symbol: "700.HK".into(),
            value,
            timestamp: OffsetDateTime::from_unix_timestamp(ts).unwrap(),
            is_replay: false,
        }
    }

    fn make_rule(operator: ConditionOperator, threshold: Decimal) -> TriggerRule {
        TriggerRule {
            rule_id: 1,
            name: "test".into(),
            is_enabled: true,
            conditions: vec![TriggerCondition {
                condition_id: 1,
                plugin: "VPIN".into(),
                metric: "vpin".into(),
                exchange: "LongPort".into(),
                symbol: "700.HK".into(),
                operator,
                threshold,
                window: None,
            }],
            actions: vec![TriggerAction {
                action_type: ActionType::UIAlert,
                cooldown_duration: 0,
                cooldown_unit: TimeWindowUnit::Seconds,
                rest_api: None,
            }],
        }
    }

    fn make_windowed_rule(
        operator: ConditionOperator,
        threshold: Decimal,
        window_secs: i32,
    ) -> TriggerRule {
        TriggerRule {
            rule_id: 2,
            name: "windowed".into(),
            is_enabled: true,
            conditions: vec![TriggerCondition {
                condition_id: 2,
                plugin: "VPIN".into(),
                metric: "vpin".into(),
                exchange: "LongPort".into(),
                symbol: "700.HK".into(),
                operator,
                threshold,
                window: Some(TimeWindow {
                    value: window_secs,
                    unit: TimeWindowUnit::Seconds,
                }),
            }],
            actions: vec![TriggerAction {
                action_type: ActionType::UIAlert,
                cooldown_duration: 0,
                cooldown_unit: TimeWindowUnit::Seconds,
                rest_api: None,
            }],
        }
    }

    fn make_cooldown_rule(threshold: Decimal, cooldown_secs: i32) -> TriggerRule {
        TriggerRule {
            rule_id: 3,
            name: "cooldown".into(),
            is_enabled: true,
            conditions: vec![TriggerCondition {
                condition_id: 3,
                plugin: "VPIN".into(),
                metric: "vpin".into(),
                exchange: "LongPort".into(),
                symbol: "700.HK".into(),
                operator: ConditionOperator::GreaterThan,
                threshold,
                window: None,
            }],
            actions: vec![TriggerAction {
                action_type: ActionType::UIAlert,
                cooldown_duration: cooldown_secs,
                cooldown_unit: TimeWindowUnit::Seconds,
                rest_api: None,
            }],
        }
    }

    async fn start_engine(engine: Arc<TriggerEngine>) {
        let e = engine.clone();
        tokio::spawn(async move { e.start().await });
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    #[test]
    fn register_metric_sends_to_channel() {
        let engine = TriggerEngine::new();
        let event = make_event(dec!(0.5), 1000);
        engine.register_metric(event);
    }

    #[test]
    fn time_window_duration() {
        let w = TimeWindow {
            value: 5,
            unit: TimeWindowUnit::Seconds,
        };
        assert_eq!(w.as_duration(), std::time::Duration::from_secs(5));
        let w = TimeWindow {
            value: 3,
            unit: TimeWindowUnit::Minutes,
        };
        assert_eq!(w.as_duration(), std::time::Duration::from_secs(180));
    }

    #[test]
    fn trigger_action_cooldown() {
        let a = TriggerAction {
            action_type: ActionType::UIAlert,
            cooldown_duration: 30,
            cooldown_unit: TimeWindowUnit::Seconds,
            rest_api: None,
        };
        assert_eq!(a.cooldown(), std::time::Duration::from_secs(30));
    }

    #[tokio::test]
    async fn equals_fires_when_equal() {
        let fired = Arc::new(AtomicU32::new(0));
        let f = fired.clone();
        let engine = Arc::new(TriggerEngine::new());
        engine
            .add_or_update_rule(make_rule(ConditionOperator::Equals, dec!(0.5)))
            .await;
        engine.on_trigger_fired(Arc::new(move |_| {
            f.fetch_add(1, Ordering::Relaxed);
        }));
        start_engine(engine.clone()).await;

        engine.register_metric(make_event(dec!(0.5), 1000));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(fired.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn greater_than_fires() {
        let fired = Arc::new(AtomicU32::new(0));
        let f = fired.clone();
        let engine = Arc::new(TriggerEngine::new());
        engine
            .add_or_update_rule(make_rule(ConditionOperator::GreaterThan, dec!(0.7)))
            .await;
        engine.on_trigger_fired(Arc::new(move |_| {
            f.fetch_add(1, Ordering::Relaxed);
        }));
        start_engine(engine.clone()).await;

        engine.register_metric(make_event(dec!(0.8), 1000));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(fired.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn greater_than_does_not_fire_below_threshold() {
        let fired = Arc::new(AtomicU32::new(0));
        let f = fired.clone();
        let engine = Arc::new(TriggerEngine::new());
        engine
            .add_or_update_rule(make_rule(ConditionOperator::GreaterThan, dec!(0.7)))
            .await;
        engine.on_trigger_fired(Arc::new(move |_| {
            f.fetch_add(1, Ordering::Relaxed);
        }));
        start_engine(engine.clone()).await;

        engine.register_metric(make_event(dec!(0.6), 1000));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(fired.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn less_than_fires() {
        let fired = Arc::new(AtomicU32::new(0));
        let f = fired.clone();
        let engine = Arc::new(TriggerEngine::new());
        engine
            .add_or_update_rule(make_rule(ConditionOperator::LessThan, dec!(0.3)))
            .await;
        engine.on_trigger_fired(Arc::new(move |_| {
            f.fetch_add(1, Ordering::Relaxed);
        }));
        start_engine(engine.clone()).await;

        engine.register_metric(make_event(dec!(0.2), 1000));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(fired.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn crosses_above_fires_on_crossing_up() {
        let fired = Arc::new(AtomicU32::new(0));
        let f = fired.clone();
        let engine = Arc::new(TriggerEngine::new());
        engine
            .add_or_update_rule(make_rule(ConditionOperator::CrossesAbove, dec!(0.5)))
            .await;
        engine.on_trigger_fired(Arc::new(move |_| {
            f.fetch_add(1, Ordering::Relaxed);
        }));
        start_engine(engine.clone()).await;

        engine.register_metric(make_event(dec!(0.4), 1000));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(fired.load(Ordering::Relaxed), 0);

        engine.register_metric(make_event(dec!(0.6), 1001));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(fired.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn crosses_below_fires_on_crossing_down() {
        let fired = Arc::new(AtomicU32::new(0));
        let f = fired.clone();
        let engine = Arc::new(TriggerEngine::new());
        engine
            .add_or_update_rule(make_rule(ConditionOperator::CrossesBelow, dec!(0.5)))
            .await;
        engine.on_trigger_fired(Arc::new(move |_| {
            f.fetch_add(1, Ordering::Relaxed);
        }));
        start_engine(engine.clone()).await;

        engine.register_metric(make_event(dec!(0.6), 1000));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(fired.load(Ordering::Relaxed), 0);

        engine.register_metric(make_event(dec!(0.4), 1001));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(fired.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn sustained_window_fires_after_duration() {
        let fired = Arc::new(AtomicU32::new(0));
        let f = fired.clone();
        let engine = Arc::new(TriggerEngine::new());
        engine
            .add_or_update_rule(make_windowed_rule(
                ConditionOperator::GreaterThan,
                dec!(0.5),
                5,
            ))
            .await;
        engine.on_trigger_fired(Arc::new(move |_| {
            f.fetch_add(1, Ordering::Relaxed);
        }));
        start_engine(engine.clone()).await;

        engine.register_metric(make_event(dec!(0.6), 1000));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(fired.load(Ordering::Relaxed), 0);

        engine.register_metric(make_event(dec!(0.6), 1006));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(fired.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn sustained_window_does_not_fire_before_duration() {
        let fired = Arc::new(AtomicU32::new(0));
        let f = fired.clone();
        let engine = Arc::new(TriggerEngine::new());
        engine
            .add_or_update_rule(make_windowed_rule(
                ConditionOperator::GreaterThan,
                dec!(0.5),
                10,
            ))
            .await;
        engine.on_trigger_fired(Arc::new(move |_| {
            f.fetch_add(1, Ordering::Relaxed);
        }));
        start_engine(engine.clone()).await;

        engine.register_metric(make_event(dec!(0.6), 1000));
        tokio::time::sleep(Duration::from_millis(50)).await;

        engine.register_metric(make_event(dec!(0.6), 1005));
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert_eq!(fired.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn condition_becoming_false_resets_window() {
        let fired = Arc::new(AtomicU32::new(0));
        let f = fired.clone();
        let engine = Arc::new(TriggerEngine::new());
        engine
            .add_or_update_rule(make_windowed_rule(
                ConditionOperator::GreaterThan,
                dec!(0.5),
                5,
            ))
            .await;
        engine.on_trigger_fired(Arc::new(move |_| {
            f.fetch_add(1, Ordering::Relaxed);
        }));
        start_engine(engine.clone()).await;

        engine.register_metric(make_event(dec!(0.6), 1000));
        tokio::time::sleep(Duration::from_millis(50)).await;

        engine.register_metric(make_event(dec!(0.3), 1003));
        tokio::time::sleep(Duration::from_millis(50)).await;

        engine.register_metric(make_event(dec!(0.6), 1004));
        tokio::time::sleep(Duration::from_millis(50)).await;

        engine.register_metric(make_event(dec!(0.6), 1010));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(fired.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn cooldown_prevents_refire_within_period() {
        let fired = Arc::new(AtomicU32::new(0));
        let f = fired.clone();
        let engine = Arc::new(TriggerEngine::new());
        engine
            .add_or_update_rule(make_cooldown_rule(dec!(0.5), 10))
            .await;
        engine.on_trigger_fired(Arc::new(move |_| {
            f.fetch_add(1, Ordering::Relaxed);
        }));
        start_engine(engine.clone()).await;

        engine.register_metric(make_event(dec!(0.6), 1000));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(fired.load(Ordering::Relaxed), 1);

        engine.register_metric(make_event(dec!(0.6), 1005));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(fired.load(Ordering::Relaxed), 1);

        engine.register_metric(make_event(dec!(0.6), 1011));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(fired.load(Ordering::Relaxed), 2);
    }

    #[tokio::test]
    async fn replay_does_not_fire_actions() {
        let fired = Arc::new(AtomicU32::new(0));
        let f = fired.clone();
        let engine = Arc::new(TriggerEngine::new());
        engine
            .add_or_update_rule(make_rule(ConditionOperator::GreaterThan, dec!(0.5)))
            .await;
        engine.on_trigger_fired(Arc::new(move |_| {
            f.fetch_add(1, Ordering::Relaxed);
        }));
        start_engine(engine.clone()).await;

        engine.register_metric(make_event(dec!(0.6), 1000));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(fired.load(Ordering::Relaxed), 1);

        let mut replay_event = make_event(dec!(0.6), 1000);
        replay_event.is_replay = true;
        engine.register_metric(replay_event);
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(fired.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn replay_updates_state_only() {
        let engine = Arc::new(TriggerEngine::new());
        engine
            .add_or_update_rule(make_rule(ConditionOperator::GreaterThan, dec!(0.5)))
            .await;
        start_engine(engine.clone()).await;

        let mut replay = make_event(dec!(0.6), 1000);
        replay.is_replay = true;
        engine.register_metric(replay);
        tokio::time::sleep(Duration::from_millis(50)).await;

        let key = "VPIN|vpin|LongPort|700.HK";
        let val = engine.last_metric_values.get(key).unwrap();
        assert_eq!(val.0, dec!(0.6));
    }

    #[tokio::test]
    async fn panicking_callback_does_not_break_others() {
        let fired = Arc::new(AtomicU32::new(0));
        let f = fired.clone();
        let engine = Arc::new(TriggerEngine::new());
        engine
            .add_or_update_rule(make_rule(ConditionOperator::GreaterThan, dec!(0.5)))
            .await;

        engine.on_trigger_fired(Arc::new(|_| {
            panic!("boom in trigger callback");
        }));
        engine.on_trigger_fired(Arc::new(move |_| {
            f.fetch_add(1, Ordering::Relaxed);
        }));
        start_engine(engine.clone()).await;

        engine.register_metric(make_event(dec!(0.6), 1000));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(fired.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn trigger_rule_serialize_deserialize() {
        let rule = TriggerRule {
            rule_id: 1,
            name: "VPIN alert".into(),
            is_enabled: true,
            conditions: vec![TriggerCondition {
                condition_id: 1,
                plugin: "VPIN".into(),
                metric: "vpin".into(),
                exchange: "LongPort".into(),
                symbol: "700.HK".into(),
                operator: ConditionOperator::GreaterThan,
                threshold: dec!(0.7),
                window: Some(TimeWindow {
                    value: 5,
                    unit: TimeWindowUnit::Seconds,
                }),
            }],
            actions: vec![TriggerAction {
                action_type: ActionType::RestApi,
                cooldown_duration: 60,
                cooldown_unit: TimeWindowUnit::Seconds,
                rest_api: Some(RestApiConfig {
                    url: "https://example.com/hook".into(),
                    method: "POST".into(),
                    headers: std::collections::HashMap::from([(
                        "Authorization".into(),
                        "Bearer xxx".into(),
                    )]),
                    body: "{\"alert\":\"vpin\"}".into(),
                }),
            }],
        };

        let toml_str = toml::to_string_pretty(&rule).unwrap();
        let back: TriggerRule = toml::from_str(&toml_str).unwrap();

        assert_eq!(back.rule_id, 1);
        assert_eq!(back.name, "VPIN alert");
        assert!(back.is_enabled);
        assert_eq!(back.conditions.len(), 1);
        assert_eq!(back.conditions[0].operator, ConditionOperator::GreaterThan);
        assert_eq!(back.conditions[0].threshold, dec!(0.7));
        assert!(back.conditions[0].window.is_some());
        assert_eq!(back.actions.len(), 1);
        assert_eq!(back.actions[0].action_type, ActionType::RestApi);
        assert!(back.actions[0].rest_api.is_some());
    }
}
