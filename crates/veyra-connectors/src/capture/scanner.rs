//! Reads usage from a relayed response body as it passes through, without keeping it.
//!
//! Event streams (`text/event-stream`) are read one `data:` line at a time; each line is
//! parsed, its usage fields are kept, and the line is dropped. Other bodies are held
//! only until they end, up to [`MAX_BODY`], then parsed once and dropped.

use serde_json::Value;
use veyra_domain::capture::{Exchange, UsageScan};
use veyra_domain::source::RawUsage;

use super::Route;
use super::wire::{Seen, raw_usage};

/// Largest non-streamed body read for usage. Larger bodies are still relayed.
const MAX_BODY: usize = 16 * 1024 * 1024;
/// Longest event-stream line read for usage. Longer lines are skipped.
const MAX_LINE: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Body,
    Events,
}

/// Collects the usage of one response from its body chunks.
#[derive(Debug)]
pub struct UsageScanner {
    route: Route,
    mode: Mode,
    pending: Vec<u8>,
    /// The body or the current line outgrew its limit and is being skipped.
    skipping: bool,
    seen: Seen,
}

impl UsageScanner {
    /// A scanner for a response of `content_type`.
    #[must_use]
    pub fn new(route: Route, content_type: Option<&str>) -> Self {
        let events = content_type.is_some_and(|t| {
            t.split(';')
                .next()
                .is_some_and(|t| t.trim().eq_ignore_ascii_case("text/event-stream"))
        });
        Self {
            route,
            mode: if events { Mode::Events } else { Mode::Body },
            pending: Vec::new(),
            skipping: false,
            seen: Seen::default(),
        }
    }

    fn event_line(&mut self, line: &[u8]) {
        let Some(data) = line.strip_prefix(b"data:") else {
            return;
        };
        let data = data.trim_ascii();
        if data == b"[DONE]" {
            return;
        }
        if let Ok(value) = serde_json::from_slice::<Value>(data) {
            self.seen.absorb(self.route.wire, &value);
        }
    }
}

impl UsageScan for UsageScanner {
    fn feed(&mut self, mut chunk: &[u8]) {
        match self.mode {
            Mode::Body => {
                if self.skipping || self.pending.len() + chunk.len() > MAX_BODY {
                    self.skipping = true;
                    self.pending = Vec::new();
                } else {
                    self.pending.extend_from_slice(chunk);
                }
            }
            Mode::Events => {
                while let Some(end) = chunk.iter().position(|&b| b == b'\n') {
                    if !self.skipping {
                        self.pending.extend_from_slice(&chunk[..end]);
                        let line = std::mem::take(&mut self.pending);
                        self.event_line(&line);
                    }
                    self.skipping = false;
                    self.pending.clear();
                    chunk = &chunk[end + 1..];
                }
                if self.skipping || self.pending.len() + chunk.len() > MAX_LINE {
                    self.skipping = true;
                    self.pending = Vec::new();
                } else {
                    self.pending.extend_from_slice(chunk);
                }
            }
        }
    }

    /// `None` when the response reported no usage and named no response.
    fn finish(mut self: Box<Self>, exchange: &Exchange<'_>) -> Option<RawUsage> {
        let wire = self.route.wire;
        match self.mode {
            Mode::Body if !self.skipping => {
                match serde_json::from_slice::<Value>(&self.pending) {
                    // Gemini streams without `alt=sse` as one JSON array of responses.
                    Ok(Value::Array(items)) => {
                        for item in &items {
                            self.seen.absorb(wire, item);
                        }
                    }
                    Ok(value) => self.seen.absorb(wire, &value),
                    Err(_) => {}
                }
            }
            Mode::Events if !self.skipping && !self.pending.is_empty() => {
                let line = std::mem::take(&mut self.pending);
                self.event_line(&line);
            }
            Mode::Body | Mode::Events => {}
        }
        raw_usage(&self.route, &self.seen, exchange)
    }
}
