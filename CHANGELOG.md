# Changelog

## 0.1.12

- Fix paired devices remaining offline when an incompatible duplicate mDNS service or an unreachable interface address displaced their working endpoint. Keep compatible IPv4 candidates and retry the last authenticated endpoint, discovered addresses, and the saved IP's reserved UDP ports.
- Resolve simultaneous reconnects consistently on both peers and remove disconnected sessions atomically, preventing an old session from removing a replacement.
- Save the actual remote endpoint on both sides of pairing, including when multicast discovery is unavailable. Register approved trust before sending acceptance, and preserve successful pairing if the first authenticated connection needs a retry.
- Accept an IP or DNS hostname with an optional port for manual pairing across routed IPv4 subnets. Routing and firewall rules must permit device-to-device UDP traffic. This does not provide cross-VLAN multicast discovery or a relay through network isolation.
- Generate and accept four-digit pairing codes with the existing 60-second lifetime, single-use behavior, attempt limits, and receiving-device approval. Both devices need 0.1.12 for new pairing; existing protocol-v3 trust remains compatible.

Validation includes frontend tests, Rust workspace tests (both pairing directions, authenticated endpoint fallback, simultaneous reconnect, discovery filtering, and code expiry), browser smoke checks, and a signed/notarized macOS ARM64 build. The notarized macOS app also reconnected to the existing trusted Ubuntu device at 10.253.19.25:53317 without re-pairing on 2026-09-08. New four-digit pairing on a physical Ubuntu 0.1.12 installation still needs verification; automated bidirectional tests emulate platform identities.
