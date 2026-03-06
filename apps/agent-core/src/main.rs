use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;

use agent_core::agent::orchestrator::Orchestrator;
use agent_core::agent::proactive::{HttpProactiveNotifier, NoopProactiveNotifier, ProactiveNotifier};
use agent_core::api::grpc::AgentGatewayService;
use agent_core::crypto::envelope::{
    EnvelopeCipher, KeyVaultConfig, KeyVaultCredential, KeyVaultEnvelopeCipher,
    ManagedIdentityConfig, PlaintextEnvelopeCipher,
};
use agent_core::proto::agent_gateway_server::AgentGatewayServer;
use agent_core::skills::graph::calendar::{
    CalendarEventCreator, CalendarReader, GraphCalendarEventCreator, GraphCalendarReader,
};
use agent_core::skills::graph::client::{GraphClient, GraphClientConfig};
use agent_core::skills::graph::mail::{GraphMailReader, GraphMailSender, MailReader, MailSender};
use agent_core::storage::approvals_repo::{ApprovalsRepo, InMemoryApprovalsRepo, PostgresApprovalsRepo};
use agent_core::storage::audit_repo::{AuditRepo, InMemoryAuditRepo, PostgresAuditRepo};
use agent_core::storage::conversation_refs_repo::{
    ConversationRefsRepo, InMemoryConversationRefsRepo, PostgresConversationRefsRepo,
};
use agent_core::storage::db::{Database, DatabaseConfig};
use agent_core::storage::migrations;
use agent_core::storage::sessions_repo::{
    InMemorySessionsRepo, PostgresSessionsRepo, SessionsRepo,
};
use agent_core::storage::tokens_repo::{InMemoryTokensRepo, PostgresTokensRepo, TokensRepo};
use tonic::transport::Server;

fn grpc_port() -> u16 {
    std::env::var("GRPC_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(50051)
}

async fn database() -> Option<Database> {
    match std::env::var("DATABASE_URL") {
        Ok(url) if !url.trim().is_empty() => match Database::connect(&DatabaseConfig { url }).await
        {
            Ok(database) => {
                if let Err(error) = migrations::run(&database).await {
                    eprintln!("database migrations failed: {}", error);
                    None
                } else {
                    Some(database)
                }
            }
            Err(error) => {
                eprintln!("database unavailable: {}", error);
                None
            }
        },
        _ => None,
    }
}

fn sessions_repo(database: Option<Database>) -> Arc<dyn SessionsRepo> {
    match database {
        Some(database) => Arc::new(PostgresSessionsRepo::new(database)),
        None => Arc::new(InMemorySessionsRepo::new()),
    }
}

fn audit_repo(database: Option<Database>) -> Arc<dyn AuditRepo> {
    match database {
        Some(database) => Arc::new(PostgresAuditRepo::new(database)),
        None => Arc::new(InMemoryAuditRepo::new()),
    }
}

fn approvals_repo(database: Option<Database>) -> Arc<dyn ApprovalsRepo> {
    match database {
        Some(database) => Arc::new(PostgresApprovalsRepo::new(database)),
        None => Arc::new(InMemoryApprovalsRepo::new()),
    }
}

fn conversation_refs_repo(database: Option<Database>) -> Arc<dyn ConversationRefsRepo> {
    match database {
        Some(database) => Arc::new(PostgresConversationRefsRepo::new(database)),
        None => Arc::new(InMemoryConversationRefsRepo::new()),
    }
}

fn token_cipher() -> Arc<dyn EnvelopeCipher> {
    let keyvault_uri = std::env::var("KEYVAULT_URI").ok();
    let kek_name = std::env::var("KEYVAULT_KEK_NAME").ok();

    if let (Some(vault_uri), Some(kek_name)) = (keyvault_uri, kek_name) {
        let api_version =
            std::env::var("KEYVAULT_API_VERSION").unwrap_or_else(|_| "7.4".to_string());
        let credential = match std::env::var("KEYVAULT_BEARER_TOKEN") {
            Ok(token) if !token.trim().is_empty() => KeyVaultCredential::StaticToken(token),
            _ => KeyVaultCredential::ManagedIdentity(ManagedIdentityConfig {
                endpoint: std::env::var("IDENTITY_ENDPOINT").ok(),
                secret_header: std::env::var("IDENTITY_HEADER").ok(),
            }),
        };

        return Arc::new(KeyVaultEnvelopeCipher::new(
            KeyVaultConfig {
                vault_uri,
                kek_name,
                api_version,
            },
            credential,
        ));
    }

    let key_version =
        std::env::var("TOKEN_ENCRYPTION_KEY_VERSION").unwrap_or_else(|_| "dev-local".to_string());
    eprintln!("warning: falling back to plaintext token cipher; configure Key Vault for production");
    Arc::new(PlaintextEnvelopeCipher::new(key_version))
}

fn tokens_repo(database: Option<Database>, cipher: Arc<dyn EnvelopeCipher>) -> Arc<dyn TokensRepo> {
    match database {
        Some(database) => Arc::new(PostgresTokensRepo::new(database, cipher)),
        None => Arc::new(InMemoryTokensRepo::new()),
    }
}

fn graph_client() -> GraphClient {
    let base_url = std::env::var("GRAPH_BASE_URL")
        .unwrap_or_else(|_| "https://graph.microsoft.com/v1.0".to_string());
    GraphClient::new(GraphClientConfig { base_url })
}

fn mail_reader(tokens_repo: Arc<dyn TokensRepo>) -> Arc<dyn MailReader> {
    Arc::new(GraphMailReader::new(graph_client(), tokens_repo))
}

fn calendar_reader(tokens_repo: Arc<dyn TokensRepo>) -> Arc<dyn CalendarReader> {
    Arc::new(GraphCalendarReader::new(graph_client(), tokens_repo))
}

fn mail_sender(tokens_repo: Arc<dyn TokensRepo>) -> Arc<dyn MailSender> {
    Arc::new(GraphMailSender::new(graph_client(), tokens_repo))
}

fn calendar_event_creator(tokens_repo: Arc<dyn TokensRepo>) -> Arc<dyn CalendarEventCreator> {
    Arc::new(GraphCalendarEventCreator::new(graph_client(), tokens_repo))
}

fn proactive_notifier() -> Arc<dyn ProactiveNotifier> {
    match std::env::var("TEAMS_ADAPTER_BASE_URL") {
        Ok(base_url) if !base_url.trim().is_empty() => Arc::new(HttpProactiveNotifier::new(base_url)),
        _ => Arc::new(NoopProactiveNotifier),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = SocketAddr::from(([0, 0, 0, 0], grpc_port()));

    let allowlist_domains = HashSet::from(["contoso.com".to_string()]);
    let known_recipients = HashSet::from(["james@contoso.com".to_string()]);
    let database = database().await;
    let cipher = token_cipher();
    let tokens_repo = tokens_repo(database.clone(), cipher);
    let mail_reader = mail_reader(tokens_repo.clone());
    let mail_sender = mail_sender(tokens_repo.clone());
    let calendar_reader = calendar_reader(tokens_repo.clone());
    let calendar_event_creator = calendar_event_creator(tokens_repo.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        allowlist_domains,
        known_recipients,
        sessions_repo(database.clone()),
        audit_repo(database.clone()),
        approvals_repo(database.clone()),
        conversation_refs_repo(database.clone()),
        tokens_repo,
        mail_reader,
        mail_sender,
        calendar_reader,
        calendar_event_creator,
        proactive_notifier(),
    ));

    let service = AgentGatewayService::new(orchestrator);

    println!("agent-core gRPC listening on {}", addr);

    Server::builder()
        .add_service(AgentGatewayServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
