use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
    thread,
};

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use parking_lot::Mutex;
use synchalo_core::{AppError, DevicePlatform, PROTOCOL_VERSION};
use tokio::sync::mpsc;
use uuid::Uuid;

pub const SERVICE_TYPE: &str = "_synchalo._udp.local.";

#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    pub device_id: Uuid,
    pub device_name: String,
    pub platform: DevicePlatform,
    pub port: u16,
    pub pairing_open: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredPeer {
    pub device_id: Uuid,
    pub device_name: String,
    pub platform: DevicePlatform,
    pub address: SocketAddr,
    pub pairing_open: bool,
    pub protocol_version: u16,
    pub fullname: String,
}

#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    Resolved(DiscoveredPeer),
    Removed { fullname: String },
    Error(String),
}

pub struct DiscoveryService {
    daemon: ServiceDaemon,
    config: Arc<Mutex<DiscoveryConfig>>,
    fullname: Arc<Mutex<String>>,
    events: Option<mpsc::UnboundedReceiver<DiscoveryEvent>>,
    listener: Option<thread::JoinHandle<()>>,
}

impl DiscoveryService {
    pub fn start(config: DiscoveryConfig) -> Result<Self, AppError> {
        let daemon = ServiceDaemon::new().map_err(network_error)?;
        let info = service_info(&config)?;
        let fullname = info.get_fullname().to_owned();
        daemon.register(info).map_err(network_error)?;
        let browser = daemon.browse(SERVICE_TYPE).map_err(network_error)?;
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let local_device_id = config.device_id;

        let listener = thread::Builder::new()
            .name("synchalo-mdns".to_owned())
            .spawn(move || {
                while let Ok(event) = browser.recv() {
                    let mapped = match event {
                        ServiceEvent::ServiceResolved(info) => {
                            parse_peer(&info, local_device_id).map(DiscoveryEvent::Resolved)
                        }
                        ServiceEvent::ServiceRemoved(_, fullname) => {
                            Some(DiscoveryEvent::Removed { fullname })
                        }
                        _ => None,
                    };
                    if let Some(event) = mapped
                        && events_tx.send(event).is_err()
                    {
                        break;
                    }
                }
            })
            .map_err(network_error)?;

        Ok(Self {
            daemon,
            config: Arc::new(Mutex::new(config)),
            fullname: Arc::new(Mutex::new(fullname)),
            events: Some(events_rx),
            listener: Some(listener),
        })
    }

    pub fn take_events(&mut self) -> Option<mpsc::UnboundedReceiver<DiscoveryEvent>> {
        self.events.take()
    }

    pub fn set_pairing_open(&self, pairing_open: bool) -> Result<(), AppError> {
        let mut config = self.config.lock();
        if config.pairing_open == pairing_open {
            return Ok(());
        }
        config.pairing_open = pairing_open;
        let old_fullname = self.fullname.lock().clone();
        let _ = self.daemon.unregister(&old_fullname);
        let info = service_info(&config)?;
        *self.fullname.lock() = info.get_fullname().to_owned();
        self.daemon.register(info).map_err(network_error)
    }

    pub fn set_device_name(&self, device_name: String) -> Result<(), AppError> {
        let mut config = self.config.lock();
        if config.device_name == device_name {
            return Ok(());
        }
        config.device_name = device_name;
        let old_fullname = self.fullname.lock().clone();
        let _ = self.daemon.unregister(&old_fullname);
        let info = service_info(&config)?;
        *self.fullname.lock() = info.get_fullname().to_owned();
        self.daemon.register(info).map_err(network_error)
    }
}

impl Drop for DiscoveryService {
    fn drop(&mut self) {
        let fullname = self.fullname.lock().clone();
        let _ = self.daemon.stop_browse(SERVICE_TYPE);
        let _ = self.daemon.unregister(&fullname);
        let _ = self.daemon.shutdown();
        if let Some(listener) = self.listener.take() {
            let _ = listener.join();
        }
    }
}

fn service_info(config: &DiscoveryConfig) -> Result<ServiceInfo, AppError> {
    let platform = match config.platform {
        DevicePlatform::Macos => "macos",
        DevicePlatform::Linux => "linux",
        DevicePlatform::Unknown => "unknown",
    };
    let properties = [
        ("v", PROTOCOL_VERSION.to_string()),
        ("id", config.device_id.to_string()),
        ("name", config.device_name.clone()),
        ("platform", platform.to_owned()),
        (
            "pair",
            if config.pairing_open { "1" } else { "0" }.to_owned(),
        ),
    ];
    let instance_name = format!("sh-{}", &config.device_id.simple().to_string()[..12]);
    let hostname = format!("{instance_name}.local.");
    ServiceInfo::new(
        SERVICE_TYPE,
        &instance_name,
        &hostname,
        "",
        config.port,
        &properties[..],
    )
    .map(ServiceInfo::enable_addr_auto)
    .map_err(network_error)
}

fn parse_peer(info: &mdns_sd::ResolvedService, local_device_id: Uuid) -> Option<DiscoveredPeer> {
    let device_id = Uuid::parse_str(info.get_property_val_str("id")?).ok()?;
    if device_id == local_device_id {
        return None;
    }
    let device_name = info
        .get_property_val_str("name")
        .unwrap_or("附近设备")
        .to_owned();
    let platform = match info.get_property_val_str("platform") {
        Some("macos") => DevicePlatform::Macos,
        Some("linux") => DevicePlatform::Linux,
        _ => DevicePlatform::Unknown,
    };
    let protocol_version = info
        .get_property_val_str("v")
        .and_then(|value| value.parse().ok())
        .unwrap_or_default();
    let pairing_open = info.get_property_val_str("pair") == Some("1");
    let address = preferred_address(info.get_addresses().iter().map(|value| value.to_ip_addr()))?;
    Some(DiscoveredPeer {
        device_id,
        device_name,
        platform,
        address: SocketAddr::new(address, info.get_port()),
        pairing_open,
        protocol_version,
        fullname: info.get_fullname().to_owned(),
    })
}

fn preferred_address(addresses: impl Iterator<Item = IpAddr>) -> Option<IpAddr> {
    let addresses: Vec<_> = addresses.filter(|address| !address.is_loopback()).collect();
    addresses
        .iter()
        .copied()
        .find(IpAddr::is_ipv4)
        .or_else(|| addresses.first().copied())
}

fn network_error(error: impl std::fmt::Display) -> AppError {
    AppError::Network(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairing_flag_can_be_republished() {
        let service = DiscoveryService::start(DiscoveryConfig {
            device_id: Uuid::new_v4(),
            device_name: "Discovery test".to_owned(),
            platform: DevicePlatform::current(),
            port: 53_327,
            pairing_open: false,
        })
        .unwrap();
        service.set_pairing_open(true).unwrap();
        service.set_pairing_open(false).unwrap();
    }
}
