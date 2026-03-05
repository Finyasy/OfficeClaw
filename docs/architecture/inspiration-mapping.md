# Inspiration and Best-Practice Mapping

## OpenClaw-inspired practices adopted

- Gateway-style architecture with clear connector boundaries.
- Session-aware orchestration and persistent context handling.
- Tool/skill-centric execution model with explicit policy checks.
- Cron/event-ready design for proactive agent actions.

## NanoClaw-inspired practices adopted

- Isolation mindset for risky actions and external calls.
- Channel adapter separation from core decision engine.
- Guardrail-first approach for side-effectful operations.

## Google Workspace CLI inspired practices adopted

- Contract-first and typed command surface.
- Extensible capability design with curated safe subset for MVP.
- Avoid exposing broad dynamic APIs without guardrails.

## Final architecture decision

- Build order for production: proto contracts and Rust core skeleton first, then TS adapter.
- Rationale: stable internal contracts reduce channel-coupling and future rework.
