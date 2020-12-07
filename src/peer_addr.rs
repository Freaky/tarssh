use std::fmt;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};

/// A compact representation of an IP and port pair
#[derive(Debug, Clone, Copy)]
#[repr(packed(2))]
pub struct PeerAddr {
    ip: u128,
    port: u16,
}

impl From<&SocketAddr> for PeerAddr {
    fn from(peer: &SocketAddr) -> Self {
        let ip = match peer.ip() {
            IpAddr::V4(v4) => v4.to_ipv6_mapped().into(),
            IpAddr::V6(v6) => v6.into(),
        };

        Self {
            ip,
            port: peer.port(),
        }
    }
}

impl From<&PeerAddr> for SocketAddr {
    fn from(peer: &PeerAddr) -> Self {
        let ip = Ipv6Addr::from(peer.ip);
        let ip = ip
            .to_ipv4()
            .map(IpAddr::V4)
            .unwrap_or_else(|| IpAddr::V6(ip));

        SocketAddr::new(ip, peer.port)
    }
}

impl From<SocketAddr> for PeerAddr {
    fn from(peer: SocketAddr) -> Self {
        Self::from(&peer)
    }
}

impl From<PeerAddr> for SocketAddr {
    fn from(peer: PeerAddr) -> Self {
        Self::from(&peer)
    }
}

impl fmt::Display for PeerAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        SocketAddr::from(self).fmt(f)
    }
}
