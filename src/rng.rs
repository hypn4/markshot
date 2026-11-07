//! The seeded generator that sketches and pixelation draw from, and the default seed derived
//! from an id.

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// The largest seed, 2^53 − 1: JSON tools that read numbers as doubles (JavaScript, jq) keep
/// integers up to it exact, so a seed in a resolved spec survives being edited with them.
pub const MAX_SEED: u64 = (1 << 53) - 1;

/// A xorshift-multiply generator; the multiplied value is the next state.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// The next draw in `[0, 1]`: the low 32 bits of the new state over `u32::MAX`.
    pub fn next_unit(&mut self) -> f64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        x = x.wrapping_mul(0x2545_F491_4F6C_DD1D);
        self.0 = x;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a draw keeps the low 32 bits of the state"
        )]
        let low = x as u32;
        f64::from(low) / 4_294_967_295.0
    }
}

/// The `rnd` table that sketches index: the first 100 draws from `seed`.
#[must_use]
pub fn rnd100(seed: u64) -> [f64; 100] {
    let mut rng = Rng::new(seed);
    std::array::from_fn(|_| rng.next_unit())
}

/// The default seed for an annotation: the low 53 bits (at most [`MAX_SEED`]) of the 64-bit
/// FNV-1a hash of its id's UTF-8 bytes.
#[must_use]
pub fn seed_from_id(id: &str) -> u64 {
    let hash = id.bytes().fold(FNV_OFFSET_BASIS, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(FNV_PRIME)
    });
    hash & MAX_SEED
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_vectors() {
        let r = rnd100(1);
        assert_eq!(
            &r[..3],
            &[
                0.536_817_378_722_321_6,
                0.777_644_203_458_364_2,
                0.818_594_869_882_472_5
            ]
        );
        assert_eq!(
            rnd100(0xDEAD_BEEF)[..2],
            [0.712_917_021_176_059_9, 0.072_543_157_048_649_89]
        );
    }

    #[test]
    fn rnd100_is_the_first_hundred_draws_of_the_stream() {
        let mut rng = Rng::new(42);
        let draws: Vec<f64> = (0..100).map(|_| rng.next_unit()).collect();
        assert_eq!(rnd100(42)[..], draws[..]);
        assert!(draws.iter().all(|u| (0.0..=1.0).contains(u)));
    }

    #[test]
    fn zero_seed_stays_zero() {
        // xorshift has 0 as a fixed point: seed 0 draws nothing but zeros.
        assert_eq!(rnd100(0), [0.0; 100]);
    }

    #[test]
    fn fnv() {
        // The low 53 bits of FNV-1a 64: 0x1adf_0676_39bf_aaa0 for "arrow-1", the offset basis
        // 0xcbf2_9ce4_8422_2325 for "".
        assert_eq!(seed_from_id("arrow-1"), 0x1f_0676_39bf_aaa0);
        assert_eq!(seed_from_id(""), 0x12_9ce4_8422_2325);
    }

    #[test]
    fn default_seeds_fit_in_a_double() {
        // JSON tools that read numbers as doubles keep integers up to 2^53 exact.
        for id in ["arrow-1", "rect-1", "oval-1", "blur-1", "redaction", ""] {
            let seed = seed_from_id(id);
            assert!(seed <= MAX_SEED, "{id}: {seed}");
            #[expect(
                clippy::cast_precision_loss,
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "checking that the round trip through f64 is exact"
            )]
            let back = seed as f64 as u64;
            assert_eq!(back, seed, "{id}");
        }
    }
}
