mod manager;
mod profile_ex;

pub use manager::{ProfileManager, ProfileManagerError, Result};
pub use profile_ex::ProfileExManager;

const DEFAULT_PROFILE_SORT_STEP: i32 = 10;
const DEFAULT_NETWORK: &str = "tcp";
const STREAM_SECURITY_TLS: &str = "tls";
const STREAM_SECURITY_REALITY: &str = "reality";
const VALID_NETWORKS: &[&str] = &[
    "tcp",
    "kcp",
    "ws",
    "http",
    "h2",
    "quic",
    "grpc",
    "httpupgrade",
    "xhttp",
];
