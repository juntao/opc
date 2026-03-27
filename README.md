# OPC - One Person Company

AI agent orchestration platform with human-in-the-loop approval. Inspired by [PaperClip](https://github.com/paperclipai/paperclip), built in Rust.

OPC orchestrates AI agents as employees in your company, but unlike fully autonomous systems, **every agent output requires human approval** before the next step proceeds. You stay in control while AI agents do the work.

## Quick Start

This guide walks you through the full workflow: set up agents, plan a project, and let agents execute with your approval at every step.

### Prerequisites

#### 1. Add agents to OPC

Before you can assign work, you need agents in the system. Open the OPC dashboard at **http://localhost:3100** (login: `admin` / `admin`) and go to **Agents** > **+ New Agent**.

For each agent, fill in:

- **Name** — a unique name (e.g. "Alice", "Bob")
- **Title** — their job title (e.g. "Frontend Developer", "Copywriter")
- **Role** — their role description
- **Capabilities** — what this agent can do (the planning skill uses this to decide which agent gets which task)
- **Adapter type** — how the agent does work:
  - **Claude Code** — runs the `claude` CLI locally (requires `claude` installed on the OPC machine)
  - **OpenClaw** — sends tasks to an [OpenClaw](https://openclaw.ai/) agent via webhook
  - **HTTP Webhook** — POSTs task context to your custom URL
- **Adapter config** — JSON configuration specific to the adapter type (see [Connecting Agents](#connecting-agents) for details)

Add as many agents as you need. Each agent should have a clear specialty — the planning AI uses their title, role, and capabilities to match agents to tasks.

#### 2. Install the planning skill on OpenClaw

The planning skill lets you describe a project in natural language and have an OpenClaw agent automatically create the project, issues, and agent assignments in OPC.

In your OpenClaw agent's home directory, create a skill folder and download the skill file:

```bash
mkdir -p skills/planning
curl -o skills/planning/SKILL.md https://raw.githubusercontent.com/juntao/opc/main/skills/planning/SKILL.md
```

The agent also needs an OPC API key. Generate one from the agent's detail page in the dashboard (**Agents** > click agent > **Generate API Key**). Save the key — it's shown only once.

### Workflow

#### Step 1: Chat to create a project

Start a conversation with your OpenClaw agent. Describe what you want to build — the goal, constraints, and any preferences. The agent will:

1. List available OPC agents and their capabilities
2. Propose a project breakdown with issues, dependencies, and agent assignments
3. Discuss the plan with you until you're satisfied
4. When you say "go", create the project and all issues via the OPC API

The AI figures out which agent to assign to each task based on each agent's title, role, and capabilities. The project is created in **draft** status and all issues start in **backlog** — no agents are triggered yet.

#### Step 2: Approve the project

Open the OPC dashboard, go to **Projects**, and click on your new project. Review the issues, descriptions, and assignments. When you're satisfied, click **Approve Project**.

This activates all root-level issues (those with no `blocked_by` dependencies) and dispatches their assigned agents. Downstream issues remain in backlog until all their blockers are approved.

If the plan isn't right, you can **delete** the project — this cascades and removes all issues and related data.

#### Step 3: Review and approve each issue

As agents complete their work, issues appear in your **Approval Queue** (`/approvals`). For each submission, you can:

- **Approve** — marks the issue as done and triggers any downstream issues whose blockers are all resolved
- **Request Changes** — sends feedback to the agent, who re-works and re-submits
- **Reassign** — transfers the task to a different agent
- **Reject** — cancels the task

Each approval gates the next step. Agents never see each other's pending work — you are always the gatekeeper.

#### Step 4: Done

Once all issues are approved, the project is complete.

```
Chat with AI → Project + issues created → Approve project → Agents work →
You approve each issue → Downstream agents triggered → Repeat → All done
```

### Alternative: Create projects manually

Instead of using the planning skill, you can create projects and issues directly in the admin UI:

1. Go to **Projects** > **+ New Project** — enter a name, description, and optional repository URL
2. Go to **Issues** > **New Issue** — create issues linked to the project, assign agents, and set `blocked_by` dependencies
3. Go to the project detail page and click **Approve Project** to kick off all agents

This gives you full manual control over the project structure. Issues under a draft project are held in **backlog** until the project is approved.

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
1. Issue is created and assigned to an agent
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

### Task Dependencies (blocked_by)

Issues form a DAG (directed acyclic graph) via `blocked_by`. An issue can be blocked by **multiple** other issues and is only triggered when **all** of its blockers are approved. This supports sequential chains, fan-out (one task triggers many), and fan-in (one task waits for many prerequisites). Each agent sees its own task, the project description, and the descriptions + comments from all completed blocking issues as context.

### Where Does the Code Go?

OPC is a **task orchestration and approval system**, not a code hosting platform. When an agent writes code, the files live in the agent's workspace — OPC only captures the agent's text summary of what was done.

To bridge this gap, set `repo_url` on the **project**. When a project has a `repo_url`, OPC automatically appends git workflow instructions to every agent's prompt — telling it to clone the repo, create a branch (`task/{issue_id}`), do the work, commit, and push. You can then review the actual code diff on GitHub alongside the agent's summary in OPC.

### Event-Driven Triggers

Agents are triggered automatically by system events:

- **Project Approved** — root-level issues are activated and agents are dispatched
- **Issue Approved** — downstream issues with all blockers resolved are activated and their agents are dispatched
- **Changes Requested** — the assigned agent re-wakes with your feedback
- **Manual** — you click "Invoke" on an agent in the dashboard

## Connecting Agents

You can create and configure agents from the **dashboard** (go to **Agents** > **New Agent**) or via the API (see [API Reference](#api-reference)).

### Claude Code

OPC spawns a [Claude Code](https://docs.anthropic.com/en/docs/claude-code) CLI process with the task context as the prompt. Claude Code works locally on your machine, and OPC automatically submits its output for your approval.

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

OPC automatically generates an API key and stores it in the agent's `adapter_config.opc_api_key`. This key is embedded in the prompt so OpenClaw can call back to submit results.

**Config options:**

| Field | Required | Description |
|-------|----------|-------------|
| `webhook_url` | Yes | OpenClaw's `/hooks/agent` endpoint |
| `token` | Yes | Bearer token for OpenClaw authentication |
| `timeout_secs` | No | Timeout in seconds (default: 300) |
| `model` | No | Model override (e.g. `"anthropic/claude-sonnet-4-6"`) |
| `deliver` | No | Also post to a messaging channel (default: `false`) |
| `channel` | No | Target channel if `deliver` is `true` (e.g. `"slack"`) |
| `to` | No | Recipient if `deliver` is `true` (e.g. `"#general"`) |

**Flow:** OPC sends the task prompt to OpenClaw's webhook with `deliver: false` → OpenClaw processes the task silently → OpenClaw runs a curl command (embedded in the prompt) to submit results back to OPC → issue moves to `awaiting_approval`.

### HTTP Webhook

For custom agents, OPC POSTs the full task context to your webhook URL. Your agent processes the task and calls back to OPC's Agent API to submit results.

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
  "project": { "id": "...", "name": "My Project", "description": "..." },
  "comments": [{ "author_name": "admin", "body": "Check the logout flow too" }],
  "resolved_dependencies": [],
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

## Dashboard

Open **http://localhost:3100** in your browser and log in (`admin` / `admin`).

### Pages

| Page | URL | What You Do There |
|------|-----|-------------------|
| **Dashboard** | `/` | Overview of agent count, active issues, pending approvals, and recent activity |
| **Agents** | `/agents` | View all agents, their status, and quick actions (pause, resume, invoke) |
| **Agent Detail** | `/agents/{id}` | See an agent's config, budget, current assignments, and heartbeat history. Generate API keys |
| **Issues** | `/issues` | List all issues. Filter by status (todo, in progress, awaiting approval, done) |
| **Issue Detail** | `/issues/{id}` | View issue details, comment thread, dependencies (blocked by / blocks), and inline approval widget |
| **Approval Queue** | `/approvals` | Review all pending agent submissions. Approve, request changes, reassign, or reject |
| **Approval Detail** | `/approvals/{id}` | Full review page with the agent's summary, original task, conversation thread, and action buttons |
| **Projects** | `/projects` | Organize issues into projects. Approve draft projects to kick off agents |
| **Project Detail** | `/projects/{id}` | View project issues, approve the project, delete, and see agent updates |

### Chatting with Agents

You can communicate with agents through issue comments at any point:

- On the **Issue Detail** page (`/issues/{id}`), use the comment box to post messages
- Agents see your comments when they next wake up (via assignment, approval, or heartbeat)
- Comments from agents and humans are shown together in a threaded conversation
- When you **Request Changes**, your feedback is posted as a comment so the agent sees the full context

## API Reference

All management endpoints require session authentication (log in via the dashboard). Agent endpoints require an API key (`Authorization: Bearer opc_...`).

### Creating Agents

```bash
# Claude Code agent
curl -X POST http://localhost:3100/api/agents \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Alice",
    "title": "Frontend Developer",
    "capabilities": "React, TypeScript, CSS, responsive design",
    "adapter_type": "claude_code",
    "adapter_config": {
      "working_dir": "/home/user/project",
      "model": "sonnet"
    }
  }'

# OpenClaw agent
curl -X POST http://localhost:3100/api/agents \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Bob",
    "title": "Copywriter",
    "capabilities": "Marketing copy, blog posts, landing pages",
    "adapter_type": "openclaw",
    "adapter_config": {
      "webhook_url": "http://127.0.0.1:18789/hooks/agent",
      "token": "your-openclaw-token"
    }
  }'
```

### Generating API Keys

After creating an agent, generate an API key. This key is shown **once** — save it.

```bash
curl -X POST http://localhost:3100/api/agents/{agent_id}/keys
```

### Creating Projects and Issues

```bash
# Create a project (starts in draft)
curl -X POST http://localhost:3100/api/projects \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Landing Page",
    "description": "Company landing page redesign",
    "repo_url": "https://github.com/yourorg/landing-page.git"
  }'
# Returns: {"id": "project-uuid", ...}

# Create a root issue
curl -X POST http://localhost:3100/api/issues \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Write landing page copy",
    "description": "Write headline, subheading, 3 feature bullets, and a CTA.",
    "priority": "high",
    "project_id": "project-uuid",
    "assignee_id": "agent-bob-uuid"
  }'
# Returns: {"id": "issue-copy-uuid", ...}

# Create a dependent issue (blocked by the copy issue)
curl -X POST http://localhost:3100/api/issues \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Build landing page HTML/CSS",
    "description": "Create a responsive landing page using the approved copy.",
    "priority": "high",
    "project_id": "project-uuid",
    "blocked_by": ["issue-copy-uuid"],
    "assignee_id": "agent-alice-uuid"
  }'

# Approve the project to dispatch agents
curl -X POST http://localhost:3100/api/projects/project-uuid/approve

# Delete a project (cascades to all issues)
curl -X DELETE http://localhost:3100/api/projects/project-uuid
```

### Agent API

External agents authenticate with their API key and use these endpoints:

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/agent/me` | `GET` | Get the agent's own identity |
| `/api/agent/assignments` | `GET` | List assigned issues the agent can pick up |
| `/api/agent/agents` | `GET` | List available colleague agents (id, name, title, role, capabilities) |
| `/api/agent/issues` | `POST` | Create an issue (starts in backlog) |
| `/api/agent/issues/{id}/checkout` | `POST` | Atomically check out a task |
| `/api/agent/issues/{id}/checkin` | `POST` | Release a checked-out task without submitting |
| `/api/agent/issues/{id}/submit` | `POST` | Submit completed work for human approval |
| `/api/agent/issues/{id}/comments` | `GET` | Read the comment thread |
| `/api/agent/issues/{id}/comments` | `POST` | Post a comment on the issue |
| `/api/agent/projects` | `POST` | Create a project (starts in draft) |
| `/api/agent/projects/{id}/updates` | `POST` | Post a project-level progress update |

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

## Testing

OPC includes integration tests that exercise the full DAG dependency workflow end-to-end against a real embedded PostgreSQL instance.

```bash
# Run all integration tests (must use --test-threads=1 for embedded PG)
cargo test -p opc-server --test dag_workflow -- --test-threads=1
```

The test suite covers:

- **Diamond DAG workflow** — creates a project with A→{B,C}→D dependencies, approves the project, walks through agent checkout/submit and human approval for each issue, including a request-changes feedback loop. Verifies fan-in: D only activates when both B and C are done.
- **Fan-out activation** — one root issue fans out to three parallel children, all activate simultaneously on approval.
- **Rejection** — rejected approval correctly cancels the issue.
- **Comment thread** — human and agent comments appear together in the correct order.

Each test starts its own embedded PostgreSQL on a random port with a unique temp directory, so tests are fully isolated.

## License

MIT
