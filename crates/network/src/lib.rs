mod discovery;
mod local_address;
mod pairing;
mod transport;

pub use discovery::{
    DiscoveredPeer, DiscoveryConfig, DiscoveryEvent, DiscoveryService, SERVICE_TYPE,
};
pub use local_address::local_address;
pub use pairing::PairingCodeManager;
pub use transport::{
    LanTransport, PairingCandidate, TransportCredentials, TransportEvent, TransportIdentity,
    TrustedPeer,
};

pub const DEFAULT_QUIC_PORT: u16 = 53_317;
