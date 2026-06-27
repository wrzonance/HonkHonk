use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeLevel {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NoticeId(u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub level: NoticeLevel,
    pub title: String,
    pub body: String,
}

impl Notice {
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(6);

    pub fn info(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self::new(NoticeLevel::Info, title, body)
    }

    pub fn warning(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self::new(NoticeLevel::Warning, title, body)
    }

    pub fn error(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self::new(NoticeLevel::Error, title, body)
    }

    fn new(level: NoticeLevel, title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            level,
            title: title.into(),
            body: body.into(),
        }
    }

    fn expires_at(&self, now: Instant) -> Option<Instant> {
        match self.level {
            NoticeLevel::Info | NoticeLevel::Warning => Some(now + Self::DEFAULT_TIMEOUT),
            NoticeLevel::Error => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedNotice {
    pub id: NoticeId,
    pub notice: Notice,
    expires_at: Option<Instant>,
}

#[derive(Debug, Default)]
pub struct NoticeQueue {
    next_id: u64,
    entries: VecDeque<QueuedNotice>,
}

impl NoticeQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, notice: Notice, now: Instant) -> NoticeId {
        let id = NoticeId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        let expires_at = notice.expires_at(now);
        self.entries.push_back(QueuedNotice {
            id,
            notice,
            expires_at,
        });
        id
    }

    pub fn dismiss(&mut self, id: NoticeId) -> Option<QueuedNotice> {
        let idx = self.entries.iter().position(|queued| queued.id == id)?;
        self.entries.remove(idx)
    }

    pub fn expire(&mut self, now: Instant) -> usize {
        let before = self.entries.len();
        self.entries
            .retain(|queued| queued.expires_at.is_none_or(|expires| expires > now));
        before - self.entries.len()
    }

    pub fn contains(&self, id: NoticeId) -> bool {
        self.entries.iter().any(|queued| queued.id == id)
    }

    pub fn has_expiring(&self) -> bool {
        self.entries
            .iter()
            .any(|queued| queued.expires_at.is_some())
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn front(&self) -> Option<&QueuedNotice> {
        self.entries.front()
    }

    pub fn iter(&self) -> impl Iterator<Item = &QueuedNotice> {
        self.entries.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn info_and_warning_notices_expire_after_timeout() {
        let start = Instant::now();
        let mut queue = NoticeQueue::new();

        let info_id = queue.push(Notice::info("Library scanned", "12 sounds found"), start);
        let warning_id = queue.push(
            Notice::warning("Shortcut unavailable", "The portal is not running"),
            start,
        );

        assert!(queue.contains(info_id));
        assert!(queue.contains(warning_id));
        assert_eq!(
            queue.expire(start + Notice::DEFAULT_TIMEOUT - Duration::from_millis(1)),
            0
        );
        assert_eq!(queue.expire(start + Notice::DEFAULT_TIMEOUT), 2);
        assert!(queue.is_empty());
    }

    #[test]
    fn error_notices_persist_until_dismissed() {
        let start = Instant::now();
        let mut queue = NoticeQueue::new();

        let id = queue.push(Notice::error("Audio error", "Decode failed"), start);

        assert_eq!(queue.expire(start + Notice::DEFAULT_TIMEOUT * 4), 0);
        assert!(queue.contains(id));
        assert!(queue.dismiss(id).is_some());
        assert!(queue.is_empty());
    }

    #[test]
    fn queued_notices_keep_insertion_order_and_ids() {
        let start = Instant::now();
        let mut queue = NoticeQueue::new();

        let first = queue.push(Notice::info("First", "Body"), start);
        let second = queue.push(Notice::error("Second", "Body"), start);

        let ids = queue.iter().map(|notice| notice.id).collect::<Vec<_>>();
        assert_eq!(ids, vec![first, second]);
    }
}
