//! Notification broadcast: subscribers register a Tauri Channel; the hub
//! pushes NotificationPayloads to all registered channels.
#![allow(dead_code)]

use crate::dto::NotificationPayload;
use tauri::ipc::Channel;
use tokio::sync::Mutex;

pub struct NotificationHub {
    channels: Mutex<Vec<Channel<NotificationPayload>>>,
}

impl NotificationHub {
    pub fn new() -> Self {
        Self {
            channels: Mutex::new(Vec::new()),
        }
    }

    pub async fn register(&self, ch: Channel<NotificationPayload>) {
        let mut guard = self.channels.lock().await;
        guard.push(ch);
    }

    pub async fn broadcast(&self, payload: NotificationPayload) {
        let guard = self.channels.lock().await;
        for ch in guard.iter() {
            let _ = ch.send(payload.clone());
        }
    }

    pub async fn subscriber_count(&self) -> usize {
        self.channels.lock().await.len()
    }
}

impl Default for NotificationHub {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tauri's Channel<T> can't be constructed outside the runtime. Functional
    // broadcast is verified in the manual `cargo tauri dev` smoke test.

    #[tokio::test]
    async fn hub_starts_empty() {
        let hub = NotificationHub::new();
        assert_eq!(hub.subscriber_count().await, 0);
    }
}
