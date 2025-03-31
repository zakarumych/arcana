use std::num::NonZeroU64;

use super::GenId;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct SeqIdGen {
    next_id: u64,
}

impl Default for SeqIdGen {
    #[inline(always)]
    fn default() -> Self {
        SeqIdGen::new()
    }
}

impl SeqIdGen {
    #[inline(always)]
    pub const fn new() -> Self {
        SeqIdGen { next_id: 1 }
    }

    #[inline(always)]
    pub const fn next(&mut self) -> NonZeroU64 {
        if self.next_id == 0 {
            panic!("SeqIdGen overflow");
        }

        let value = NonZeroU64::new(self.next_id).unwrap();
        self.next_id += 1;
        value
    }
}

impl GenId for SeqIdGen {
    type Value = NonZeroU64;

    #[inline(always)]
    fn generate(&mut self) -> NonZeroU64 {
        self.next()
    }
}
