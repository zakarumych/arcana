use std::num::NonZeroU64;

use rand::random;

use super::GenId;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ShufIdGen {
    shuf_mul: u64,
    next_id: u64,
}

impl Default for ShufIdGen {
    #[inline(always)]
    fn default() -> Self {
        ShufIdGen::new()
    }
}

impl ShufIdGen {
    #[inline(always)]
    pub fn new() -> Self {
        ShufIdGen {
            shuf_mul: random::<u64>() | 1,
            next_id: 1,
        }
    }

    #[inline(always)]
    pub fn next(&mut self) -> NonZeroU64 {
        if self.next_id == 0 {
            panic!("ShufIdGen overflow");
        }

        let value = NonZeroU64::new(self.next_id.wrapping_mul(self.shuf_mul)).unwrap();
        self.next_id += 1;
        value
    }
}

impl GenId for ShufIdGen {
    type Value = NonZeroU64;

    #[inline(always)]
    fn generate(&mut self) -> NonZeroU64 {
        self.next()
    }
}
