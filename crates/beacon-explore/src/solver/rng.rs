//! Per-stage RNG seeding with ChaCha8.
//!
//! Each fracture stage gets its own ChaCha8Rng seeded from
//! `(global_seed + stage_id)`. Same seed -> same vectors, always.

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

/// Create a deterministic RNG for a given global seed and stage ID.
pub fn stage_rng(global_seed: u64, stage_id: u64) -> ChaCha8Rng {
    let combined = global_seed.wrapping_add(stage_id);
    ChaCha8Rng::seed_from_u64(combined)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    #[test]
    fn test_deterministic_rng() {
        let mut rng1 = stage_rng(42, 0);
        let mut rng2 = stage_rng(42, 0);

        let vals1: Vec<u64> = (0..10).map(|_| rng1.gen()).collect();
        let vals2: Vec<u64> = (0..10).map(|_| rng2.gen()).collect();

        assert_eq!(vals1, vals2);
    }

    #[test]
    fn test_different_stages_different_output() {
        let mut rng1 = stage_rng(42, 0);
        let mut rng2 = stage_rng(42, 1);

        let val1: u64 = rng1.gen();
        let val2: u64 = rng2.gen();

        assert_ne!(val1, val2);
    }

    #[test]
    fn test_different_seeds_different_output() {
        let mut rng1 = stage_rng(42, 0);
        let mut rng2 = stage_rng(43, 0);

        let val1: u64 = rng1.gen();
        let val2: u64 = rng2.gen();

        assert_ne!(val1, val2);
    }
}
