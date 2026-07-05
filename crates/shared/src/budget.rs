//! Budget for agent execution — token, cost, turn, and delegation depth limits.
//!
//! Implements SentinelAgent P1 (authority monotonic narrowing) and P5
//! (bounded cascade containment): when a parent delegates to a child,
//! the child receives `min(parent_remaining, child_inherent)` and
//! `max_depth - 1`.

use serde::{Deserialize, Serialize};

/// Execution budget for an agent invocation.
///
/// Tracks limits (max) and consumption (used) for tokens, cost, turns,
/// and delegation depth. When a parent delegates to a subagent, the
/// child's budget is derived via `for_child()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Budget {
    /// Maximum tokens this agent can consume (None = unlimited).
    pub max_tokens: Option<u64>,
    /// Maximum cost in USD (None = unlimited).
    pub max_cost_usd: Option<f64>,
    /// Maximum LLM turns this agent can take.
    pub max_turns: u32,
    /// Maximum delegation depth (0 = leaf agent, cannot delegate).
    pub max_depth: u8,
    /// Tokens consumed so far.
    pub used_tokens: u64,
    /// Cost consumed so far (USD).
    pub used_cost: f64,
    /// Turns consumed so far.
    pub used_turns: u32,
}

impl Budget {
    /// Create a budget with no limits (for root orchestrator).
    pub fn unlimited() -> Self {
        Self {
            max_tokens: None,
            max_cost_usd: None,
            max_turns: u32::MAX,
            max_depth: 3,
            used_tokens: 0,
            used_cost: 0.0,
            used_turns: 0,
        }
    }

    /// Create a leaf budget (cannot delegate).
    pub fn leaf(max_tokens: Option<u64>, max_cost_usd: Option<f64>, max_turns: u32) -> Self {
        Self {
            max_tokens,
            max_cost_usd,
            max_turns,
            max_depth: 0,
            used_tokens: 0,
            used_cost: 0.0,
            used_turns: 0,
        }
    }

    /// Remaining tokens this agent can consume.
    pub fn remaining_tokens(&self) -> u64 {
        self.max_tokens.map_or(u64::MAX, |max| max.saturating_sub(self.used_tokens))
    }

    /// Remaining cost this agent can consume.
    pub fn remaining_cost(&self) -> f64 {
        self.max_cost_usd.map_or(f64::INFINITY, |max| (max - self.used_cost).max(0.0))
    }

    /// Remaining turns this agent can take.
    pub fn remaining_turns(&self) -> u32 {
        self.max_turns.saturating_sub(self.used_turns)
    }

    /// Is the budget exhausted (any limit reached)?
    pub fn exhausted(&self) -> bool {
        self.max_tokens.map_or(false, |max| self.used_tokens >= max)
            || self.max_cost_usd.map_or(false, |max| self.used_cost >= max)
            || self.remaining_turns() == 0
    }

    /// Can this agent delegate to a subagent?
    /// Requires: max_depth > 0, not exhausted, and remaining budget.
    pub fn can_delegate(&self) -> bool {
        self.max_depth > 0 && !self.exhausted()
    }

    /// Derive a child budget for delegation.
    ///
    /// SentinelAgent P1: child budget = min(parent_remaining, child_inherent).
    /// SentinelAgent P5: max_depth decrements by 1, but never exceeds child's own max_depth.
    pub fn for_child(&self, child_inherent: &Budget) -> Budget {
        Budget {
            max_tokens: narrow_tokens(self.max_tokens, self.used_tokens, child_inherent.max_tokens),
            max_cost_usd: narrow_cost(self.max_cost_usd, self.used_cost, child_inherent.max_cost_usd),
            // P1: narrow turns — child gets min(parent_remaining, child_inherent)
            max_turns: self.remaining_turns().min(child_inherent.max_turns),
            // P5: depth decrements from parent, but capped by child's own max_depth
            // (a leaf child with max_depth=0 stays a leaf even if parent has depth left)
            max_depth: self.max_depth.saturating_sub(1).min(child_inherent.max_depth),
            used_tokens: 0,
            used_cost: 0.0,
            used_turns: 0,
        }
    }

    /// Record token usage. Returns false if budget exhausted after recording.
    pub fn record_usage(&mut self, tokens: u64, cost: f64) -> bool {
        self.used_tokens += tokens;
        self.used_cost += cost;
        self.used_turns += 1;
        !self.exhausted()
    }

    /// Roll up a child's usage into this parent budget.
    pub fn rollup_child(&mut self, child: &Budget) {
        self.used_tokens += child.used_tokens;
        self.used_cost += child.used_cost;
        self.used_turns += child.used_turns;
    }
}

impl Default for Budget {
    fn default() -> Self {
        Self::unlimited()
    }
}

// ─── Narrowing helpers (P1: authority monotonic narrowing) ─────

fn narrow_tokens(
    parent_max: Option<u64>,
    parent_used: u64,
    child_max: Option<u64>,
) -> Option<u64> {
    match (parent_max, child_max) {
        (Some(pmax), Some(cmax)) => {
            let parent_remaining = pmax.saturating_sub(parent_used);
            Some(parent_remaining.min(cmax))
        }
        (Some(pmax), None) => Some(pmax.saturating_sub(parent_used)),
        (None, Some(cmax)) => Some(cmax),
        (None, None) => None,
    }
}

fn narrow_cost(
    parent_max: Option<f64>,
    parent_used: f64,
    child_max: Option<f64>,
) -> Option<f64> {
    match (parent_max, child_max) {
        (Some(pmax), Some(cmax)) => {
            let parent_remaining = (pmax - parent_used).max(0.0);
            Some(parent_remaining.min(cmax))
        }
        (Some(pmax), None) => Some((pmax - parent_used).max(0.0)),
        (None, Some(cmax)) => Some(cmax),
        (None, None) => None,
    }
}

// ─── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unlimited_budget() {
        let b = Budget::unlimited();
        assert!(!b.exhausted());
        assert!(b.can_delegate());
        assert_eq!(b.remaining_tokens(), u64::MAX);
        assert!(b.remaining_cost().is_infinite());
    }

    #[test]
    fn test_leaf_budget_cannot_delegate() {
        let b = Budget::leaf(Some(1000), Some(0.5), 10);
        assert_eq!(b.max_depth, 0);
        assert!(!b.can_delegate());
    }

    #[test]
    fn test_exhausted_tokens() {
        let mut b = Budget::leaf(Some(100), None, 10);
        assert!(b.record_usage(50, 0.0));
        assert!(!b.exhausted());
        assert!(!b.record_usage(50, 0.0));
        assert!(b.exhausted());
    }

    #[test]
    fn test_exhausted_cost() {
        let mut b = Budget::leaf(None, Some(1.0), 10);
        assert!(b.record_usage(0, 0.5));
        assert!(!b.exhausted());
        assert!(!b.record_usage(0, 0.5));
        assert!(b.exhausted());
    }

    #[test]
    fn test_exhausted_turns() {
        let mut b = Budget::leaf(None, None, 2);
        assert!(b.record_usage(10, 0.0));
        assert!(!b.record_usage(10, 0.0));
        assert!(b.exhausted());
    }

    #[test]
    fn test_for_child_narrows_tokens() {
        // Parent has 1000 tokens, used 300, remaining 700
        let parent = Budget {
            max_tokens: Some(1000),
            max_cost_usd: None,
            max_turns: 25,
            max_depth: 2,
            used_tokens: 300,
            used_cost: 0.0,
            used_turns: 3,
        };
        // Child inherent budget: 500 tokens, can delegate once (max_depth=1)
        let child_inherent = Budget {
            max_tokens: Some(500),
            max_cost_usd: None,
            max_turns: 20,
            max_depth: 1,
            used_tokens: 0,
            used_cost: 0.0,
            used_turns: 0,
        };
        let child = parent.for_child(&child_inherent);

        // P1: child gets min(700, 500) = 500
        assert_eq!(child.max_tokens, Some(500));
        // P5: depth = min(2-1, 1) = 1
        assert_eq!(child.max_depth, 1);
        // Child starts with zero usage
        assert_eq!(child.used_tokens, 0);
    }

    #[test]
    fn test_for_child_parent_remaining_smaller() {
        // Parent has 1000 tokens, used 800, remaining 200
        let parent = Budget {
            max_tokens: Some(1000),
            max_cost_usd: None,
            max_turns: 25,
            max_depth: 2,
            used_tokens: 800,
            used_cost: 0.0,
            used_turns: 3,
        };
        // Child inherent: 500 tokens
        let child_inherent = Budget::leaf(Some(500), None, 20);
        let child = parent.for_child(&child_inherent);

        // P1: child gets min(200, 500) = 200
        assert_eq!(child.max_tokens, Some(200));
    }

    #[test]
    fn test_for_child_unlimited_parent() {
        let parent = Budget::unlimited();
        let child_inherent = Budget::leaf(Some(500), Some(0.5), 20);
        let child = parent.for_child(&child_inherent);

        // Parent unlimited → child gets its own inherent budget
        assert_eq!(child.max_tokens, Some(500));
        assert_eq!(child.max_cost_usd, Some(0.5));
    }

    #[test]
    fn test_for_child_depth_zero_is_leaf() {
        let parent = Budget {
            max_tokens: Some(1000),
            max_cost_usd: None,
            max_turns: 25,
            max_depth: 1,
            used_tokens: 0,
            used_cost: 0.0,
            used_turns: 0,
        };
        let child_inherent = Budget::leaf(Some(500), None, 20);
        let child = parent.for_child(&child_inherent);

        // Depth 1 → 0: child is a leaf
        assert_eq!(child.max_depth, 0);
        assert!(!child.can_delegate());
    }

    #[test]
    fn test_rollup_child() {
        let mut parent = Budget {
            max_tokens: Some(10000),
            max_cost_usd: Some(5.0),
            max_turns: 100,
            max_depth: 3,
            used_tokens: 1000,
            used_cost: 0.5,
            used_turns: 5,
        };
        let child = Budget {
            max_tokens: Some(2000),
            max_cost_usd: Some(1.0),
            max_turns: 20,
            max_depth: 2,
            used_tokens: 800,
            used_cost: 0.3,
            used_turns: 8,
        };
        parent.rollup_child(&child);
        assert_eq!(parent.used_tokens, 1800);
        assert!((parent.used_cost - 0.8).abs() < 0.001);
        assert_eq!(parent.used_turns, 13);
    }

    #[test]
    fn test_can_delegate_checks_all_conditions() {
        // Depth 0 → cannot delegate
        let b = Budget::leaf(None, None, 10);
        assert!(!b.can_delegate());

        // Depth > 0 but exhausted → cannot delegate
        let mut b = Budget {
            max_tokens: Some(100),
            max_cost_usd: None,
            max_turns: 10,
            max_depth: 2,
            used_tokens: 100,
            used_cost: 0.0,
            used_turns: 0,
        };
        assert!(!b.can_delegate());

        // Depth > 0 and not exhausted → can delegate
        b.used_tokens = 50;
        assert!(b.can_delegate());
    }

    #[test]
    fn test_narrow_cost() {
        let parent = Budget {
            max_tokens: None,
            max_cost_usd: Some(10.0),
            max_turns: 25,
            max_depth: 2,
            used_tokens: 0,
            used_cost: 3.0,
            used_turns: 0,
        };
        let child_inherent = Budget::leaf(None, Some(5.0), 20);
        let child = parent.for_child(&child_inherent);

        // Parent remaining = 7.0, child inherent = 5.0 → min = 5.0
        assert!((child.max_cost_usd.unwrap() - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_for_child_narrows_turns() {
        // Parent has 25 turns, used 20, remaining 5
        let parent = Budget {
            max_tokens: None,
            max_cost_usd: None,
            max_turns: 25,
            max_depth: 2,
            used_tokens: 0,
            used_cost: 0.0,
            used_turns: 20,
        };
        // Child inherent: 30 turns
        let child_inherent = Budget::leaf(None, None, 30);
        let child = parent.for_child(&child_inherent);

        // P1: child gets min(5, 30) = 5
        assert_eq!(child.max_turns, 5);
    }

    #[test]
    fn test_for_child_respects_leaf_child_max_depth() {
        // Parent has max_depth=3, child is declared as leaf (max_depth=0)
        let parent = Budget {
            max_tokens: None,
            max_cost_usd: None,
            max_turns: 25,
            max_depth: 3,
            used_tokens: 0,
            used_cost: 0.0,
            used_turns: 0,
        };
        let child_inherent = Budget::leaf(None, None, 20); // max_depth=0
        let child = parent.for_child(&child_inherent);

        // Child stays a leaf: min(3-1, 0) = 0
        assert_eq!(child.max_depth, 0);
        assert!(!child.can_delegate());
    }

    #[test]
    fn test_for_child_depth_capped_by_child_inherent() {
        // Parent has max_depth=5, child has max_depth=1
        let parent = Budget {
            max_tokens: None,
            max_cost_usd: None,
            max_turns: 25,
            max_depth: 5,
            used_tokens: 0,
            used_cost: 0.0,
            used_turns: 0,
        };
        let child_inherent = Budget {
            max_tokens: None,
            max_cost_usd: None,
            max_turns: 20,
            max_depth: 1,
            used_tokens: 0,
            used_cost: 0.0,
            used_turns: 0,
        };
        let child = parent.for_child(&child_inherent);

        // min(5-1, 1) = 1 — child can delegate once more, not 4 times
        assert_eq!(child.max_depth, 1);
    }
}
