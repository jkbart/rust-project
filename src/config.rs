// TODO: make config be read at a start of program from external file to avoid recompilation after changing any setting.

// TODO array of multicasts address in case of busy port.
pub static MULTICAST_IP: &str = "239.42.17.19";
pub static MULTICAST_PORT: u16 = 7899;

pub static UNIQUE_BYTES: &[u8] = b"CHATapp>4RxPOv@1Gy8SZ8syH7$MlVAA2>0y]D`%KTIN\"Y[Lk9Z}\"k{p)";

use std::path::PathBuf;
use once_cell::sync::Lazy;
use rand::{distributions::Alphanumeric, Rng};
use directories::UserDirs;

// Lazily initialized static variable for USER_ID
pub static USER_ID: Lazy<u64> = Lazy::new(|| {
    let mut rng = rand::thread_rng();
    rng.gen() // Generate a random u64
});

pub static USER_NAME: Lazy<String> = Lazy::new(|| {
    let mut rng = rand::thread_rng();
    let random_str: String = (0..10).map(|_| rng.sample(Alphanumeric) as char).collect();
    random_str
});

pub static DOWNLOAD_PATH: Lazy<PathBuf> = Lazy::new(|| {
    UserDirs::new().unwrap().download_dir().unwrap().to_path_buf()
});