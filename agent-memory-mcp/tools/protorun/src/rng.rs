//! CPython's `random.Random`, reimplemented bit for bit.
//!
//! The streams, the bootstrap confidence intervals and the injected T5 confidences are
//! drawn from seeded Mersenne Twisters. For the Rust run to reproduce the Python run's
//! numbers, the generator has to produce the same sequence, so this is a transcription
//! of Modules/_randommodule.c rather than a substitute RNG. Seeding follows
//! `random_seed()`: the absolute value of the integer becomes a little-endian array of
//! 32-bit words which is fed to `init_by_array`.

const N: usize = 624;
const M: usize = 397;
const MATRIX_A: u32 = 0x9908_b0df;
const UPPER_MASK: u32 = 0x8000_0000;
const LOWER_MASK: u32 = 0x7fff_ffff;

pub struct PyRandom {
    mt: [u32; N],
    index: usize,
}

impl PyRandom {
    /// `random.Random(seed)` for an integer seed.
    pub fn new(seed: i64) -> Self {
        let mut words: Vec<u32> = Vec::new();
        let mut v = seed.unsigned_abs();
        if v == 0 {
            words.push(0);
        }
        while v > 0 {
            words.push((v & 0xffff_ffff) as u32);
            v >>= 32;
        }
        let mut rng = PyRandom {
            mt: [0u32; N],
            index: 0,
        };
        rng.init_by_array(&words);
        rng
    }

    fn init_genrand(&mut self, s: u32) {
        self.mt[0] = s;
        for i in 1..N {
            let prev = self.mt[i - 1];
            self.mt[i] = 1_812_433_253u32
                .wrapping_mul(prev ^ (prev >> 30))
                .wrapping_add(i as u32);
        }
        self.index = N;
    }

    fn init_by_array(&mut self, key: &[u32]) {
        self.init_genrand(19650218);
        let key_length = key.len();
        let mut i = 1usize;
        let mut j = 0usize;
        let mut k = if N > key_length { N } else { key_length };
        while k > 0 {
            let prev = self.mt[i - 1];
            self.mt[i] = (self.mt[i] ^ ((prev ^ (prev >> 30)).wrapping_mul(1_664_525)))
                .wrapping_add(key[j])
                .wrapping_add(j as u32);
            i += 1;
            j += 1;
            if i >= N {
                self.mt[0] = self.mt[N - 1];
                i = 1;
            }
            if j >= key_length {
                j = 0;
            }
            k -= 1;
        }
        k = N - 1;
        while k > 0 {
            let prev = self.mt[i - 1];
            self.mt[i] = (self.mt[i] ^ ((prev ^ (prev >> 30)).wrapping_mul(1_566_083_941)))
                .wrapping_sub(i as u32);
            i += 1;
            if i >= N {
                self.mt[0] = self.mt[N - 1];
                i = 1;
            }
            k -= 1;
        }
        self.mt[0] = 0x8000_0000;
    }

    /// `genrand_uint32`.
    pub fn next_u32(&mut self) -> u32 {
        if self.index >= N {
            let mag01 = [0u32, MATRIX_A];
            for kk in 0..(N - M) {
                let y = (self.mt[kk] & UPPER_MASK) | (self.mt[kk + 1] & LOWER_MASK);
                self.mt[kk] = self.mt[kk + M] ^ (y >> 1) ^ mag01[(y & 1) as usize];
            }
            for kk in (N - M)..(N - 1) {
                let y = (self.mt[kk] & UPPER_MASK) | (self.mt[kk + 1] & LOWER_MASK);
                self.mt[kk] = self.mt[kk + M - N] ^ (y >> 1) ^ mag01[(y & 1) as usize];
            }
            let y = (self.mt[N - 1] & UPPER_MASK) | (self.mt[0] & LOWER_MASK);
            self.mt[N - 1] = self.mt[M - 1] ^ (y >> 1) ^ mag01[(y & 1) as usize];
            self.index = 0;
        }
        let mut y = self.mt[self.index];
        self.index += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^= y >> 18;
        y
    }

    /// `random.random()` — 53 random bits.
    pub fn random(&mut self) -> f64 {
        let a = (self.next_u32() >> 5) as f64;
        let b = (self.next_u32() >> 6) as f64;
        (a * 67_108_864.0 + b) * (1.0 / 9_007_199_254_740_992.0)
    }

    /// `random.getrandbits(k)` for 1 <= k <= 32.
    pub fn getrandbits(&mut self, k: u32) -> u64 {
        debug_assert!(k >= 1 && k <= 32);
        (self.next_u32() >> (32 - k)) as u64
    }

    /// `random._randbelow_with_getrandbits(n)`.
    pub fn randbelow(&mut self, n: u64) -> u64 {
        debug_assert!(n > 0);
        let k = 64 - n.leading_zeros(); // n.bit_length()
        let mut r = self.getrandbits(k);
        while r >= n {
            r = self.getrandbits(k);
        }
        r
    }

    /// `random.randrange(a, b)`.
    pub fn randrange(&mut self, a: i64, b: i64) -> i64 {
        debug_assert!(b > a);
        a + self.randbelow((b - a) as u64) as i64
    }

    /// `random.choice(seq)` — the index it picks.
    pub fn choice_index(&mut self, len: usize) -> usize {
        self.randbelow(len as u64) as usize
    }

    /// `random.uniform(a, b)`.
    pub fn uniform(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.random()
    }
}
