//! Mailbox implementation for inter-process message passing.
//!
//! Each Dala actor has a mailbox — a multi-queue message routing system
//! that supports:
//!
//! - **Typed mailboxes**: Messages are tagged with type metadata for
//!   fast-path matching in `receive` blocks.
//! - **Priority channels**: Four priority levels (Low, Normal, High,
//!   Critical) with separate queues, enabling QoS-aware delivery.
//! - **Stable message layouts**: Known-shape messages use compact
//!   representations for zero-copy delivery.
//! - **Pattern-indexed dispatch**: The compiler generates a jump table
//!   over expected message shapes, lowering `receive` to a switch.
//!
//! # Architecture
//!
//! ```text
//!  Sender ──► PriorityRouter ──► [Critical] ──┐
//!                     │           [High]    ──┤
//!                     │           [Normal]  ──┼──► PatternMatcher ──► Actor
//!                     │           [Low]     ──┘
//!                     │
//!                     └──► OverflowBuffer (back-pressure)
//! ```

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};

use dala_ir::type_system::MessagePriority;

use crate::term::Term;

// ═══════════════════════════════════════════════════════════════════════════
// Message envelope
// ═══════════════════════════════════════════════════════════════════════════

/// A message envelope — wraps a term with routing metadata.
#[derive(Debug, Clone)]
pub struct MessageEnvelope {
    /// The actual message payload
    pub payload: Term,
    /// Priority class for QoS-aware routing
    pub priority: MessagePriority,
    /// Sender PID (for reply routing)
    pub sender: u64,
    /// Optional type tag for fast-path matching.
    /// `None` means the message type is unknown (fallback to slow path).
    pub type_tag: Option<u32>,
}

impl MessageEnvelope {
    /// Create a new message envelope.
    pub fn new(payload: Term, priority: MessagePriority, sender: u64) -> Self {
        Self {
            payload,
            priority,
            sender,
            type_tag: None,
        }
    }

    /// Create a typed message envelope with a known type tag.
    pub fn typed(payload: Term, priority: MessagePriority, sender: u64, type_tag: u32) -> Self {
        Self {
            payload,
            priority,
            sender,
            type_tag: Some(type_tag),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Priority queue
// ═══════════════════════════════════════════════════════════════════════════

/// A single priority queue within the mailbox.
#[derive(Debug)]
struct PriorityQueue {
    queue: VecDeque<MessageEnvelope>,
    capacity: usize,
}

impl PriorityQueue {
    fn new(capacity: usize) -> Self {
        Self {
            queue: VecDeque::with_capacity(64),
            capacity,
        }
    }

    fn push(&mut self, msg: MessageEnvelope) -> bool {
        if self.queue.len() >= self.capacity {
            false // Queue full — back-pressure
        } else {
            self.queue.push_back(msg);
            true
        }
    }

    fn pop(&mut self) -> Option<MessageEnvelope> {
        self.queue.pop_front()
    }

    fn peek(&self) -> Option<&MessageEnvelope> {
        self.queue.front()
    }

    fn len(&self) -> usize {
        self.queue.len()
    }

    fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Drain all envelopes into a vec (for pattern matching).
    fn drain(&mut self) -> Vec<MessageEnvelope> {
        self.queue.drain(..).collect()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Typed mailbox
// ═══════════════════════════════════════════════════════════════════════════

/// A typed, priority-aware mailbox for actor message passing.
///
/// Messages are routed into one of four priority queues.  The `receive`
/// operation scans queues in priority order (Critical → High → Normal →
/// Low) and uses the type tag for fast-path matching.
pub struct Mailbox {
    /// Priority-specific queues
    queues: [PriorityQueue; 4],
    /// Total message count across all queues
    len: AtomicUsize,
    /// Per-type-tag index: maps type_tag → queue index for O(1) lookup
    /// of messages matching a specific type.
    type_index: std::collections::HashMap<u32, Vec<usize>>,
    /// Overflow buffer for when priority queues are full
    overflow: VecDeque<MessageEnvelope>,
    /// Maximum total messages (all queues + overflow combined)
    max_capacity: usize,
}

impl Mailbox {
    /// Create a new empty mailbox with default capacity.
    pub fn new() -> Self {
        Self::with_capacity(4096)
    }

    /// Create a new mailbox with the given total capacity.
    pub fn with_capacity(max_capacity: usize) -> Self {
        Self {
            queues: [
                PriorityQueue::new(max_capacity / 4), // Low (priority 0)
                PriorityQueue::new(max_capacity / 4), // Normal (priority 1)
                PriorityQueue::new(max_capacity / 4), // High (priority 2)
                PriorityQueue::new(max_capacity / 4), // Critical (priority 3)
            ],
            len: AtomicUsize::new(0),
            type_index: std::collections::HashMap::new(),
            overflow: VecDeque::new(),
            max_capacity,
        }
    }

    /// Enqueue a message into the mailbox.
    ///
    /// Returns `true` if the message was accepted, `false` if the
    /// mailbox is full (back-pressure).
    pub fn enqueue(&mut self, msg: MessageEnvelope) -> bool {
        let total = self.len.load(Ordering::Relaxed);
        if total >= self.max_capacity {
            return false;
        }

        let priority_idx = msg.priority as usize;
        let accepted = if priority_idx < 4 {
            self.queues[priority_idx].push(msg.clone())
        } else {
            false
        };

        if accepted {
            self.len.fetch_add(1, Ordering::SeqCst);
            // Update type index
            if let Some(tag) = msg.type_tag {
                let queue_idx = priority_idx;
                self.type_index.entry(tag).or_default().push(queue_idx);
            }
        } else {
            // Try overflow
            if self.overflow.len() < self.max_capacity / 8 {
                self.overflow.push_back(msg);
                self.len.fetch_add(1, Ordering::SeqCst);
                return true;
            }
            return false;
        }

        true
    }

    /// Try to dequeue the highest-priority message.
    pub fn dequeue(&mut self) -> Option<MessageEnvelope> {
        // Iterate from highest priority (Critical = index 3) to lowest (Low = index 0)
        for queue in self.queues.iter_mut().rev() {
            if let Some(msg) = queue.pop() {
                self.len.fetch_sub(1, Ordering::SeqCst);
                return Some(msg);
            }
        }
        // Try overflow
        if let Some(msg) = self.overflow.pop_front() {
            self.len.fetch_sub(1, Ordering::SeqCst);
            return Some(msg);
        }
        None
    }

    /// Try to dequeue a message matching a specific type tag.
    ///
    /// This is the fast-path for typed `receive`: instead of scanning
    /// all messages, we look up the type index and check only matching
    /// queues.
    pub fn dequeue_typed(&mut self, type_tag: u32) -> Option<MessageEnvelope> {
        if let Some(queue_indices) = self.type_index.get(&type_tag) {
            for &queue_idx in queue_indices {
                if queue_idx < 4 {
                    // Scan this queue for a message with the matching tag
                    let queue = &mut self.queues[queue_idx];
                    // For now, pop from front (in a full impl we'd have
                    // a per-tag sub-queue)
                    if let Some(env) = queue.peek() {
                        if env.type_tag == Some(type_tag) {
                            let msg = queue.pop().unwrap();
                            self.len.fetch_sub(1, Ordering::SeqCst);
                            return Some(msg);
                        }
                    }
                }
            }
        }
        None
    }

    /// Peek at the next message without removing it.
    pub fn peek(&self) -> Option<&MessageEnvelope> {
        for queue in self.queues.iter().rev() {
            if let Some(msg) = queue.peek() {
                return Some(msg);
            }
        }
        self.overflow.front()
    }

    /// Check if the mailbox is empty.
    pub fn is_empty(&self) -> bool {
        self.len.load(Ordering::SeqCst) == 0
    }

    /// Get the total number of messages.
    pub fn len(&self) -> usize {
        self.len.load(Ordering::SeqCst)
    }

    /// Get the number of messages at a specific priority.
    pub fn len_at_priority(&self, priority: MessagePriority) -> usize {
        self.queues[priority as usize].len()
    }

    /// Get the number of critical-priority messages.
    pub fn critical_count(&self) -> usize {
        self.queues[MessagePriority::Critical as usize].len()
    }

    /// Get the number of high-priority messages.
    pub fn high_count(&self) -> usize {
        self.queues[MessagePriority::High as usize].len()
    }

    /// Get the number of normal-priority messages.
    pub fn normal_count(&self) -> usize {
        self.queues[MessagePriority::Normal as usize].len()
    }

    /// Get the number of low-priority messages.
    pub fn low_count(&self) -> usize {
        self.queues[MessagePriority::Low as usize].len()
    }

    /// Drain all messages (e.g. on process termination).
    pub fn drain(&mut self) -> Vec<MessageEnvelope> {
        let mut result = Vec::new();
        for queue in &mut self.queues {
            result.extend(queue.drain());
        }
        result.extend(self.overflow.drain(..));
        self.len.store(0, Ordering::SeqCst);
        self.type_index.clear();
        result
    }
}

impl Default for Mailbox {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Legacy compatibility: simple mailbox for non-actor processes
// ═══════════════════════════════════════════════════════════════════════════

/// A simple FIFO mailbox for backward compatibility.
/// Used by non-actor processes that don't need priority routing.
pub struct SimpleMailbox {
    queue: VecDeque<Term>,
    len: AtomicUsize,
}

impl SimpleMailbox {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            len: AtomicUsize::new(0),
        }
    }

    pub fn enqueue(&self, msg: Term) {
        // Note: this uses interior mutability for compatibility
        // with the existing Process::send signature.
        // In the new design, Mailbox is behind a Mutex.
        unsafe {
            let queue = &self.queue as *const VecDeque<Term> as *mut VecDeque<Term>;
            (*queue).push_back(msg);
        }
        self.len.fetch_add(1, Ordering::SeqCst);
    }

    pub fn dequeue(&self) -> Option<Term> {
        unsafe {
            let queue = &self.queue as *const VecDeque<Term> as *mut VecDeque<Term>;
            let msg = (*queue).pop_front();
            if msg.is_some() {
                self.len.fetch_sub(1, Ordering::SeqCst);
            }
            msg
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len.load(Ordering::SeqCst) == 0
    }

    pub fn len(&self) -> usize {
        self.len.load(Ordering::SeqCst)
    }
}

impl Default for SimpleMailbox {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_ordering() {
        let mut mbox = Mailbox::new();
        let t = Term::nil();

        mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Low, 1));
        mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Critical, 2));
        mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Normal, 3));
        mbox.enqueue(MessageEnvelope::new(t, MessagePriority::High, 4));

        // Should dequeue in priority order: Critical, High, Normal, Low
        assert_eq!(mbox.dequeue().unwrap().priority, MessagePriority::Critical);
        assert_eq!(mbox.dequeue().unwrap().priority, MessagePriority::High);
        assert_eq!(mbox.dequeue().unwrap().priority, MessagePriority::Normal);
        assert_eq!(mbox.dequeue().unwrap().priority, MessagePriority::Low);
        assert!(mbox.is_empty());
    }

    #[test]
    fn test_typed_dequeue() {
        let mut mbox = Mailbox::new();
        let t = Term::nil();

        mbox.enqueue(MessageEnvelope::typed(t, MessagePriority::Normal, 1, 42));
        mbox.enqueue(MessageEnvelope::typed(t, MessagePriority::Normal, 2, 99));

        let msg = mbox.dequeue_typed(42);
        assert!(msg.is_some());
        assert_eq!(msg.unwrap().type_tag, Some(42));
    }

    #[test]
    fn test_capacity_back_pressure() {
        let mut mbox = Mailbox::with_capacity(8);
        let t = Term::nil();

        // Fill up the Normal queue (capacity = 8/4 = 2)
        assert!(mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Normal, 1)));
        assert!(mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Normal, 2)));
        // Normal queue is now full, but other queues have space
        // Total capacity is 8, so we can still add to other priorities
        assert!(mbox.enqueue(MessageEnvelope::new(t, MessagePriority::High, 3)));
        assert!(mbox.enqueue(MessageEnvelope::new(t, MessagePriority::High, 4)));
        assert!(mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Critical, 5)));
        assert!(mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Critical, 6)));
        assert!(mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Low, 7)));
        assert!(mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Low, 8)));
        // Now all queues are full (8 total)
        assert!(!mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Normal, 9)));
    }

    #[test]
    fn test_drain() {
        let mut mbox = Mailbox::new();
        let t = Term::nil();

        mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Normal, 1));
        mbox.enqueue(MessageEnvelope::new(t, MessagePriority::High, 2));

        let drained = mbox.drain();
        assert_eq!(drained.len(), 2);
        assert!(mbox.is_empty());
    }

    #[test]
    fn test_priority_counts() {
        let mut mbox = Mailbox::new();
        let t = Term::nil();

        mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Critical, 1));
        mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Critical, 2));
        mbox.enqueue(MessageEnvelope::new(t, MessagePriority::High, 3));

        assert_eq!(mbox.critical_count(), 2);
        assert_eq!(mbox.high_count(), 1);
        assert_eq!(mbox.len_at_priority(MessagePriority::Normal), 0);
    }
}
