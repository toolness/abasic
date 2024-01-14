use crate::InterpreterError;

const MODULUS: u64 = 2 << 32;
const MULTIPLIER: u64 = 1664525;
const INCREMENT: u64 = 1013904223;

#[derive(Default, Debug)]
pub struct Rng {
    seed: u64,
}

/// A simple linear congruential random number generator, as described in
/// https://en.wikipedia.org/wiki/Linear_congruential_generator.
///
/// The parameters for this RNG are taken from Numerical Recipes
/// by Knuth and H. W. Lewis.
impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng { seed }
    }

    pub fn random(&mut self) -> f64 {
        self.seed = (MULTIPLIER * self.seed + INCREMENT) % MODULUS;
        self.latest_random()
    }

    pub fn latest_random(&self) -> f64 {
        (self.seed as f64) / (MODULUS as f64)
    }

    /// Simulates a BASIC-style RND call.
    pub fn rnd(&mut self, number: f64) -> Result<f64, InterpreterError> {
        // Applesoft BASIC would always return the most recent random with the argument '0', and
        // predefined items in the sequence with '-1', but in practice all the code I've seen
        // just calls it with '1', and *any* positive number is supposed to return a random number
        // in the interval [0, 1).
        if number < 0.0 {
            // None of the code I've seen actually uses this, and
            // I don't fully understand what it means, so just don't
            // support it for now.
            Err(InterpreterError::Unimplemented)
        } else if number == 0.0 {
            Ok(self.latest_random())
        } else {
            Ok(self.random())
        }
    }
}

impl Iterator for Rng {
    type Item = f64;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.random())
    }
}
