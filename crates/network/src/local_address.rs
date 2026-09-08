use std::net::{IpAddr, SocketAddr};

/// Read current interfaces on each refresh so network changes reach the device view.
pub fn local_address(port: u16) -> Option<SocketAddr> {
    let interfaces = if_addrs::get_if_addrs().ok()?;
    let addresses = interfaces
        .iter()
        .filter(|interface| interface.is_oper_up() && !interface.is_p2p())
        .map(|interface| interface.ip());
    preferred_local_ip(addresses).map(|ip| SocketAddr::new(ip, port))
}

fn preferred_local_ip(addresses: impl Iterator<Item = IpAddr>) -> Option<IpAddr> {
    addresses
        .filter(|ip| {
            !ip.is_loopback()
                && !ip.is_unspecified()
                && !ip.is_multicast()
                && match ip {
                    IpAddr::V4(ip) => !ip.is_link_local() && !ip.is_broadcast(),
                    // Link-local IPv6 requires a scope ID, which this view does not carry.
                    IpAddr::V6(ip) => !ip.is_unicast_link_local(),
                }
        })
        .min_by_key(|ip| {
            let priority = match ip {
                IpAddr::V4(ip) if ip.is_private() => 0,
                IpAddr::V4(_) => 1,
                IpAddr::V6(_) => 2,
            };
            (priority, *ip)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn select(addresses: &[&str]) -> Option<IpAddr> {
        preferred_local_ip(addresses.iter().map(|address| address.parse().unwrap()))
    }

    #[test]
    fn prefers_lan_ipv4_and_has_stable_order() {
        assert_eq!(
            select(&["fd00::1", "8.8.8.8", "192.168.1.12", "10.0.0.2"]),
            Some("10.0.0.2".parse().unwrap())
        );
        assert_eq!(select(&["fd00::1"]), Some("fd00::1".parse().unwrap()));
    }

    #[test]
    fn unavailable_network_does_not_show_unusable_addresses() {
        assert_eq!(select(&[]), None);
        assert_eq!(
            select(&[
                "127.0.0.1",
                "::1",
                "0.0.0.0",
                "::",
                "224.0.0.1",
                "ff02::1",
                "169.254.1.1",
                "fe80::1",
                "255.255.255.255"
            ]),
            None
        );
    }
}
