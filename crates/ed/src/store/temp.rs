use std::path::{Path, PathBuf};

use base64::{
    alphabet::URL_SAFE,
    engine::general_purpose::{GeneralPurpose, NO_PAD},
    Engine,
};
use rand::random;

pub fn make_temporary(base: &Path) -> PathBuf {
    loop {
        let key: u128 = random();
        let key_bytes = key.to_le_bytes();
        let mut filename = [0; 22];
        let len = GeneralPurpose::new(&URL_SAFE, NO_PAD)
            .encode_slice(&key_bytes, &mut filename)
            .unwrap();
        debug_assert_eq!(len, 22);
        let path = base.join(std::str::from_utf8(&filename).unwrap());
        if !path.exists() {
            return path;
        }
    }
}
