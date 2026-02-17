//! Lockfree vector pool for pre-generated test vectors.
//!
//! Provides a concurrent-safe pool of pre-generated test vectors,
//! indexed by coverage target. Multiple consumer threads (traversal)
//! can draw vectors without blocking the solver (producer).
//!
//! Uses crossbeam's lock-free ArrayQueue for bounded, wait-free
//! concurrent access.

use std::collections::HashMap;
use std::sync::Arc;

use crossbeam::queue::ArrayQueue;

use super::coverage::CoveragePoint;
use super::TestVector;

/// Default capacity per target queue.
const DEFAULT_QUEUE_CAPACITY: usize = 256;

/// A lockfree pool of pre-generated test vectors.
///
/// Organized into:
/// - A general pool of vectors (not tied to any specific target).
/// - Per-target queues for vectors that specifically cover a given point.
#[derive(Debug)]
pub struct VectorPool {
    /// General-purpose vector queue.
    general: Arc<ArrayQueue<TestVector>>,
    /// Per-coverage-target queues.
    targeted: HashMap<CoveragePointKey, Arc<ArrayQueue<TestVector>>>,
    /// Stats: total vectors pushed.
    pushed: std::sync::atomic::AtomicUsize,
    /// Stats: total vectors popped.
    popped: std::sync::atomic::AtomicUsize,
}

/// A hashable key for coverage points (used as HashMap key).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CoveragePointKey(pub CoveragePoint);

impl VectorPool {
    /// Create a new pool with the given general capacity.
    pub fn new(general_capacity: usize) -> Self {
        Self {
            general: Arc::new(ArrayQueue::new(general_capacity)),
            targeted: HashMap::new(),
            pushed: std::sync::atomic::AtomicUsize::new(0),
            popped: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Create a new pool with default capacity.
    pub fn with_defaults() -> Self {
        Self::new(DEFAULT_QUEUE_CAPACITY * 4)
    }

    /// Register a coverage target with its own queue.
    pub fn register_target(&mut self, target: CoveragePoint) {
        let key = CoveragePointKey(target);
        self.targeted
            .entry(key)
            .or_insert_with(|| Arc::new(ArrayQueue::new(DEFAULT_QUEUE_CAPACITY)));
    }

    /// Push a vector into the general pool.
    /// Returns false if the queue is full.
    pub fn push_general(&self, vector: TestVector) -> bool {
        match self.general.push(vector) {
            Ok(()) => {
                self.pushed
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                true
            }
            Err(_) => false,
        }
    }

    /// Push a vector into a specific target's queue.
    /// Falls back to the general queue if target is unknown.
    pub fn push_targeted(&self, target: &CoveragePoint, vector: TestVector) -> bool {
        let key = CoveragePointKey(target.clone());
        if let Some(queue) = self.targeted.get(&key) {
            match queue.push(vector) {
                Ok(()) => {
                    self.pushed
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    true
                }
                Err(rejected) => {
                    // Target queue full — try general.
                    self.push_general(rejected)
                }
            }
        } else {
            self.push_general(vector)
        }
    }

    /// Pop a vector from the general pool.
    pub fn pop_general(&self) -> Option<TestVector> {
        let result = self.general.pop();
        if result.is_some() {
            self.popped
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        result
    }

    /// Pop a vector from a specific target's queue.
    /// Falls back to general pool if target queue is empty or unknown.
    pub fn pop_targeted(&self, target: &CoveragePoint) -> Option<TestVector> {
        let key = CoveragePointKey(target.clone());
        if let Some(queue) = self.targeted.get(&key) {
            if let Some(v) = queue.pop() {
                self.popped
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                return Some(v);
            }
        }
        // Fallback to general.
        self.pop_general()
    }

    /// Bulk-push vectors into the general pool.
    /// Returns the number successfully pushed.
    pub fn push_many(&self, vectors: Vec<TestVector>) -> usize {
        let mut count = 0;
        for v in vectors {
            if self.push_general(v) {
                count += 1;
            } else {
                break; // Queue full.
            }
        }
        count
    }

    /// Get the number of vectors currently in the general queue.
    pub fn general_len(&self) -> usize {
        self.general.len()
    }

    /// Get the number of vectors in a target's queue.
    pub fn targeted_len(&self, target: &CoveragePoint) -> usize {
        let key = CoveragePointKey(target.clone());
        self.targeted.get(&key).map(|q| q.len()).unwrap_or(0)
    }

    /// Get total vectors pushed (cumulative).
    pub fn total_pushed(&self) -> usize {
        self.pushed.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Get total vectors popped (cumulative).
    pub fn total_popped(&self) -> usize {
        self.popped.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Check if the pool is empty (general + all targets).
    pub fn is_empty(&self) -> bool {
        if !self.general.is_empty() {
            return false;
        }
        for queue in self.targeted.values() {
            if !queue.is_empty() {
                return false;
            }
        }
        true
    }

    /// Get a handle to the general queue for sharing across threads.
    pub fn general_handle(&self) -> Arc<ArrayQueue<TestVector>> {
        Arc::clone(&self.general)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::thread;

    use crate::solver::DomainValue;

    fn make_vector(role: &str, auth: bool) -> TestVector {
        let mut assignments = BTreeMap::new();
        assignments.insert("role".to_string(), DomainValue::Enum(role.to_string()));
        assignments.insert("auth".to_string(), DomainValue::Bool(auth));
        TestVector { assignments }
    }

    #[test]
    fn test_push_pop_general() {
        let pool = VectorPool::with_defaults();

        let v = make_vector("admin", true);
        assert!(pool.push_general(v.clone()));
        assert_eq!(pool.general_len(), 1);

        let popped = pool.pop_general();
        assert_eq!(popped, Some(v));
        assert_eq!(pool.general_len(), 0);
    }

    #[test]
    fn test_push_pop_targeted() {
        let target = CoveragePoint::Boundary {
            var: "role".into(),
            value: DomainValue::Enum("admin".into()),
        };

        let mut pool = VectorPool::with_defaults();
        pool.register_target(target.clone());

        let v = make_vector("admin", true);
        assert!(pool.push_targeted(&target, v.clone()));
        assert_eq!(pool.targeted_len(&target), 1);

        let popped = pool.pop_targeted(&target);
        assert_eq!(popped, Some(v));
        assert_eq!(pool.targeted_len(&target), 0);
    }

    #[test]
    fn test_targeted_fallback_to_general() {
        let target = CoveragePoint::Boundary {
            var: "role".into(),
            value: DomainValue::Enum("admin".into()),
        };

        let mut pool = VectorPool::with_defaults();
        pool.register_target(target.clone());

        // Put a vector in general, nothing in targeted.
        let v = make_vector("admin", true);
        assert!(pool.push_general(v.clone()));

        // Pop from targeted — should fall back to general.
        let popped = pool.pop_targeted(&target);
        assert_eq!(popped, Some(v));
    }

    #[test]
    fn test_bulk_push() {
        let pool = VectorPool::with_defaults();

        let vectors = vec![
            make_vector("admin", true),
            make_vector("admin", false),
            make_vector("guest", true),
        ];

        let pushed = pool.push_many(vectors);
        assert_eq!(pushed, 3);
        assert_eq!(pool.general_len(), 3);
    }

    #[test]
    fn test_pool_full_returns_false() {
        let pool = VectorPool::new(2); // Only 2 slots.

        assert!(pool.push_general(make_vector("admin", true)));
        assert!(pool.push_general(make_vector("admin", false)));
        assert!(!pool.push_general(make_vector("guest", true))); // Full.
    }

    #[test]
    fn test_empty_pool() {
        let pool = VectorPool::with_defaults();
        assert!(pool.is_empty());
        assert!(pool.pop_general().is_none());
    }

    #[test]
    fn test_stats() {
        let pool = VectorPool::with_defaults();

        pool.push_general(make_vector("admin", true));
        pool.push_general(make_vector("admin", false));
        assert_eq!(pool.total_pushed(), 2);
        assert_eq!(pool.total_popped(), 0);

        pool.pop_general();
        assert_eq!(pool.total_popped(), 1);
    }

    #[test]
    fn test_concurrent_pop() {
        let pool = VectorPool::new(100);

        // Pre-fill with 20 vectors.
        for i in 0..20 {
            pool.push_general(make_vector(&format!("role_{i}"), i % 2 == 0));
        }

        let general = pool.general_handle();

        // Spawn 4 consumer threads, each tries to pop 5.
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let queue = Arc::clone(&general);
                thread::spawn(move || {
                    let mut collected = Vec::new();
                    for _ in 0..5 {
                        if let Some(v) = queue.pop() {
                            collected.push(v);
                        }
                    }
                    collected
                })
            })
            .collect();

        let mut all_collected = Vec::new();
        for h in handles {
            all_collected.extend(h.join().unwrap());
        }

        // All 20 vectors should be consumed across the 4 threads.
        assert_eq!(all_collected.len(), 20);
    }

    #[test]
    fn test_concurrent_push_pop() {
        let pool = Arc::new(VectorPool::new(200));

        // Producer thread pushes 50 vectors.
        let producer_pool = Arc::clone(&pool);
        let producer = thread::spawn(move || {
            for i in 0..50 {
                let v = make_vector(&format!("role_{i}"), i % 2 == 0);
                while !producer_pool.push_general(v.clone()) {
                    thread::yield_now();
                }
            }
        });

        // Consumer threads pop vectors.
        let consumer_handles: Vec<_> = (0..4)
            .map(|_| {
                let consumer_pool = Arc::clone(&pool);
                thread::spawn(move || {
                    let mut collected = Vec::new();
                    loop {
                        if let Some(v) = consumer_pool.pop_general() {
                            collected.push(v);
                            if collected.len() >= 12 {
                                break;
                            }
                        } else {
                            thread::yield_now();
                        }
                    }
                    collected
                })
            })
            .collect();

        producer.join().unwrap();

        let mut total = 0;
        for h in consumer_handles {
            total += h.join().unwrap().len();
        }

        // At least 48 consumed (4 threads x 12).
        assert!(total >= 48);
    }
}
