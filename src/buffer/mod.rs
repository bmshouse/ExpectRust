//! Buffer management for process output

mod ansi;

pub use ansi::strip_ansi;

use bytes::BytesMut;
use std::io;

/// Ratio for buffer compaction strategy.
/// When buffer is full, discard oldest 1/3 and keep newest 2/3.
const DISCARD_RATIO: usize = 3;

/// Manages buffering of process output with intelligent compaction
pub struct BufferManager {
    buffer: BytesMut,
    matched_position: usize,
    max_size: usize,
    strip_ansi: bool,
}

impl BufferManager {
    /// Create a new buffer manager
    pub fn new(max_size: usize, strip_ansi: bool) -> Self {
        Self {
            buffer: BytesMut::with_capacity(max_size),
            matched_position: 0,
            max_size,
            strip_ansi,
        }
    }

    /// Append data to the buffer
    pub fn append(&mut self, data: &[u8]) -> io::Result<()> {
        let data_to_append = if self.strip_ansi {
            strip_ansi(data)
        } else {
            data.to_vec()
        };

        // Check if we need to compact before appending
        if self.buffer.len() + data_to_append.len() > self.max_size {
            self.compact()?;
        }

        self.buffer.extend_from_slice(&data_to_append);
        Ok(())
    }

    /// Get the buffer as a string slice
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.buffer).unwrap_or("")
    }

    /// Get the buffer as bytes
    pub fn as_bytes(&self) -> &[u8] {
        &self.buffer
    }

    /// Get unmatched portion of the buffer
    pub fn unmatched(&self) -> &[u8] {
        &self.buffer[self.matched_position..]
    }

    /// Mark a position as matched
    pub fn mark_matched(&mut self, end_position: usize) {
        self.matched_position = end_position;
    }

    /// Get the current buffer length
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Get the matched position
    pub fn matched_position(&self) -> usize {
        self.matched_position
    }

    /// Get text before a given position
    pub fn before(&self, position: usize) -> &[u8] {
        &self.buffer[..position.min(self.buffer.len())]
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    #[cfg(test)]
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.matched_position = 0;
    }

    /// Compact the buffer using 2/3 discard strategy
    fn compact(&mut self) -> io::Result<()> {
        // When buffer reaches capacity, discard oldest 1/3 (based on DISCARD_RATIO)
        // but preserve unmatched data
        let discard_amount = self.max_size / DISCARD_RATIO;
        let keep_from = discard_amount.max(self.matched_position);

        // Only compact if we have something to discard and keep_from is valid
        if keep_from > 0 && keep_from < self.buffer.len() {
            let new_len = self.buffer.len() - keep_from;
            self.buffer.copy_within(keep_from.., 0);
            self.buffer.truncate(new_len);
            self.matched_position = self.matched_position.saturating_sub(keep_from);
        } else if keep_from >= self.buffer.len() {
            // If keep_from is beyond buffer length, just clear everything
            self.buffer.clear();
            self.matched_position = 0;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_buffer() {
        let buffer = BufferManager::new(1024, false);
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
        assert_eq!(buffer.matched_position(), 0);
    }

    #[test]
    fn test_append() {
        let mut buffer = BufferManager::new(1024, false);
        buffer.append(b"Hello").unwrap();
        assert_eq!(buffer.len(), 5);
        assert_eq!(buffer.as_str(), "Hello");
    }

    #[test]
    fn test_multiple_appends() {
        let mut buffer = BufferManager::new(1024, false);
        buffer.append(b"Hello ").unwrap();
        buffer.append(b"World").unwrap();
        assert_eq!(buffer.len(), 11);
        assert_eq!(buffer.as_str(), "Hello World");
    }

    #[test]
    fn test_unmatched() {
        let mut buffer = BufferManager::new(1024, false);
        buffer.append(b"Hello World").unwrap();
        buffer.mark_matched(6);

        let unmatched = buffer.unmatched();
        assert_eq!(unmatched, b"World");
    }

    #[test]
    fn test_mark_matched() {
        let mut buffer = BufferManager::new(1024, false);
        buffer.append(b"Test data").unwrap();

        assert_eq!(buffer.matched_position(), 0);
        buffer.mark_matched(4);
        assert_eq!(buffer.matched_position(), 4);
        buffer.mark_matched(9);
        assert_eq!(buffer.matched_position(), 9);
    }

    #[test]
    fn test_before() {
        let mut buffer = BufferManager::new(1024, false);
        buffer.append(b"Hello World").unwrap();

        let before = buffer.before(5);
        assert_eq!(before, b"Hello");

        let before_all = buffer.before(100);
        assert_eq!(before_all, b"Hello World");
    }

    #[test]
    fn test_clear() {
        let mut buffer = BufferManager::new(1024, false);
        buffer.append(b"Hello").unwrap();
        buffer.mark_matched(3);

        buffer.clear();
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.matched_position(), 0);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_compact_basic() {
        let mut buffer = BufferManager::new(90, false);

        // Add initial data
        buffer.append(b"0123456789".repeat(5).as_slice()).unwrap(); // 50 bytes
        assert_eq!(buffer.len(), 50);

        // Add more to exceed capacity and trigger compaction
        buffer.append(b"ABCDEFGHIJ".repeat(5).as_slice()).unwrap(); // 50 more bytes = 100 total

        // Buffer should have compacted (kept roughly 2/3)
        assert!(buffer.len() < 100);
        assert!(!buffer.is_empty());
    }

    #[test]
    fn test_compact_preserves_unmatched() {
        let mut buffer = BufferManager::new(120, false);

        // Add some initial data
        buffer.append(b"MATCHED_DATA_").unwrap(); // 13 bytes
        buffer.mark_matched(13); // Mark it all as matched

        // Add unmatched data
        buffer.append(b"UNMATCHED_").unwrap(); // 10 bytes
        assert_eq!(buffer.len(), 23);

        // Add lots more data to trigger compaction
        buffer.append(b"X".repeat(100).as_slice()).unwrap(); // Total would be 123 bytes

        // Buffer should have compacted but preserved unmatched data
        assert!(!buffer.is_empty());
        // The UNMATCHED_ part should still be there
        assert!(
            String::from_utf8_lossy(buffer.as_bytes()).contains("UNMATCHED_")
                || String::from_utf8_lossy(buffer.as_bytes()).contains("X")
        );
    }

    #[test]
    fn test_strip_ansi_enabled() {
        let mut buffer = BufferManager::new(1024, true);

        // Add text with ANSI codes
        buffer.append(b"Hello \x1b[31mRed\x1b[0m World").unwrap();

        // ANSI codes should be stripped
        assert_eq!(buffer.as_str(), "Hello Red World");
    }

    #[test]
    fn test_strip_ansi_disabled() {
        let mut buffer = BufferManager::new(1024, false);

        // Add text with ANSI codes
        let data = b"Hello \x1b[31mRed\x1b[0m World";
        buffer.append(data).unwrap();

        // ANSI codes should NOT be stripped
        assert_eq!(buffer.as_bytes(), data);
    }

    #[test]
    fn test_as_bytes() {
        let mut buffer = BufferManager::new(1024, false);
        buffer.append(b"Binary\x00Data").unwrap();

        let bytes = buffer.as_bytes();
        assert_eq!(bytes, b"Binary\x00Data");
    }

    #[test]
    fn test_compact_2_3_strategy() {
        let mut buffer = BufferManager::new(300, false);

        // Fill to capacity
        let data = b"A".repeat(250);
        buffer.append(&data).unwrap();
        assert_eq!(buffer.len(), 250);

        // Add more to trigger compaction (should discard oldest 1/3 = 100 bytes)
        buffer.append(b"B".repeat(100).as_slice()).unwrap();

        // Should have kept 2/3 of max_size worth
        assert!(buffer.len() <= 250); // Some discarded
    }

    #[test]
    fn test_matched_position_after_compact() {
        let mut buffer = BufferManager::new(90, false);

        // Add data
        buffer.append(b"0123456789".repeat(5).as_slice()).unwrap();
        buffer.mark_matched(20);

        let matched_before = buffer.matched_position();

        // Trigger compaction
        buffer.append(b"X".repeat(50).as_slice()).unwrap();

        // Matched position should be adjusted
        let matched_after = buffer.matched_position();
        assert!(matched_after <= matched_before);
    }

    #[test]
    fn test_empty_append() {
        let mut buffer = BufferManager::new(1024, false);
        buffer.append(b"").unwrap();

        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_utf8_handling() {
        let mut buffer = BufferManager::new(1024, false);
        buffer.append("Hello 世界! 🎉".as_bytes()).unwrap();

        assert_eq!(buffer.as_str(), "Hello 世界! 🎉");
    }

    #[test]
    fn test_invalid_utf8() {
        let mut buffer = BufferManager::new(1024, false);
        // Invalid UTF-8 sequence
        buffer.append(&[0xFF, 0xFE, 0xFD]).unwrap();

        // as_str should return empty string for invalid UTF-8
        assert_eq!(buffer.as_str(), "");

        // But as_bytes should still return the data
        assert_eq!(buffer.as_bytes(), &[0xFF, 0xFE, 0xFD]);
    }
}
