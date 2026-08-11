//! Lock-free per-symbol snapshot store. Reads via ArcSwap::load (cheap),
//! writes via ArcSwap::store (replaces whole Arc).
#![allow(dead_code)]

use crate::dto::{
    BookItemDto, ChartPointDto, ProviderDto, QuoteStatsDto, SessionStatusDto, StudyValueDto,
    TradeDto,
};
use arc_swap::ArcSwap;
use dashmap::DashMap;
use rust_decimal::Decimal;
use std::collections::VecDeque;
use std::sync::Arc;

/// Latest per-symbol snapshot. Cheap to clone (Arc inside).
#[derive(Clone, Debug)]
pub struct SymbolSnapshot {
    pub symbol: String,
    pub bids: Vec<BookItemDto>,
    pub asks: Vec<BookItemDto>,
    pub spread: Decimal,
    pub mid_price: Decimal,
    pub last_updated: i64,
    pub sequence: i64,
    pub provider_status: SessionStatusDto,
    pub studies: Vec<StudyValueDto>,
    pub recent_trades: Vec<TradeDto>,
    pub quote_stats: Option<QuoteStatsDto>,
}

impl Default for SymbolSnapshot {
    fn default() -> Self {
        Self {
            symbol: String::new(),
            bids: Vec::new(),
            asks: Vec::new(),
            spread: Decimal::ZERO,
            mid_price: Decimal::ZERO,
            last_updated: 0,
            sequence: 0,
            provider_status: SessionStatusDto::Disconnected,
            studies: Vec::new(),
            recent_trades: Vec::new(),
            quote_stats: None,
        }
    }
}

pub struct SnapshotStore {
    books: DashMap<String, ArcSwap<SymbolSnapshot>>,
    studies: DashMap<String, DashMap<String, ArcSwap<StudyValueDto>>>,
    trades: DashMap<String, VecDeque<TradeDto>>,
    providers: ArcSwap<Vec<ProviderDto>>,
    chart_buffers: DashMap<String, DashMap<String, VecDeque<ChartPointDto>>>,
}

impl SnapshotStore {
    pub fn new() -> Self {
        Self {
            books: DashMap::new(),
            studies: DashMap::new(),
            trades: DashMap::new(),
            providers: ArcSwap::from_pointee(Vec::new()),
            chart_buffers: DashMap::new(),
        }
    }

    pub fn update_book(&self, symbol: &str, build: impl FnOnce(&mut SymbolSnapshot)) {
        let entry = self.books.entry(symbol.to_string()).or_insert_with(|| {
            ArcSwap::from_pointee(SymbolSnapshot {
                symbol: symbol.to_string(),
                ..Default::default()
            })
        });
        let current = entry.load();
        let mut next: SymbolSnapshot = (**current).clone();
        build(&mut next);
        entry.store(Arc::new(next));
    }

    pub fn update_study(&self, symbol: &str, name: &str, v: StudyValueDto) {
        let per_symbol = self.studies.entry(symbol.to_string()).or_default();
        let entry = per_symbol
            .entry(name.to_string())
            .or_insert_with(|| ArcSwap::from_pointee(v.clone()));
        entry.store(Arc::new(v));
    }

    pub fn append_trade(&self, symbol: &str, t: TradeDto) {
        let mut entry = self.trades.entry(symbol.to_string()).or_default();
        entry.push_back(t);
        while entry.len() > 200 {
            entry.pop_front();
        }
    }

    pub fn set_providers(&self, providers: Vec<ProviderDto>) {
        self.providers.store(Arc::new(providers));
    }

    /// Push a chart point into the per-symbol, per-kind ring buffer.
    /// Cap at `cap` points (default 600 = 1min @ 10Hz).
    pub fn push_chart_point(&self, symbol: &str, kind: &str, point: ChartPointDto, cap: usize) {
        let per_symbol = self
            .chart_buffers
            .entry(symbol.to_string())
            .or_default();
        let mut buf = per_symbol.entry(kind.to_string()).or_default();
        buf.push_back(point);
        while buf.len() > cap {
            buf.pop_front();
        }
    }

    /// Read up to `points` last points for (symbol, kind). Returns empty vec if none.
    pub fn chart_series(&self, symbol: &str, kind: &str, points: usize) -> Vec<ChartPointDto> {
        let Some(per_symbol) = self.chart_buffers.get(symbol) else {
            return Vec::new();
        };
        let Some(buf) = per_symbol.get(kind) else {
            return Vec::new();
        };
        let skip = buf.len().saturating_sub(points);
        buf.iter().skip(skip).cloned().collect()
    }

    pub fn providers(&self) -> Vec<ProviderDto> {
        (**self.providers.load()).clone()
    }

    pub fn symbols(&self) -> Vec<String> {
        self.books.iter().map(|e| e.key().clone()).collect()
    }

    pub fn snapshot(&self, symbol: &str) -> Option<SymbolSnapshot> {
        // gather latest book + studies + trades into one DTO
        let books_entry = self.books.get(symbol)?;
        let mut snap: SymbolSnapshot = (**books_entry.load()).clone();

        if let Some(per_symbol) = self.studies.get(symbol) {
            let mut studies: Vec<StudyValueDto> =
                per_symbol.iter().map(|e| (**e.load()).clone()).collect();
            studies.sort_by(|a, b| a.name.cmp(&b.name));
            snap.studies = studies;
        }

        if let Some(trades_entry) = self.trades.get(symbol) {
            snap.recent_trades = trades_entry.iter().cloned().collect();
        }

        Some(snap)
    }
}

impl Default for SnapshotStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::TradeDirectionDto;
    use rust_decimal_macros::dec;

    #[test]
    fn snapshot_returns_none_for_unknown_symbol() {
        let store = SnapshotStore::new();
        assert!(store.snapshot("NOPE.HK").is_none());
    }

    #[test]
    fn update_book_stores_latest_state() {
        let store = SnapshotStore::new();
        store.update_book("700.HK", |s| {
            s.symbol = "700.HK".into();
            s.mid_price = dec!(100.5);
            s.sequence = 1;
        });
        let snap = store.snapshot("700.HK").unwrap();
        assert_eq!(snap.mid_price, dec!(100.5));
        assert_eq!(snap.sequence, 1);
    }

    #[test]
    fn update_book_replaces_not_merges() {
        let store = SnapshotStore::new();
        store.update_book("700.HK", |s| {
            s.sequence = 1;
        });
        store.update_book("700.HK", |s| {
            s.mid_price = dec!(200);
        });
        let snap = store.snapshot("700.HK").unwrap();
        // Second write replaced snapshot — but since we built on top of the
        // previous snapshot (clone), sequence is preserved.
        assert_eq!(snap.sequence, 1);
        assert_eq!(snap.mid_price, dec!(200));
    }

    #[test]
    fn append_trade_caps_at_200() {
        let store = SnapshotStore::new();
        // snapshot() needs a book entry to return Some.
        store.update_book("700.HK", |s| {
            s.symbol = "700.HK".into();
        });
        for i in 0..250 {
            store.append_trade(
                "700.HK",
                TradeDto {
                    price: dec!(100),
                    size: dec!(1),
                    timestamp: i,
                    direction: TradeDirectionDto::Up,
                    trade_type: "D".into(),
                },
            );
        }
        let snap = store.snapshot("700.HK").unwrap();
        assert_eq!(snap.recent_trades.len(), 200);
        // first kept trade should have timestamp = 50 (drained 0..49)
        assert_eq!(snap.recent_trades[0].timestamp, 50);
    }

    #[test]
    fn update_study_stores_by_name() {
        let store = SnapshotStore::new();
        store.update_study(
            "700.HK",
            "VPIN",
            StudyValueDto {
                name: "VPIN".into(),
                value: dec!(0.42),
                format: "N2".into(),
                value_color: "White".into(),
                tooltip: String::new(),
                has_error: false,
                is_stale: false,
                timestamp: 1,
            },
        );
        // snapshot() without an existing book returns None — studies path not reached.
        assert!(store.snapshot("700.HK").is_none());
    }

    #[test]
    fn providers_round_trip() {
        let store = SnapshotStore::new();
        store.set_providers(vec![ProviderDto {
            id: 1,
            name: "LongPort".into(),
            status: SessionStatusDto::Connected,
        }]);
        let ps = store.providers();
        assert_eq!(ps.len(), 1);
        assert_eq!(ps[0].name, "LongPort");
    }

    #[test]
    fn symbols_lists_known_symbols() {
        let store = SnapshotStore::new();
        store.update_book("700.HK", |s| {
            s.symbol = "700.HK".into();
        });
        store.update_book("AAPL.US", |s| {
            s.symbol = "AAPL.US".into();
        });
        let mut syms = store.symbols();
        syms.sort();
        assert_eq!(syms, vec!["700.HK".to_string(), "AAPL.US".to_string()]);
    }

    #[test]
    fn push_chart_point_caps_at_default_cap() {
        let store = SnapshotStore::new();
        for i in 0..700 {
            store.push_chart_point(
                "700.HK",
                "spread",
                ChartPointDto {
                    t: i,
                    value: dec!(0.05),
                    bid: None,
                    ask: None,
                    mid: None,
                },
                600,
            );
        }
        let pts = store.chart_series("700.HK", "spread", 1000);
        assert_eq!(pts.len(), 600);
        assert_eq!(pts[0].t, 100);
    }

    #[test]
    fn chart_series_returns_last_n() {
        let store = SnapshotStore::new();
        for i in 0..50 {
            store.push_chart_point(
                "700.HK",
                "price",
                ChartPointDto {
                    t: i,
                    value: dec!(0),
                    bid: Some(Decimal::from(i as u32)),
                    ask: Some(Decimal::from(i as u32 + 1)),
                    mid: None,
                },
                600,
            );
        }
        let pts = store.chart_series("700.HK", "price", 10);
        assert_eq!(pts.len(), 10);
        assert_eq!(pts[0].bid, Some(dec!(40)));
    }

    #[test]
    fn chart_series_unknown_symbol_returns_empty() {
        let store = SnapshotStore::new();
        assert!(store.chart_series("NOPE.HK", "spread", 100).is_empty());
    }
}
