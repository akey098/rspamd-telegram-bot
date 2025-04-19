use get_if_addrs::{get_if_addrs, IfAddr};

pub fn detect_local_ipv4() -> Option<String> {
    if let Ok(ifaces) = get_if_addrs() {
        for iface in ifaces {
            if let IfAddr::V4(v4addr) = iface.addr {
                let ip = v4addr.ip;
                if !ip.is_loopback() {
                    return Some(format!("{}/32", ip));
                }
            }
        }
    }
    None
}