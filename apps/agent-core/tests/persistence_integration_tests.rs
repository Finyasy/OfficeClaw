use std::sync::Arc;

use agent_core::crypto::envelope::{
    KeyVaultConfig, KeyVaultCredential, KeyVaultEnvelopeCipher,
};
use agent_core::domain::{
    ActivityEnvelope, Actor, ApprovalKind, AuditEventRecord, Conversation, ConversationRefRecord,
    CreateEventApprovalPayload, MailApprovalPayload, OAuthTokenBundle, SessionKey, SessionState,
};
use agent_core::policy::state_machine::ApprovalStatus;
use agent_core::skills::graph::calendar::{
    CalendarEventCreator, CalendarReader, GraphCalendarEventCreator, GraphCalendarReader,
};
use agent_core::skills::graph::client::{GraphClient, GraphClientConfig};
use agent_core::skills::graph::mail::{GraphMailReader, GraphMailSender, MailReader, MailSender};
use agent_core::storage::approvals_repo::{ApprovalsRepo, PostgresApprovalsRepo};
use agent_core::storage::audit_repo::{AuditRepo, PostgresAuditRepo};
use agent_core::storage::conversation_refs_repo::{
    ConversationRefsRepo, PostgresConversationRefsRepo,
};
use agent_core::storage::db::{Database, DatabaseConfig};
use agent_core::storage::migrations;
use agent_core::storage::sessions_repo::{PostgresSessionsRepo, SessionsRepo};
use agent_core::storage::tokens_repo::{PostgresTokensRepo, TokensRepo};
use serde_json::{json, Value};
use uuid::Uuid;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

fn test_database_url() -> Option<String> {
    std::env::var("TEST_DATABASE_URL").ok().filter(|value| !value.trim().is_empty())
}

async fn database_or_skip() -> Option<Database> {
    let Some(url) = test_database_url() else {
        eprintln!("skipping persistence integration test: TEST_DATABASE_URL not set");
        return None;
    };

    let database = Database::connect(&DatabaseConfig { url }).await.unwrap();
    migrations::run(&database).await.unwrap();
    Some(database)
}

fn actor(id: &str) -> Actor {
    Actor {
        tenant_id: format!("tenant-{}", id),
        user_id: format!("user-{}", id),
    }
}

fn activity(actor: &Actor, conversation_id: &str) -> ActivityEnvelope {
    ActivityEnvelope {
        actor: actor.clone(),
        conversation: Conversation {
            channel: "teams".to_string(),
            conversation_id: conversation_id.to_string(),
            message_id: "msg-1".to_string(),
        },
        text: "reply to this email".to_string(),
        attachments: vec![],
        action: None,
        action_payload_json: None,
        recipients: vec!["james@contoso.com".to_string()],
        attendee_email: Some("james@contoso.com".to_string()),
        attendee_known: true,
        contains_sensitive: false,
        request_hour_local: 10,
        conversation_ref_json: Some(
            json!({
                "serviceUrl": "https://smba.trafficmanager.net/teams/",
                "conversation": { "id": conversation_id }
            })
            .to_string(),
        ),
    }
}

#[derive(Clone)]
struct EchoWrapResponder {
    vault_uri: String,
    kek_name: String,
    key_version: String,
}

impl Respond for EchoWrapResponder {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let payload: Value = serde_json::from_slice(&request.body).unwrap();
        let wrapped_value = payload
            .get("value")
            .and_then(|value| value.as_str())
            .unwrap_or_default();

        ResponseTemplate::new(200).set_body_json(json!({
            "value": wrapped_value,
            "kid": format!("{}/keys/{}/{}", self.vault_uri, self.kek_name, self.key_version)
        }))
    }
}

#[derive(Clone)]
struct EchoUnwrapResponder;

impl Respond for EchoUnwrapResponder {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let payload: Value = serde_json::from_slice(&request.body).unwrap();
        let wrapped_value = payload
            .get("value")
            .and_then(|value| value.as_str())
            .unwrap_or_default();

        ResponseTemplate::new(200).set_body_json(json!({
            "value": wrapped_value
        }))
    }
}

#[tokio::test]
async fn postgres_repos_and_graph_clients_round_trip_with_mocked_keyvault_and_graph() {
    let Some(database) = database_or_skip().await else {
        return;
    };

    let run_id = Uuid::new_v4().to_string();
    let actor = actor(&run_id);
    let activity = activity(&actor, &format!("conv-{}", run_id));

    let key_vault_server = MockServer::start().await;
    let graph_server = MockServer::start().await;
    let key_version = "test-key-version";
    let kek_name = "teams-agent-kek";

    Mock::given(method("POST"))
        .and(path(format!("/keys/{}/wrapkey", kek_name)))
        .and(header("authorization", "Bearer keyvault-token"))
        .respond_with(EchoWrapResponder {
            vault_uri: key_vault_server.uri(),
            kek_name: kek_name.to_string(),
            key_version: key_version.to_string(),
        })
        .mount(&key_vault_server)
        .await;

    Mock::given(method("POST"))
        .and(path(format!("/keys/{}/{}/unwrapkey", kek_name, key_version)))
        .and(header("authorization", "Bearer keyvault-token"))
        .respond_with(EchoUnwrapResponder)
        .mount(&key_vault_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/v1.0/me/mailFolders/inbox/messages"))
        .and(header("authorization", "Bearer access-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "value": [
                {
                    "id": "mail-1",
                    "subject": "Budget review",
                    "receivedDateTime": "2026-03-06T08:00:00Z",
                    "from": {
                        "emailAddress": {
                            "name": "James",
                            "address": "james@contoso.com"
                        }
                    }
                }
            ]
        })))
        .mount(&graph_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/v1.0/me/calendarView"))
        .and(header("authorization", "Bearer access-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "value": [
                {
                    "start": { "dateTime": "2026-03-09T09:00:00Z" },
                    "end": { "dateTime": "2026-03-09T09:30:00Z" }
                }
            ]
        })))
        .mount(&graph_server)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1.0/me/sendMail"))
        .and(header("authorization", "Bearer access-token"))
        .respond_with(ResponseTemplate::new(202))
        .mount(&graph_server)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1.0/me/events"))
        .and(header("authorization", "Bearer access-token"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "event-123"
        })))
        .mount(&graph_server)
        .await;

    let cipher = Arc::new(KeyVaultEnvelopeCipher::new(
        KeyVaultConfig {
            vault_uri: key_vault_server.uri(),
            kek_name: kek_name.to_string(),
            api_version: "7.4".to_string(),
        },
        KeyVaultCredential::StaticToken("keyvault-token".to_string()),
    ));
    let tokens_repo = Arc::new(PostgresTokensRepo::new(database.clone(), cipher.clone()));
    let sessions_repo = PostgresSessionsRepo::new(database.clone());
    let approvals_repo = PostgresApprovalsRepo::new(database.clone());
    let conversation_refs_repo = PostgresConversationRefsRepo::new(database.clone());
    let audit_repo = PostgresAuditRepo::new(database.clone());

    let token = OAuthTokenBundle {
        access_token: "access-token".to_string(),
        refresh_token: Some("refresh-token".to_string()),
        expires_at_utc: Some("2026-03-06T12:00:00Z".to_string()),
        scope: Some("Mail.Read Calendars.Read Mail.Send Calendars.ReadWrite".to_string()),
    };
    tokens_repo.store_graph_token(&actor, &token).await.unwrap();
    let loaded_token = tokens_repo.load_graph_token(&actor).await.unwrap().unwrap();
    assert_eq!(loaded_token, token);

    let session_key = SessionKey::from_activity(&activity);
    let session_state = SessionState {
        last_intent: Some("schedule_meeting".to_string()),
        unread_summary_count: Some(1),
        proposed_slots: vec![],
    };
    sessions_repo.upsert(&session_key, &session_state).await.unwrap();
    assert_eq!(sessions_repo.load(&session_key).await.unwrap(), Some(session_state));

    let conversation_ref = ConversationRefRecord::from_activity(&activity).unwrap();
    conversation_refs_repo.upsert(&conversation_ref).await.unwrap();
    let loaded_ref = conversation_refs_repo
        .load(
            &actor.tenant_id,
            &actor.user_id,
            &activity.conversation.channel,
            &activity.conversation.conversation_id,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded_ref, conversation_ref);

    let approval = approvals_repo
        .create(
            &activity,
            ApprovalKind::SendMail,
            "LOW",
            serde_json::to_value(MailApprovalPayload {
                recipients: vec!["james@contoso.com".to_string()],
                draft_text: "Draft reply prepared.".to_string(),
                contains_sensitive: false,
            })
            .unwrap(),
            json!({ "policy": "EXPLICIT_APPROVAL_REQUIRED" }),
        )
        .await
        .unwrap();
    let loaded_approval = approvals_repo
        .load(approval.approval_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded_approval.status, "PENDING");
    let updated_approval = approvals_repo
        .update_status(approval.approval_id, ApprovalStatus::Approved)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated_approval.status, "APPROVED");

    let audit_event = AuditEventRecord {
        event_id: Uuid::new_v4(),
        tenant_id: actor.tenant_id.clone(),
        user_id: actor.user_id.clone(),
        channel: "teams".to_string(),
        conversation_id: activity.conversation.conversation_id.clone(),
        correlation_id: format!("corr-{}", run_id),
        event_type: "INTEGRATION_TEST".to_string(),
        event_json: json!({ "run_id": run_id }),
    };
    audit_repo.append(audit_event.clone()).await.unwrap();
    let stored_events = audit_repo.list().await.unwrap();
    assert!(stored_events.iter().any(|event| event.event_id == audit_event.event_id));

    let graph_client = GraphClient::new(GraphClientConfig {
        base_url: format!("{}/v1.0", graph_server.uri()),
    });
    let mail_reader = GraphMailReader::new(graph_client.clone(), tokens_repo.clone());
    let mail_sender = GraphMailSender::new(graph_client.clone(), tokens_repo.clone());
    let calendar_reader = GraphCalendarReader::new(graph_client.clone(), tokens_repo.clone());
    let calendar_event_creator = GraphCalendarEventCreator::new(graph_client, tokens_repo);

    let messages = mail_reader.list_unread_today(&actor).await.unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].subject, "Budget review");

    let slots = calendar_reader.propose_slots_next_week(&actor, 30).await.unwrap();
    assert!(!slots.is_empty());

    mail_sender
        .send_draft(
            &actor,
            &MailApprovalPayload {
                recipients: vec!["james@contoso.com".to_string()],
                draft_text: "Draft reply prepared.".to_string(),
                contains_sensitive: false,
            },
        )
        .await
        .unwrap();

    let event_id = calendar_event_creator
        .create_event(
            &actor,
            &CreateEventApprovalPayload {
                slot_index: 0,
                start_utc: "2026-03-09T09:30:00Z".to_string(),
                end_utc: "2026-03-09T10:00:00Z".to_string(),
                attendee_email: Some("james@contoso.com".to_string()),
                request_hour_local: 10,
            },
        )
        .await
        .unwrap();
    assert_eq!(event_id, "event-123");
}
