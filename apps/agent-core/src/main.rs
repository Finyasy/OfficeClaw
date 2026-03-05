use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;

use agent_core::agent::orchestrator::Orchestrator;
use agent_core::api::grpc::AgentGatewayService;
use agent_core::proto::agent_gateway_server::AgentGatewayServer;
use tonic::transport::Server;

fn grpc_port() -> u16 {
    std::env::var("GRPC_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(50051)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = SocketAddr::from(([0, 0, 0, 0], grpc_port()));

    let allowlist_domains = HashSet::from(["contoso.com".to_string()]);
    let known_recipients = HashSet::from(["james@contoso.com".to_string()]);
    let orchestrator = Arc::new(Orchestrator::new(allowlist_domains, known_recipients));

    let service = AgentGatewayService::new(orchestrator);

    println!("agent-core gRPC listening on {}", addr);

    Server::builder()
        .add_service(AgentGatewayServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
