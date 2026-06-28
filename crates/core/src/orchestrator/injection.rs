//! Mid-Loop Injection — allows humans to inject instructions while agents are running.
//!
//! Enables changing agent behavior, correcting mistakes, and adding context
//! without stopping the loop. Messages have priority and are processed before
//! the next LLM call.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

// ─── Injection Message Types ──────────────────────────────────

/// Type of injection.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InjectionType {
    /// Instruction to change behavior.
    Instruction,
    /// Additional context to provide.
    Context,
    /// Correction of a mistake.
    Correction,
    /// Halt the current task.
    Halt,
}

/// Priority of the injection (higher = processed first).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum InjectionPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,  // Never compressed, always processed first
}

/// A single injection message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Injection {
    pub id: String,
    pub session_id: String,
    pub target_agent: String, // "all" or specific agent name
    pub injection_type: InjectionType,
    pub priority: InjectionPriority,
    pub message: String,
    pub timestamp: String,
    pub source: InjectionSource,
    pub processed: bool,
}

/// Where the injection came from.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InjectionSource {
    CLI,
    Dashboard,
    API,
    System,
}

impl Injection {
    /// Create a new injection.
    pub fn new(
        session_id: &str,
        target_agent: &str,
        injection_type: InjectionType,
        priority: InjectionPriority,
        message: &str,
        source: InjectionSource,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            target_agent: target_agent.to_string(),
            injection_type,
            priority,
            message: message.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            source,
            processed: false,
        }
    }

    /// Check if this injection targets a specific agent or all agents.
    pub fn targets(&self, agent_id: &str) -> bool {
        self.target_agent == "all" || self.target_agent == agent_id
    }

    /// Check if this injection should be compressed (only Critical is never compressed).
    pub fn should_compress(&self) -> bool {
        self.priority < InjectionPriority::Critical
    }
}

// ─── Injection Channel ────────────────────────────────────────

/// Channel for mid-loop injection with priority queue.
pub struct InjectionChannel {
    /// Pending injections, sorted by priority.
    pending: VecDeque<Injection>,
    /// History of processed injections.
    history: Vec<Injection>,
    /// Maximum pending before dropping low-priority.
    max_pending: usize,
}

impl InjectionChannel {
    /// Create a new injection channel.
    pub fn new(max_pending: usize) -> Self {
        Self {
            pending: VecDeque::new(),
            history: Vec::new(),
            max_pending,
        }
    }

    /// Create with default max (100).
    pub fn default_channel() -> Self {
        Self::new(100)
    }

    /// Submit an injection.
    pub fn submit(&mut self, injection: Injection) -> Result<(), InjectionError> {
        if self.pending.len() >= self.max_pending {
            // If at capacity, only accept Critical priority
            if injection.priority < InjectionPriority::Critical {
                return Err(InjectionError::QueueFull {
                    pending: self.pending.len(),
                    max: self.max_pending,
                });
            }
        }

        self.pending.push_back(injection);
        Ok(())
    }

    /// Get the next injection for a specific agent (sorted by priority).
    pub fn next_for_agent(&mut self, agent_id: &str) -> Option<Injection> {
        // Find the highest-priority injection that targets this agent
        let mut best_idx = None;
        let mut best_priority = InjectionPriority::Low;

        for (i, inj) in self.pending.iter().enumerate() {
            if inj.targets(agent_id) && inj.priority >= best_priority {
                best_idx = Some(i);
                best_priority = inj.priority;
            }
        }

        if let Some(idx) = best_idx {
            let mut injection = self.pending.remove(idx).unwrap();
            injection.processed = true;
            self.history.push(injection.clone());
            Some(injection)
        } else {
            None
        }
    }

    /// Get all pending injections for a specific agent.
    pub fn drain_for_agent(&mut self, agent_id: &str) -> Vec<Injection> {
        let mut result = Vec::new();
        let mut remaining = VecDeque::new();

        while let Some(inj) = self.pending.pop_front() {
            if inj.targets(agent_id) && !inj.processed {
                let mut processed = inj.clone();
                processed.processed = true;
                self.history.push(processed.clone());
                result.push(processed);
            } else {
                remaining.push_back(inj);
            }
        }

        self.pending = remaining;
        result
    }

    /// Get all pending injections (any agent).
    pub fn pending_count(&self) -> usize {
        self.pending.iter().filter(|i| !i.processed).count()
    }

    /// Get injection history.
    pub fn history(&self) -> &[Injection] {
        &self.history
    }

    /// Get history for a specific session.
    pub fn history_for_session(&self, session_id: &str) -> Vec<&Injection> {
        self.history.iter().filter(|i| i.session_id == session_id).collect()
    }

    /// Clear all pending injections.
    pub fn clear_pending(&mut self) {
        self.pending.clear();
    }

    /// Get statistics.
    pub fn stats(&self) -> InjectionStats {
        let total = self.history.len();
        let by_type = self.history.iter().fold(
            std::collections::HashMap::new(),
            |mut acc, inj| {
                *acc.entry(inj.injection_type.clone()).or_insert(0) += 1;
                acc
            },
        );
        let by_priority = self.history.iter().fold(
            std::collections::HashMap::new(),
            |mut acc, inj| {
                *acc.entry(inj.priority).or_insert(0) += 1;
                acc
            },
        );

        InjectionStats {
            total,
            pending: self.pending_count(),
            by_type,
            by_priority,
        }
    }
}

impl Default for InjectionChannel {
    fn default() -> Self {
        Self::default_channel()
    }
}

// ─── Errors ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum InjectionError {
    QueueFull { pending: usize, max: usize },
    InvalidTarget(String),
    SessionNotFound(String),
}

impl std::fmt::Display for InjectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::QueueFull { pending, max } => {
                write!(f, "Injection queue full: {}/{} (only Critical accepted)", pending, max)
            }
            Self::InvalidTarget(target) => write!(f, "Invalid target: '{}'", target),
            Self::SessionNotFound(id) => write!(f, "Session not found: '{}'", id),
        }
    }
}

// ─── Statistics ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InjectionStats {
    pub total: usize,
    pub pending: usize,
    pub by_type: std::collections::HashMap<InjectionType, u32>,
    pub by_priority: std::collections::HashMap<InjectionPriority, u32>,
}

// ─── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_injection_creation() {
        let inj = Injection::new(
            "s1",
            "coder",
            InjectionType::Instruction,
            InjectionPriority::Normal,
            "Use thiserror instead of anyhow",
            InjectionSource::CLI,
        );
        assert_eq!(inj.session_id, "s1");
        assert_eq!(inj.target_agent, "coder");
        assert_eq!(inj.injection_type, InjectionType::Instruction);
        assert!(!inj.processed);
    }

    #[test]
    fn test_injection_targets() {
        let inj = Injection::new(
            "s1", "coder", InjectionType::Instruction,
            InjectionPriority::Normal, "test", InjectionSource::CLI,
        );
        assert!(inj.targets("coder"));
        assert!(!inj.targets("reviewer"));

        let all = Injection::new(
            "s1", "all", InjectionType::Instruction,
            InjectionPriority::Normal, "test", InjectionSource::CLI,
        );
        assert!(all.targets("coder"));
        assert!(all.targets("anyone"));
    }

    #[test]
    fn test_injection_channel_submit() {
        let mut channel = InjectionChannel::new(5);

        let inj = Injection::new(
            "s1", "coder", InjectionType::Instruction,
            InjectionPriority::Normal, "test", InjectionSource::CLI,
        );
        assert!(channel.submit(inj).is_ok());
        assert_eq!(channel.pending_count(), 1);
    }

    #[test]
    fn test_injection_channel_priority() {
        let mut channel = InjectionChannel::new(10);

        channel.submit(Injection::new("s1", "coder", InjectionType::Instruction,
            InjectionPriority::Low, "low priority", InjectionSource::CLI)).unwrap();
        channel.submit(Injection::new("s1", "coder", InjectionType::Instruction,
            InjectionPriority::Critical, "critical", InjectionSource::CLI)).unwrap();
        channel.submit(Injection::new("s1", "coder", InjectionType::Instruction,
            InjectionPriority::Normal, "normal", InjectionSource::CLI)).unwrap();

        // Should get Critical first
        let inj = channel.next_for_agent("coder").unwrap();
        assert_eq!(inj.priority, InjectionPriority::Critical);
        assert_eq!(inj.message, "critical");
    }

    #[test]
    fn test_injection_channel_drain() {
        let mut channel = InjectionChannel::new(10);

        channel.submit(Injection::new("s1", "coder", InjectionType::Instruction,
            InjectionPriority::Normal, "msg1", InjectionSource::CLI)).unwrap();
        channel.submit(Injection::new("s1", "reviewer", InjectionType::Instruction,
            InjectionPriority::Normal, "msg2", InjectionSource::CLI)).unwrap();
        channel.submit(Injection::new("s1", "coder", InjectionType::Instruction,
            InjectionPriority::Normal, "msg3", InjectionSource::CLI)).unwrap();

        let drained = channel.drain_for_agent("coder");
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].message, "msg1");
        assert_eq!(drained[1].message, "msg3");

        // Reviewer still has its message
        let remaining = channel.drain_for_agent("reviewer");
        assert_eq!(remaining.len(), 1);
    }

    #[test]
    fn test_injection_channel_queue_full() {
        let mut channel = InjectionChannel::new(2);

        channel.submit(Injection::new("s1", "coder", InjectionType::Instruction,
            InjectionPriority::Normal, "msg1", InjectionSource::CLI)).unwrap();
        channel.submit(Injection::new("s1", "coder", InjectionType::Instruction,
            InjectionPriority::Normal, "msg2", InjectionSource::CLI)).unwrap();

        // Queue full, Normal priority should fail
        let result = channel.submit(Injection::new("s1", "coder", InjectionType::Instruction,
            InjectionPriority::Normal, "msg3", InjectionSource::CLI));
        assert!(result.is_err());

        // But Critical should succeed
        let result = channel.submit(Injection::new("s1", "coder", InjectionType::Instruction,
            InjectionPriority::Critical, "critical", InjectionSource::CLI));
        assert!(result.is_ok());
    }

    #[test]
    fn test_injection_history() {
        let mut channel = InjectionChannel::new(10);

        channel.submit(Injection::new("s1", "coder", InjectionType::Instruction,
            InjectionPriority::Normal, "msg1", InjectionSource::CLI)).unwrap();
        channel.submit(Injection::new("s1", "coder", InjectionType::Instruction,
            InjectionPriority::Normal, "msg2", InjectionSource::CLI)).unwrap();

        // Process one
        channel.next_for_agent("coder");

        let history = channel.history();
        assert_eq!(history.len(), 1);
        assert!(history[0].processed);

        // Session filter
        let session_history = channel.history_for_session("s1");
        assert_eq!(session_history.len(), 1);
    }

    #[test]
    fn test_injection_stats() {
        let mut channel = InjectionChannel::new(10);

        channel.submit(Injection::new("s1", "coder", InjectionType::Instruction,
            InjectionPriority::Normal, "msg1", InjectionSource::CLI)).unwrap();
        channel.submit(Injection::new("s1", "coder", InjectionType::Correction,
            InjectionPriority::Critical, "msg2", InjectionSource::CLI)).unwrap();

        let stats = channel.stats();
        assert_eq!(stats.total, 0); // None processed yet
        assert_eq!(stats.pending, 2);
    }
}