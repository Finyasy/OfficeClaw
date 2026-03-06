use std::cmp::Ordering;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Datelike, Duration, NaiveDateTime, TimeZone, Utc, Weekday};
use serde::{Deserialize, Serialize};

use crate::domain::{Actor, CreateEventApprovalPayload, ProposedSlot};
use crate::skills::graph::client::{GraphClient, GraphClientError};
use crate::storage::tokens_repo::TokensRepo;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalendarReadError {
    Retryable(String),
    Permanent(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalendarWriteError {
    Retryable(String),
    Permanent(String),
}

#[async_trait]
pub trait CalendarReader: Send + Sync {
    async fn propose_slots_next_week(
        &self,
        actor: &Actor,
        duration_minutes: u32,
    ) -> Result<Vec<ProposedSlot>, CalendarReadError>;
}

#[async_trait]
pub trait CalendarEventCreator: Send + Sync {
    async fn create_event(
        &self,
        actor: &Actor,
        event: &CreateEventApprovalPayload,
    ) -> Result<String, CalendarWriteError>;
}

#[derive(Clone)]
pub struct GraphCalendarReader {
    client: GraphClient,
    tokens_repo: Arc<dyn TokensRepo>,
}

impl GraphCalendarReader {
    pub fn new(client: GraphClient, tokens_repo: Arc<dyn TokensRepo>) -> Self {
        Self {
            client,
            tokens_repo,
        }
    }
}

#[async_trait]
impl CalendarReader for GraphCalendarReader {
    async fn propose_slots_next_week(
        &self,
        actor: &Actor,
        duration_minutes: u32,
    ) -> Result<Vec<ProposedSlot>, CalendarReadError> {
        let token = self
            .tokens_repo
            .load_graph_token(actor)
            .await
            .map_err(|error| CalendarReadError::Permanent(error.message))?
            .ok_or_else(|| CalendarReadError::Permanent("Graph token unavailable".to_string()))?;

        let now = Utc::now();
        let (window_start, window_end) = next_week_window(now);
        let path = calendar_view_path(window_start, window_end);
        let response: GraphCalendarViewResponse = self
            .client
            .get_json(&path, &token.access_token)
            .await
            .map_err(map_graph_error)?;
        let events = response
            .value
            .into_iter()
            .filter_map(|event| BusyInterval::from_graph(event).ok())
            .collect::<Vec<_>>();

        Ok(propose_slots_from_events(
            now,
            &events,
            duration_minutes as i64,
            3,
        ))
    }
}

#[derive(Clone)]
pub struct GraphCalendarEventCreator {
    client: GraphClient,
    tokens_repo: Arc<dyn TokensRepo>,
}

impl GraphCalendarEventCreator {
    pub fn new(client: GraphClient, tokens_repo: Arc<dyn TokensRepo>) -> Self {
        Self {
            client,
            tokens_repo,
        }
    }
}

#[async_trait]
impl CalendarEventCreator for GraphCalendarEventCreator {
    async fn create_event(
        &self,
        actor: &Actor,
        event: &CreateEventApprovalPayload,
    ) -> Result<String, CalendarWriteError> {
        let token = self
            .tokens_repo
            .load_graph_token(actor)
            .await
            .map_err(|error| CalendarWriteError::Permanent(error.message))?
            .ok_or_else(|| CalendarWriteError::Permanent("Graph token unavailable".to_string()))?;
        let attendee_email =
            event
                .attendee_email
                .clone()
                .ok_or_else(|| CalendarWriteError::Permanent(
                    "Attendee email unavailable for event creation".to_string(),
                ))?;

        let request = GraphCreateEventRequest {
            subject: "Meeting scheduled by OfficeClaw".to_string(),
            start: GraphEventDateTime {
                date_time: event.start_utc.clone(),
                time_zone: "UTC".to_string(),
            },
            end: GraphEventDateTime {
                date_time: event.end_utc.clone(),
                time_zone: "UTC".to_string(),
            },
            attendees: vec![GraphEventAttendee {
                email_address: GraphEventEmailAddress {
                    address: attendee_email,
                },
                r#type: "required".to_string(),
            }],
        };

        let response: GraphCreateEventResponse = self
            .client
            .post_json("/me/events", &token.access_token, &request)
            .await
            .map_err(map_graph_write_error)?;

        Ok(response.id)
    }
}

#[derive(Clone)]
pub struct StaticCalendarReader {
    result: Result<Vec<ProposedSlot>, CalendarReadError>,
}

impl StaticCalendarReader {
    pub fn succeed(slots: Vec<ProposedSlot>) -> Self {
        Self { result: Ok(slots) }
    }

    pub fn fail(error: CalendarReadError) -> Self {
        Self { result: Err(error) }
    }
}

#[async_trait]
impl CalendarReader for StaticCalendarReader {
    async fn propose_slots_next_week(
        &self,
        _actor: &Actor,
        _duration_minutes: u32,
    ) -> Result<Vec<ProposedSlot>, CalendarReadError> {
        self.result.clone()
    }
}

#[derive(Clone)]
pub struct StaticCalendarEventCreator {
    result: Result<String, CalendarWriteError>,
}

impl StaticCalendarEventCreator {
    pub fn succeed(event_id: impl Into<String>) -> Self {
        Self {
            result: Ok(event_id.into()),
        }
    }

    pub fn fail(error: CalendarWriteError) -> Self {
        Self { result: Err(error) }
    }
}

#[async_trait]
impl CalendarEventCreator for StaticCalendarEventCreator {
    async fn create_event(
        &self,
        _actor: &Actor,
        _event: &CreateEventApprovalPayload,
    ) -> Result<String, CalendarWriteError> {
        self.result.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BusyInterval {
    start: DateTime<Utc>,
    end: DateTime<Utc>,
}

impl BusyInterval {
    fn from_graph(event: GraphCalendarEvent) -> Result<Self, CalendarReadError> {
        let start = parse_graph_datetime(&event.start.date_time)?;
        let end = parse_graph_datetime(&event.end.date_time)?;

        Ok(Self { start, end })
    }
}

fn calendar_view_path(window_start: DateTime<Utc>, window_end: DateTime<Utc>) -> String {
    format!(
        "/me/calendarView?startDateTime={}&endDateTime={}&$select=start,end&$orderby=start/dateTime",
        window_start.format("%Y-%m-%dT%H:%M:%SZ"),
        window_end.format("%Y-%m-%dT%H:%M:%SZ")
    )
}

fn next_week_window(now: DateTime<Utc>) -> (DateTime<Utc>, DateTime<Utc>) {
    let next_day = now + Duration::days(1);
    let start = Utc
        .with_ymd_and_hms(next_day.year(), next_day.month(), next_day.day(), 0, 0, 0)
        .single()
        .unwrap_or(next_day);
    let end = start + Duration::days(7);
    (start, end)
}

fn propose_slots_from_events(
    now: DateTime<Utc>,
    events: &[BusyInterval],
    duration_minutes: i64,
    max_slots: usize,
) -> Vec<ProposedSlot> {
    let duration = Duration::minutes(duration_minutes);
    let mut slots = Vec::new();
    let mut busy = events.to_vec();
    busy.sort_by(|left, right| {
        if left.start == right.start {
            Ordering::Equal
        } else if left.start < right.start {
            Ordering::Less
        } else {
            Ordering::Greater
        }
    });

    let start_day = (now + Duration::days(1)).date_naive();
    for day_offset in 0..7 {
        let day = start_day + Duration::days(day_offset);
        if matches!(day.weekday(), Weekday::Sat | Weekday::Sun) {
            continue;
        }

        let work_start = Utc
            .with_ymd_and_hms(day.year(), day.month(), day.day(), 9, 0, 0)
            .single()
            .unwrap();
        let work_end = Utc
            .with_ymd_and_hms(day.year(), day.month(), day.day(), 17, 0, 0)
            .single()
            .unwrap();

        let mut cursor = work_start;
        while cursor + duration <= work_end && slots.len() < max_slots {
            let candidate_end = cursor + duration;
            if busy
                .iter()
                .all(|interval| interval.end <= cursor || interval.start >= candidate_end)
            {
                slots.push(ProposedSlot {
                    start_utc: cursor.to_rfc3339(),
                    end_utc: candidate_end.to_rfc3339(),
                });
            }
            cursor += Duration::minutes(30);
        }

        if slots.len() == max_slots {
            break;
        }
    }

    slots
}

fn parse_graph_datetime(value: &str) -> Result<DateTime<Utc>, CalendarReadError> {
    if let Ok(parsed) = DateTime::parse_from_rfc3339(value) {
        return Ok(parsed.with_timezone(&Utc));
    }

    let parsed = NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f")
        .map_err(|error| CalendarReadError::Permanent(error.to_string()))?;
    Ok(Utc.from_utc_datetime(&parsed))
}

fn map_graph_error(error: GraphClientError) -> CalendarReadError {
    if error.retryable {
        return CalendarReadError::Retryable(error.message);
    }

    CalendarReadError::Permanent(error.message)
}

fn map_graph_write_error(error: GraphClientError) -> CalendarWriteError {
    if error.retryable {
        return CalendarWriteError::Retryable(error.message);
    }

    CalendarWriteError::Permanent(error.message)
}

#[derive(Debug, Deserialize)]
struct GraphCalendarViewResponse {
    value: Vec<GraphCalendarEvent>,
}

#[derive(Debug, Deserialize)]
struct GraphCalendarEvent {
    start: GraphDateTimeValue,
    end: GraphDateTimeValue,
}

#[derive(Debug, Deserialize)]
struct GraphDateTimeValue {
    #[serde(rename = "dateTime")]
    date_time: String,
}

#[derive(Debug, Serialize)]
struct GraphCreateEventRequest {
    subject: String,
    start: GraphEventDateTime,
    end: GraphEventDateTime,
    attendees: Vec<GraphEventAttendee>,
}

#[derive(Debug, Serialize)]
struct GraphEventDateTime {
    #[serde(rename = "dateTime")]
    date_time: String,
    #[serde(rename = "timeZone")]
    time_zone: String,
}

#[derive(Debug, Serialize)]
struct GraphEventAttendee {
    #[serde(rename = "emailAddress")]
    email_address: GraphEventEmailAddress,
    #[serde(rename = "type")]
    r#type: String,
}

#[derive(Debug, Serialize)]
struct GraphEventEmailAddress {
    address: String,
}

#[derive(Debug, Deserialize)]
struct GraphCreateEventResponse {
    id: String,
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::{
        calendar_view_path, parse_graph_datetime, propose_slots_from_events, BusyInterval,
        CalendarReadError, CalendarReader, StaticCalendarReader,
    };
    use crate::domain::{Actor, ProposedSlot};

    #[test]
    fn calendar_view_path_contains_window_bounds() {
        let start = Utc.with_ymd_and_hms(2026, 3, 9, 0, 0, 0).single().unwrap();
        let end = Utc.with_ymd_and_hms(2026, 3, 16, 0, 0, 0).single().unwrap();

        let path = calendar_view_path(start, end);

        assert!(path.contains("startDateTime=2026-03-09T00:00:00Z"));
        assert!(path.contains("endDateTime=2026-03-16T00:00:00Z"));
    }

    #[test]
    fn propose_slots_skips_busy_intervals() {
        let now = Utc.with_ymd_and_hms(2026, 3, 6, 12, 0, 0).single().unwrap();
        let busy = vec![BusyInterval {
            start: Utc.with_ymd_and_hms(2026, 3, 9, 9, 0, 0).single().unwrap(),
            end: Utc.with_ymd_and_hms(2026, 3, 9, 9, 30, 0).single().unwrap(),
        }];

        let slots = propose_slots_from_events(now, &busy, 30, 3);

        assert_eq!(slots.len(), 3);
        assert_eq!(slots[0].start_utc, "2026-03-09T09:30:00+00:00");
        assert_eq!(slots[1].start_utc, "2026-03-09T10:00:00+00:00");
    }

    #[test]
    fn parse_graph_datetime_supports_naive_utc_values() {
        let parsed = parse_graph_datetime("2026-03-09T09:00:00.0000000").unwrap();
        assert_eq!(
            parsed,
            Utc.with_ymd_and_hms(2026, 3, 9, 9, 0, 0).single().unwrap()
        );
    }

    #[tokio::test]
    async fn static_calendar_reader_returns_configured_failure() {
        let reader =
            StaticCalendarReader::fail(CalendarReadError::Retryable("graph busy".to_string()));
        let actor = Actor {
            tenant_id: "tenant-1".to_string(),
            user_id: "user-1".to_string(),
        };

        let result = reader.propose_slots_next_week(&actor, 30).await;
        assert_eq!(
            result,
            Err(CalendarReadError::Retryable("graph busy".to_string()))
        );
    }

    #[tokio::test]
    async fn static_calendar_reader_returns_configured_slots() {
        let slots = vec![ProposedSlot {
            start_utc: "2026-03-09T09:30:00+00:00".to_string(),
            end_utc: "2026-03-09T10:00:00+00:00".to_string(),
        }];
        let reader = StaticCalendarReader::succeed(slots.clone());
        let actor = Actor {
            tenant_id: "tenant-1".to_string(),
            user_id: "user-1".to_string(),
        };

        let result = reader.propose_slots_next_week(&actor, 30).await.unwrap();
        assert_eq!(result, slots);
    }
}
