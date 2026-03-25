# OPC - One Person Company

AI agent orchestration platform with human-in-the-loop approval. Inspired by [PaperClip](https://github.com/paperclipai/paperclip), built in Rust.

OPC orchestrates AI agents as employees in your company, but unlike fully autonomous systems, **every agent output requires human approval** before the next step proceeds. You stay in control while AI agents do the work.

## Build from Source

Requires [Rust](https://rustup.rs/) (1.75+).

```bash
git clone https://github.com/juntao/opc.git
cd opc
cargo build --release
```

The binary is produced at `target/release/opc-server`.

## Running

```bash
./target/release/opc-server
```

On first run, OPC will:
1. Download and start an embedded PostgreSQL instance (stored in `./db/` next to the binary)
2. Run database migrations
3. Create a default company and admin user

The server starts at **http://localhost:3100**. Log in with username `admin` and password `admin`.

## Configuration

OPC is configured via environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `3100` | HTTP server port |
| `PG_PORT` | `5433` | Port for the embedded PostgreSQL instance |
| `DATABASE_URL` | *(embedded)* | Set this to use an external PostgreSQL instead of the embedded one |
| `RUST_LOG` | `opc=info` | Log level filter (uses `tracing` / `env_filter` syntax) |

Examples:

```bash
# Run on a different port
PORT=8080 ./target/release/opc-server

# Use an external PostgreSQL database
DATABASE_URL=postgresql://user:pass@localhost:5432/opc ./target/release/opc-server

# Run with debug logging
RUST_LOG=opc=debug ./target/release/opc-server
```

## How Agents Work

Every agent in OPC follows the same lifecycle, regardless of adapter type:

```
1. You create an issue and assign it to an agent
2. OPC triggers the agent (via heartbeat)
3. The agent checks out the issue (atomic — no other agent can take it)
4. The agent does the work
5. The agent submits results → issue moves to "awaiting_approval"
6. You review in the approval queue:
   - Approve → issue is done, downstream agents are triggered
   - Request Changes → agent re-wakes with your feedback, re-works, re-submits
   - Reject → issue is cancelled
```

The key rule: **agents never see issues in `awaiting_approval` status.** They can only pick up `todo`, `approved`, or `changes_requested` issues. This ensures the human always gates the workflow.

### Adapter Types

OPC supports three adapter types. Each one handles the "do the work and submit results" step differently, but the human-in-the-loop approval flow is identical for all.

| Adapter | How It Works | Submit Mechanism |
|---------|-------------|-----------------|
| **Claude Code** | Spawns a local `claude` CLI process | OPC auto-submits the output |
| **OpenClaw** | Sends task to OpenClaw's webhook | OpenClaw calls back to OPC via curl |
| **HTTP Webhook** | POSTs task context to your URL | Your agent calls OPC's Agent API |

### Event-Driven Triggers

Agents are triggered automatically by system events:

- **Assignment** — you assign an issue to an agent
- **Approval** — you approve a parent task, waking agents on child tasks
- **Changes Requested** — you request changes, re-waking the assigned agent
- **Manual** — you click "Invoke" on an agent in the dashboard
- **Schedule** — periodic heartbeat (configurable)

## Connecting Agents

### Claude Code

OPC spawns a [Claude Code](https://docs.anthropic.com/en/docs/claude-code) CLI process with the task context as the prompt. Claude Code works locally on your machine, and OPC automatically submits its output for your approval.

```bash
curl -X POST http://localhost:3100/api/agents \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Claude Coder",
    "title": "Full-Stack Developer",
    "adapter_type": "claude_code",
    "adapter_config": {
      "working_dir": "/path/to/project",
      "model": "sonnet",
      "max_turns": 10
    }
  }'
```

Requires the `claude` CLI to be installed and authenticated on the machine running OPC.

**Config options:**

| Field | Required | Description |
|-------|----------|-------------|
| `working_dir` | No | Directory for Claude Code to work in |
| `model` | No | Model to use (`"sonnet"`, `"opus"`, etc.) |
| `max_turns` | No | Maximum turns for the session |

**Flow:** OPC builds a prompt from the issue → spawns `claude` CLI → captures output → auto-submits for approval. The entire cycle is synchronous — OPC waits for Claude Code to finish.

### OpenClaw

OPC sends the task to an [OpenClaw](https://openclaw.ai/) agent via its webhook API. OpenClaw processes the task, then calls back to OPC to submit results. No messaging channels are involved — OpenClaw works silently and submits directly back to OPC.

**Setup:**

1. Create the agent and generate an API key:

```bash
# Create the agent
curl -X POST http://localhost:3100/api/agents \
  -H "Content-Type: application/json" \
  -d '{
    "name": "OpenClaw Agent",
    "title": "Research Assistant",
    "adapter_type": "openclaw",
    "adapter_config": {
      "webhook_url": "http://127.0.0.1:18789/hooks/agent",
      "token": "your-openclaw-hooks-token",
      "opc_api_key": "PASTE_KEY_HERE"
    }
  }'

# Generate an API key for the agent
curl -X POST http://localhost:3100/api/agents/{agent_id}/keys
# Returns: {"api_key": "opc_abc123...", ...}
# Paste this key into the adapter_config.opc_api_key field
```

2. Update the agent config with the generated API key:

```bash
curl -X PUT http://localhost:3100/api/agents/{agent_id} \
  -H "Content-Type: application/json" \
  -d '{
    "adapter_config": {
      "webhook_url": "http://127.0.0.1:18789/hooks/agent",
      "token": "your-openclaw-hooks-token",
      "opc_api_key": "opc_abc123..."
    }
  }'
```

**Config options:**

| Field | Required | Description |
|-------|----------|-------------|
| `webhook_url` | Yes | OpenClaw's `/hooks/agent` endpoint |
| `token` | Yes | Bearer token for OpenClaw authentication |
| `opc_api_key` | Yes | OPC API key so OpenClaw can submit results back |
| `timeout_secs` | No | Timeout in seconds (default: 300) |
| `model` | No | Model override (e.g. `"anthropic/claude-sonnet-4-6"`) |
| `deliver` | No | Also post to a messaging channel (default: `false`) |
| `channel` | No | Target channel if `deliver` is `true` (e.g. `"slack"`) |
| `to` | No | Recipient if `deliver` is `true` (e.g. `"#general"`) |

**Flow:** OPC sends the task prompt to OpenClaw's webhook with `deliver: false` → OpenClaw processes the task silently → OpenClaw runs a curl command (embedded in the prompt) to submit results back to OPC → issue moves to `awaiting_approval`. The prompt includes the issue ID, API key, and exact curl command that OpenClaw needs to call back.

### HTTP Webhook

For custom agents, OPC POSTs the full task context to your webhook URL. Your agent processes the task and calls back to OPC's Agent API to submit results.

```bash
curl -X POST http://localhost:3100/api/agents \
  -H "Content-Type: application/json" \
  -d '{
    "name": "My Agent",
    "title": "Backend Developer",
    "adapter_type": "http",
    "adapter_config": {
      "webhook_url": "https://your-agent.example.com/webhook",
      "timeout_secs": 300,
      "headers": {"X-Custom-Header": "value"}
    }
  }'
```

**Config options:**

| Field | Required | Description |
|-------|----------|-------------|
| `webhook_url` | Yes | URL to POST the task context to |
| `timeout_secs` | No | HTTP timeout in seconds (default: 300) |
| `headers` | No | Custom headers to include in the request |

**Webhook payload:** OPC POSTs the following JSON to your webhook:

```json
{
  "agent": { "id": "...", "name": "My Agent" },
  "issue": { "id": "...", "title": "Fix the login bug", "description": "..." },
  "comments": [{ "author_name": "admin", "body": "Check the logout flow too" }],
  "parent_chain": [],
  "trigger": "assignment",
  "api_base_url": "http://localhost:3100",
  "api_key": ""
}
```

Your agent should return an `AgentResponse` JSON body:

```json
{
  "status": "NeedsApproval",
  "summary": "Fixed the login bug by correcting session validation",
  "artifacts": [],
  "cost": null
}
```

OPC auto-submits the response for approval. Alternatively, your agent can use the Agent API directly (see below) for more control — post comments, report costs, and submit when ready.

### Generate an API Key

After creating an agent, generate an API key for it. This key is shown **once** — save it.

```bash
curl -X POST http://localhost:3100/api/agents/{agent_id}/keys
```

Response:

```json
{
  "api_key": "opc_abc123...",
  "prefix": "abc12345",
  "note": "Save this key - it will not be shown again"
}
```

## Agent API

External agents authenticate with their API key (`Authorization: Bearer opc_...`) and use these endpoints:

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/agent/me` | `GET` | Get the agent's own identity |
| `/api/agent/assignments` | `GET` | List assigned issues the agent can pick up |
| `/api/agent/issues/{id}/checkout` | `POST` | Atomically check out a task (prevents other agents from taking it) |
| `/api/agent/issues/{id}/checkin` | `POST` | Release a checked-out task without submitting |
| `/api/agent/issues/{id}/submit` | `POST` | Submit completed work for human approval |
| `/api/agent/issues/{id}/comments` | `GET` | Read the comment thread (including human feedback) |
| `/api/agent/issues/{id}/comments` | `POST` | Post a comment on the issue |

### Typical Agent Workflow

```bash
API="http://localhost:3100"
KEY="opc_abc123..."

# 1. Check for assigned work
curl -H "Authorization: Bearer $KEY" $API/api/agent/assignments

# 2. Check out a task
curl -X POST -H "Authorization: Bearer $KEY" $API/api/agent/issues/{id}/checkout

# 3. (Do the work...)

# 4. Post a progress comment
curl -X POST -H "Authorization: Bearer $KEY" \
  -H "Content-Type: application/json" \
  -d '{"body": "Fixed the bug in auth.rs, running tests now..."}' \
  $API/api/agent/issues/{id}/comments

# 5. Submit work for approval
curl -X POST -H "Authorization: Bearer $KEY" \
  -H "Content-Type: application/json" \
  -d '{"summary": "Fixed login bug by correcting session validation logic", "artifacts": null}' \
  $API/api/agent/issues/{id}/submit
```

After submitting, the issue moves to `awaiting_approval` and appears in the dashboard approval queue.

## Dashboard

Open **http://localhost:3100** in your browser and log in (`admin` / `admin`).

### Pages

| Page | URL | What You Do There |
|------|-----|-------------------|
| **Dashboard** | `/` | Overview of agent count, active issues, pending approvals, and recent activity |
| **Agents** | `/agents` | View all agents, their status, and quick actions (pause, resume, invoke) |
| **Agent Detail** | `/agents/{id}` | See an agent's config, budget, current assignments, and heartbeat history. Generate API keys |
| **Issues** | `/issues` | List all issues. Filter by status (todo, in progress, awaiting approval, done) |
| **Issue Detail** | `/issues/{id}` | View issue details, comment thread, sub-tasks, and inline approval widget |
| **Approval Queue** | `/approvals` | Review all pending agent submissions. Approve, request changes, or reject |
| **Approval Detail** | `/approvals/{id}` | Full review page with the agent's summary, original task, conversation thread, and action buttons |
| **Projects** | `/projects` | Organize issues into projects |

### Reviewing and Approving Agent Work

This is the core workflow:

1. **An agent submits work** — the issue appears in the **Approval Queue** (`/approvals`) with a notification via SSE
2. **Open the approval** to see:
   - The agent's summary of what was done
   - The original task description
   - The full comment thread between you and the agent
3. **Take action**:
   - **Approve** — the issue moves to `approved`. If there are downstream tasks assigned to other agents, those agents are automatically triggered
   - **Request Changes** — write feedback in the text box. The issue moves to `changes_requested` and the agent is re-triggered with your feedback visible in the comment thread. The agent re-works and re-submits, bringing it back to the approval queue
   - **Reject** — the issue is cancelled

### Chatting with Agents

You can communicate with agents through issue comments at any point:

- On the **Issue Detail** page (`/issues/{id}`), use the comment box to post messages
- Agents see your comments when they next wake up (via assignment, approval, or heartbeat)
- Comments from agents and humans are shown together in a threaded conversation
- When you **Request Changes**, your feedback is posted as a comment so the agent sees the full context

### Creating and Assigning Work

1. Go to **Issues** > **New Issue**
2. Fill in the title, description, and priority
3. Assign it to an agent and optionally link it to a project
4. The assigned agent is automatically triggered to pick up the task

You can also create parent-child task hierarchies — when a parent task is approved, agents assigned to child tasks are automatically woken up.

## License

MIT
