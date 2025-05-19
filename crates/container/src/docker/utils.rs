use bollard::models::PortBinding;
use bollard::models::PortMap;
use bollard::secret::ContainerInspectResponse;

pub fn port_bindings(host_port: u16, container_port: u16) -> Option<PortMap> {
    let mut ports = PortMap::new();
    ports.insert(
        tcp_port(&container_port.to_string()),
        Some(vec![PortBinding {
            host_port: Some(host_port.to_string()),
            host_ip: Some("127.0.0.1".to_string()),
        }]),
    );
    Some(ports)
}

pub fn tcp_port(p: &str) -> String {
    format!("{}/tcp", p).to_string()
}

pub fn parse_container_inspect_response_port(inspect: &ContainerInspectResponse) -> u16 {
    let ports = inspect
        .host_config
        .as_ref()
        .and_then(|host_config| host_config.port_bindings.as_ref())
        .map(|ports| {
            ports
                .into_iter()
                .filter_map(|(_, host_config)| {
                    host_config.as_ref().map(|host_config| {
                        host_config
                            .iter()
                            .filter_map(|port_config| port_config.host_port.clone())
                            .collect::<Vec<_>>()
                    })
                })
                .collect::<Vec<_>>()
        });

    let ports = ports
        .as_ref()
        .into_iter()
        .filter_map(|ports| ports.first())
        .filter_map(|port| port.first())
        .collect::<Vec<_>>();
    ports
        .first()
        .and_then(|port| port.parse::<u16>().ok())
        .unwrap_or(0)
}
