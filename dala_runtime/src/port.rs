//! Port implementation for external OS resource interaction.
//!
//! Ports provide the interface between Erlang/Elixir processes and
//! external OS resources (file descriptors, sockets, etc.).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use crate::term::Term;

/// A port identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PortId(u64);

/// A port connected to an external resource.
pub struct Port {
    /// Unique port identifier
    pub id: PortId,
    /// The connected process (owner)
    pub owner: u64,
    /// Port name (atom index)
    pub name: u64,
    /// Message queue for port events
    pub message_queue: Mutex<VecDeque<Term>>,
    /// Whether the port is open
    pub open: bool,
    /// Whether the port is in passive (manual) mode
    pub passive: bool,
}

impl Port {
    /// Create a new port.
    pub fn new(id: PortId, owner: u64, name: u64) -> Self {
        Self {
            id,
            owner,
            name,
            message_queue: Mutex::new(VecDeque::new()),
            open: true,
            passive: false,
        }
    }

    /// Push a message to the port's message queue.
    pub fn push_message(&self, msg: Term) {
        let mut queue = self.message_queue.lock().unwrap();
        queue.push_back(msg);
    }

    /// Try to read a message from the port.
    pub fn try_read(&self) -> Option<Term> {
        let mut queue = self.message_queue.lock().unwrap();
        queue.pop_front()
    }

    /// Close the port.
    pub fn close(&mut self) {
        self.open = false;
    }
}

/// Global port registry.
pub struct PortRegistry {
    ports: Mutex<indexmap::IndexMap<PortId, Arc<Mutex<Port>>>>,
    next_id: Mutex<u64>,
}

impl PortRegistry {
    /// Create a new port registry.
    pub fn new() -> Self {
        Self {
            ports: Mutex::new(indexmap::IndexMap::new()),
            next_id: Mutex::new(1),
        }
    }

    /// Register a new port.
    pub fn register(&self, port: Port) -> PortId {
        let mut ports = self.ports.lock().unwrap();
        let mut next_id = self.next_id.lock().unwrap();
        let id = PortId(*next_id);
        *next_id += 1;
        ports.insert(id, Arc::new(Mutex::new(port)));
        id
    }

    /// Get a port by ID.
    pub fn get(&self, id: &PortId) -> Option<Arc<Mutex<Port>>> {
        let ports = self.ports.lock().unwrap();
        ports.get(id).cloned()
    }

    /// Remove a port.
    pub fn remove(&self, id: &PortId) -> Option<Arc<Mutex<Port>>> {
        let mut ports = self.ports.lock().unwrap();
        ports.swap_remove(id)
    }
}

impl Default for PortRegistry {
    fn default() -> Self {
        Self::new()
    }
}
