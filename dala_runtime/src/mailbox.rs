//! Mailbox implementation for inter-process message passing.
//!
//! Each BEAM process has a mailbox - a lock-free queue of incoming messages.
//! Messages are sent via `Process::send` and received via `receive` blocks.

use crossbeam::queue::SegQueue;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::term::Term;

/// A process mailbox - a lock-free multi-producer single-consumer queue.
pub struct Mailbox {
    queue: SegQueue<Term>,
    len: AtomicUsize,
}

impl Mailbox {
    /// Create a new empty mailbox.
    pub fn new() -> Self {
        Self {
            queue: SegQueue::new(),
            len: AtomicUsize::new(0),
        }
    }

    /// Enqueue a message into the mailbox.
    pub fn enqueue(&self, msg: Term) {
        self.queue.push(msg);
        self.len.fetch_add(1, Ordering::SeqCst);
    }

    /// Try to dequeue a message from the mailbox.
    pub fn dequeue(&self) -> Option<Term> {
        let msg = self.queue.pop();
        if msg.is_some() {
            self.len.fetch_sub(1, Ordering::SeqCst);
        }
        msg
    }

    /// Check if the mailbox is empty.
    pub fn is_empty(&self) -> bool {
        self.len.load(Ordering::SeqCst) == 0
    }

    /// Get the number of messages in the mailbox.
    pub fn len(&self) -> usize {
        self.len.load(Ordering::SeqCst)
    }
}

impl Default for Mailbox {
    fn default() -> Self {
        Self::new()
    }
}
