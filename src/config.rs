// TODO: make config be read at a start of program from external file to avoid recompilation after changing any setting.

// Arbitrary picked ip and port for multicast discovery.
// Must be ready to take n datagrams per broadcast delay.
pub const MULTICAST_IP: &str = "239.42.17.19"; // Any address between 224.0.0.1 – 239.255.255.255 will do.
pub const MULTICAST_PORT: u16 = 7878;

pub static UNIQUE_BYTES: &[u8] = b"CHATapp>4RxPOv@1Gy8SZ8syH7$MlVAA2>0y]D`%KTIN\"Y[Lk9Z}\"k{p)";

pub const USER_ID: u64 = 123123; // TODO: lazy generate random id.
