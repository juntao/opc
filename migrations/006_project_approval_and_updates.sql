-- Project-level approval workflow and cascade deletes

-- Add 'draft' to project status, change default
ALTER TABLE projects DROP CONSTRAINT IF EXISTS projects_status_check;
ALTER TABLE projects ADD CONSTRAINT projects_status_check
    CHECK (status IN ('draft', 'active', 'archived'));
ALTER TABLE projects ALTER COLUMN status SET DEFAULT 'draft';

-- Project updates table (agents post progress at project level)
CREATE TABLE project_updates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    company_id UUID NOT NULL REFERENCES companies(id),
    agent_id UUID NOT NULL REFERENCES agents(id),
    issue_id UUID REFERENCES issues(id) ON DELETE SET NULL,
    body TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_project_updates_project ON project_updates(project_id);

-- Cascade deletes: deleting a project cleans up everything
ALTER TABLE issues DROP CONSTRAINT IF EXISTS issues_project_id_fkey;
ALTER TABLE issues ADD CONSTRAINT issues_project_id_fkey
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE;

ALTER TABLE approval_requests DROP CONSTRAINT IF EXISTS approval_requests_issue_id_fkey;
ALTER TABLE approval_requests ADD CONSTRAINT approval_requests_issue_id_fkey
    FOREIGN KEY (issue_id) REFERENCES issues(id) ON DELETE CASCADE;

ALTER TABLE heartbeat_runs DROP CONSTRAINT IF EXISTS heartbeat_runs_issue_id_fkey;
ALTER TABLE heartbeat_runs ADD CONSTRAINT heartbeat_runs_issue_id_fkey
    FOREIGN KEY (issue_id) REFERENCES issues(id) ON DELETE CASCADE;

ALTER TABLE cost_events DROP CONSTRAINT IF EXISTS cost_events_issue_id_fkey;
ALTER TABLE cost_events ADD CONSTRAINT cost_events_issue_id_fkey
    FOREIGN KEY (issue_id) REFERENCES issues(id) ON DELETE CASCADE;

ALTER TABLE cost_events DROP CONSTRAINT IF EXISTS cost_events_project_id_fkey;
ALTER TABLE cost_events ADD CONSTRAINT cost_events_project_id_fkey
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE;
