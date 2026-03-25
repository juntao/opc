# OPC - One Person Company

## Purpose

OPC is an AI agent orchestration platform inspired by [PaperClip](https://github.com/paperclipai/paperclip). While PaperClip orchestrates agents autonomously in "zero-human companies," OPC enforces **human-in-the-loop approval** at every task transition. A single human operator stays in control while AI agents do the work.

## Goal

Build a Rust-based platform where:
- AI agents are assigned tickets and work on them
- Every agent output requires human approval before the next step proceeds
- Humans can chat with agents, request changes, and only approve when satisfied
- Agents connect via HTTP webhooks or Claude Code CLI
- The system is event-driven with ticket-based work management

## Architecture

```
opc/
├── crates/
│   ├── opc-server/    # Axum HTTP server, routes, HTMX/Askama templates, SSE
│   ├── opc-db/        # PostgreSQL queries, migrations, embedded PG (pg-embed)
│   ├── opc-core/      # Domain types, business logic, event bus
│   ├── opc-agents/    # Agent adapters (HTTP webhook, Claude Code), heartbeat
│   └── opc-cli/       # CLI management tool
├── migrations/        # SQL schema migrations (001_initial.sql)
├── static/            # HTMX, CSS
└── templates/         # Askama HTML templates
```

### Key Design Decisions

- **Frontend**: HTMX + Askama templates (server-rendered, no JS build step)
- **Database**: Embedded PostgreSQL via `pg-embed` (PG_V15), zero-setup local dev
- **Queries**: Runtime `sqlx::query_as::<_, T>()` (NOT compile-time macros, since no DB at build time)
- **Events**: `tokio::sync::broadcast` internal event bus
- **Real-time**: Server-Sent Events (SSE) for live UI updates
- **Auth**: Argon2 password hashing for both board user passwords and agent API keys

### Core Flow (Human-in-the-Loop)

```
Issue created → assigned to agent → agent checks out (atomic) → agent works
→ agent submits → status: awaiting_approval → human reviews in approval queue
→ Approve: next agent can proceed
→ Request Changes: agent re-wakes with feedback, re-works, re-submits
→ Reject: task cancelled
```

Agents calling `GET /api/agent/assignments` ONLY see issues with status `todo`, `approved`, or `changes_requested` — never `awaiting_approval`.

### Database Schema

10 tables defined in `migrations/001_initial.sql`:
companies, board_users, agents, agent_api_keys, projects, issues, issue_comments, approval_requests, heartbeat_runs, cost_events, activity_log

## Running

```bash
cargo run -p opc-server
# Server starts at http://localhost:3100
# Login: admin / admin
# First run downloads embedded PostgreSQL
```

## Development Rules

### Before Committing

Both of these MUST pass with zero warnings/errors:
```bash
cargo fmt --check
cargo clippy
```

### Branching Strategy

- **Small features/fixes**: Commit and push directly to `main`
- **Large features or refactoring** (anything requiring plan mode): Create a branch and raise a PR first

### Code Conventions

- Domain enums use `as_str()` to serialize and `parse()` to deserialize (not `from_str` to avoid clippy `should_implement_trait` warning)
- All DB-mapped structs derive `sqlx::FromRow`
- Askama templates cannot use `&` in patterns, `.as_deref()`, or `|truncate()` — use `{% match %}` blocks for Option fields
- Template struct fields that hold `Option<String>` for filter params should be plain `String` (empty = no filter)

## Environment Variables

| Var | Default | Description |
|-----|---------|-------------|
| `PORT` | `3100` | Server port |
| `PG_PORT` | `5433` | Embedded PostgreSQL port |
| `DATABASE_URL` | (embedded) | External PostgreSQL URL (skips pg-embed) |

## Implementation Status

### Complete
- Cargo workspace with 5 crates
- Full database schema with migrations
- Domain types, services, event bus (opc-core)
- Embedded PostgreSQL setup (opc-db)
- All query modules with runtime sqlx (opc-db)
- Agent adapters: HTTP webhook + Claude Code (opc-agents)
- Heartbeat system with event-driven triggers (opc-agents)
- Axum server with auth middleware, SSE (opc-server)
- HTMX + Askama UI: dashboard, agents, issues, approvals, projects
- Approval queue with approve/request-changes/reject flow

### Not Yet Implemented
- Cost dashboard and budget enforcement UI
- Kanban board drag-and-drop view
- CLI tool (skeleton only)
- Comprehensive test suite
- Agent @-mention detection in comments
