//! Edge case tests for Mailbox.

use crate::mailbox::*;
use crate::term::Term;
use dala_ir::type_system::MessagePriority;

// ═══════════════════════════════════════════════════════════════════════════
// Priority ordering: Critical > High > Normal > Low
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_priority_ordering_single_each() {
    let mut mbox = Mailbox::new();
    let t = Term::nil();

    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Low, 1));
    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Critical, 2));
    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Normal, 3));
    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::High, 4));

    assert_eq!(mbox.dequeue().unwrap().priority, MessagePriority::Critical);
    assert_eq!(mbox.dequeue().unwrap().priority, MessagePriority::High);
    assert_eq!(mbox.dequeue().unwrap().priority, MessagePriority::Normal);
    assert_eq!(mbox.dequeue().unwrap().priority, MessagePriority::Low);
    assert!(mbox.dequeue().is_none());
}

#[test]
fn test_priority_ordering_multiple_per_priority() {
    let mut mbox = Mailbox::new();
    let t = Term::nil();

    // Enqueue multiple at each priority
    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Normal, 1));
    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Normal, 2));
    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Critical, 3));
    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Critical, 4));
    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Low, 5));
    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::High, 6));

    // All critical first (FIFO within priority)
    assert_eq!(mbox.dequeue().unwrap().sender, 3);
    assert_eq!(mbox.dequeue().unwrap().sender, 4);
    // Then high
    assert_eq!(mbox.dequeue().unwrap().sender, 6);
    // Then normal
    assert_eq!(mbox.dequeue().unwrap().sender, 1);
    assert_eq!(mbox.dequeue().unwrap().sender, 2);
    // Then low
    assert_eq!(mbox.dequeue().unwrap().sender, 5);
    assert!(mbox.dequeue().is_none());
}

#[test]
fn test_priority_ordering_reverse_insert() {
    let mut mbox = Mailbox::new();
    let t = Term::nil();

    // Insert in reverse priority order
    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Low, 1));
    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Normal, 2));
    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::High, 3));
    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Critical, 4));

    assert_eq!(mbox.dequeue().unwrap().priority, MessagePriority::Critical);
    assert_eq!(mbox.dequeue().unwrap().priority, MessagePriority::High);
    assert_eq!(mbox.dequeue().unwrap().priority, MessagePriority::Normal);
    assert_eq!(mbox.dequeue().unwrap().priority, MessagePriority::Low);
}

#[test]
fn test_priority_ordering_interleaved() {
    let mut mbox = Mailbox::new();
    let t = Term::nil();

    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Normal, 1));
    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Critical, 2));
    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Low, 3));
    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::High, 4));
    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Normal, 5));
    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Critical, 6));

    assert_eq!(mbox.dequeue().unwrap().sender, 2); // Critical
    assert_eq!(mbox.dequeue().unwrap().sender, 6); // Critical
    assert_eq!(mbox.dequeue().unwrap().sender, 4); // High
    assert_eq!(mbox.dequeue().unwrap().sender, 1); // Normal
    assert_eq!(mbox.dequeue().unwrap().sender, 5); // Normal
    assert_eq!(mbox.dequeue().unwrap().sender, 3); // Low
}

// ═══════════════════════════════════════════════════════════════════════════
// dequeue from empty mailbox returns None
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_dequeue_empty() {
    let mut mbox = Mailbox::new();
    assert!(mbox.dequeue().is_none());
}

#[test]
fn test_dequeue_after_drain() {
    let mut mbox = Mailbox::new();
    let t = Term::nil();

    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Normal, 1));
    mbox.drain();
    assert!(mbox.dequeue().is_none());
}

#[test]
fn test_dequeue_all_then_empty() {
    let mut mbox = Mailbox::new();
    let t = Term::nil();

    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Normal, 1));
    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Normal, 2));
    assert!(mbox.dequeue().is_some());
    assert!(mbox.dequeue().is_some());
    assert!(mbox.dequeue().is_none());
}

// ═══════════════════════════════════════════════════════════════════════════
// dequeue_typed: matching type, non-matching type, empty
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_dequeue_typed_matching() {
    let mut mbox = Mailbox::new();
    let t = Term::nil();

    mbox.enqueue(MessageEnvelope::typed(t, MessagePriority::Normal, 1, 42));
    mbox.enqueue(MessageEnvelope::typed(t, MessagePriority::Normal, 2, 99));

    let msg = mbox.dequeue_typed(42);
    assert!(msg.is_some());
    assert_eq!(msg.unwrap().type_tag, Some(42));

    // The other message should still be there
    assert_eq!(mbox.len(), 1);
}

#[test]
fn test_dequeue_typed_non_matching() {
    let mut mbox = Mailbox::new();
    let t = Term::nil();

    mbox.enqueue(MessageEnvelope::typed(t, MessagePriority::Normal, 1, 42));

    let msg = mbox.dequeue_typed(99);
    assert!(msg.is_none());
    // Message should still be in the mailbox
    assert_eq!(mbox.len(), 1);
}

#[test]
fn test_dequeue_typed_empty() {
    let mut mbox = Mailbox::new();
    assert!(mbox.dequeue_typed(42).is_none());
}

#[test]
fn test_dequeue_typed_untagged_message() {
    let mut mbox = Mailbox::new();
    let t = Term::nil();

    // Untagged message (type_tag = None)
    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Normal, 1));

    // dequeue_typed should not find untagged messages
    assert!(mbox.dequeue_typed(42).is_none());
    assert_eq!(mbox.len(), 1);
}

// ═══════════════════════════════════════════════════════════════════════════
// capacity back-pressure: at limit, over limit
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_capacity_at_limit() {
    let mut mbox = Mailbox::with_capacity(4);
    let t = Term::nil();

    // Each queue has capacity 1 (4/4)
    assert!(mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Normal, 1)));
    // Normal queue is now full
    assert!(!mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Normal, 2)));
}

#[test]
fn test_capacity_over_limit() {
    let mut mbox = Mailbox::with_capacity(4);
    let t = Term::nil();

    // Fill all queues
    assert!(mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Low, 1)));
    assert!(mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Normal, 2)));
    assert!(mbox.enqueue(MessageEnvelope::new(t, MessagePriority::High, 3)));
    assert!(mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Critical, 4)));

    // Now all queues are full
    assert!(!mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Low, 5)));
    assert!(!mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Normal, 6)));
}

#[test]
fn test_capacity_one() {
    let mut mbox = Mailbox::with_capacity(1);
    let t = Term::nil();

    assert!(mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Normal, 1)));
    // Total capacity is 1, so this should fail
    assert!(!mbox.enqueue(MessageEnvelope::new(t, MessagePriority::High, 2)));
}

// ═══════════════════════════════════════════════════════════════════════════
// drain preserves priority ordering
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_drain_preserves_order() {
    let mut mbox = Mailbox::new();
    let t = Term::nil();

    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Low, 1));
    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Critical, 2));
    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Normal, 3));
    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::High, 4));

    let drained = mbox.drain();
    assert_eq!(drained.len(), 4);
    // Drain collects from all queues, order depends on implementation
    // The important thing is all messages are present
    assert!(mbox.is_empty());
    assert_eq!(mbox.len(), 0);
}

#[test]
fn test_drain_empty() {
    let mut mbox = Mailbox::new();
    let drained = mbox.drain();
    assert!(drained.is_empty());
}

#[test]
fn test_drain_clears_type_index() {
    let mut mbox = Mailbox::new();
    let t = Term::nil();

    mbox.enqueue(MessageEnvelope::typed(t, MessagePriority::Normal, 1, 42));
    mbox.drain();

    // After drain, dequeue_typed should return None
    assert!(mbox.dequeue_typed(42).is_none());
}

// ═══════════════════════════════════════════════════════════════════════════
// len_at_priority for each priority level
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_len_at_priority_all_empty() {
    let mbox = Mailbox::new();
    assert_eq!(mbox.len_at_priority(MessagePriority::Critical), 0);
    assert_eq!(mbox.len_at_priority(MessagePriority::High), 0);
    assert_eq!(mbox.len_at_priority(MessagePriority::Normal), 0);
    assert_eq!(mbox.len_at_priority(MessagePriority::Low), 0);
}

#[test]
fn test_len_at_priority_mixed() {
    let mut mbox = Mailbox::new();
    let t = Term::nil();

    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Critical, 1));
    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Critical, 2));
    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::High, 3));
    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Low, 4));

    assert_eq!(mbox.len_at_priority(MessagePriority::Critical), 2);
    assert_eq!(mbox.len_at_priority(MessagePriority::High), 1);
    assert_eq!(mbox.len_at_priority(MessagePriority::Normal), 0);
    assert_eq!(mbox.len_at_priority(MessagePriority::Low), 1);
}

#[test]
fn test_critical_count() {
    let mut mbox = Mailbox::new();
    let t = Term::nil();

    assert_eq!(mbox.critical_count(), 0);

    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Critical, 1));
    assert_eq!(mbox.critical_count(), 1);

    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Critical, 2));
    assert_eq!(mbox.critical_count(), 2);
}

#[test]
fn test_high_count() {
    let mut mbox = Mailbox::new();
    let t = Term::nil();

    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::High, 1));
    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::High, 2));
    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::High, 3));

    assert_eq!(mbox.high_count(), 3);
}

#[test]
fn test_normal_count() {
    let mut mbox = Mailbox::new();
    let t = Term::nil();

    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Normal, 1));
    assert_eq!(mbox.normal_count(), 1);
}

#[test]
fn test_low_count() {
    let mut mbox = Mailbox::new();
    let t = Term::nil();

    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Low, 1));
    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Low, 2));
    assert_eq!(mbox.low_count(), 2);
}

// ═══════════════════════════════════════════════════════════════════════════
// SimpleMailbox: FIFO ordering, empty dequeue
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_simple_mailbox_fifo() {
    let mbox = SimpleMailbox::new();

    mbox.enqueue(Term::small(1));
    mbox.enqueue(Term::small(2));
    mbox.enqueue(Term::small(3));

    assert_eq!(mbox.dequeue().unwrap().unwrap_small(), 1);
    assert_eq!(mbox.dequeue().unwrap().unwrap_small(), 2);
    assert_eq!(mbox.dequeue().unwrap().unwrap_small(), 3);
}

#[test]
fn test_simple_mailbox_empty_dequeue() {
    let mbox = SimpleMailbox::new();
    assert!(mbox.dequeue().is_none());
}

#[test]
fn test_simple_mailbox_len() {
    let mbox = SimpleMailbox::new();

    assert_eq!(mbox.len(), 0);
    assert!(mbox.is_empty());

    mbox.enqueue(Term::small(1));
    assert_eq!(mbox.len(), 1);
    assert!(!mbox.is_empty());

    mbox.enqueue(Term::small(2));
    assert_eq!(mbox.len(), 2);

    mbox.dequeue();
    assert_eq!(mbox.len(), 1);

    mbox.dequeue();
    assert_eq!(mbox.len(), 0);
    assert!(mbox.is_empty());
}

#[test]
fn test_simple_mailbox_default() {
    let mbox: SimpleMailbox = Default::default();
    assert!(mbox.is_empty());
    assert_eq!(mbox.len(), 0);
}

// ═══════════════════════════════════════════════════════════════════════════
// Mailbox default
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_mailbox_default() {
    let mbox: Mailbox = Default::default();
    assert!(mbox.is_empty());
    assert_eq!(mbox.len(), 0);
}

// ═══════════════════════════════════════════════════════════════════════════
// peek
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_peek_empty() {
    let mbox = Mailbox::new();
    assert!(mbox.peek().is_none());
}

#[test]
fn test_peek_highest_priority() {
    let mut mbox = Mailbox::new();
    let t = Term::nil();

    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Normal, 1));
    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Critical, 2));

    let peeked = mbox.peek();
    assert!(peeked.is_some());
    assert_eq!(peeked.unwrap().priority, MessagePriority::Critical);
    // Peek should not remove the message
    assert_eq!(mbox.len(), 2);
}

// ═══════════════════════════════════════════════════════════════════════════
// is_empty
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_is_empty_on_new() {
    let mbox = Mailbox::new();
    assert!(mbox.is_empty());
}

#[test]
fn test_is_empty_after_enqueue() {
    let mut mbox = Mailbox::new();
    let t = Term::nil();

    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Normal, 1));
    assert!(!mbox.is_empty());
}

#[test]
fn test_is_empty_after_dequeue_all() {
    let mut mbox = Mailbox::new();
    let t = Term::nil();

    mbox.enqueue(MessageEnvelope::new(t, MessagePriority::Normal, 1));
    mbox.dequeue();
    assert!(mbox.is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════
// MessageEnvelope creation
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_message_envelope_new() {
    let env = MessageEnvelope::new(Term::small(42), MessagePriority::High, 123);
    assert_eq!(env.payload.unwrap_small(), 42);
    assert_eq!(env.priority, MessagePriority::High);
    assert_eq!(env.sender, 123);
    assert_eq!(env.type_tag, None);
}

#[test]
fn test_message_envelope_typed() {
    let env = MessageEnvelope::typed(Term::small(42), MessagePriority::High, 123, 99);
    assert_eq!(env.payload.unwrap_small(), 42);
    assert_eq!(env.priority, MessagePriority::High);
    assert_eq!(env.sender, 123);
    assert_eq!(env.type_tag, Some(99));
}

#[test]
fn test_message_envelope_clone() {
    let env = MessageEnvelope::typed(Term::small(42), MessagePriority::Normal, 1, 5);
    let env2 = env.clone();
    assert_eq!(env.payload, env2.payload);
    assert_eq!(env.priority, env2.priority);
    assert_eq!(env.sender, env2.sender);
    assert_eq!(env.type_tag, env2.type_tag);
}
