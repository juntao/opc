-- Replace parent_issue_id (tree) with issue_dependencies (DAG)

CREATE TABLE issue_dependencies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    issue_id UUID NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    depends_on_id UUID NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT uq_issue_dependency UNIQUE (issue_id, depends_on_id),
    CONSTRAINT chk_no_self_dependency CHECK (issue_id != depends_on_id)
);

CREATE INDEX idx_issue_deps_issue ON issue_dependencies(issue_id);
CREATE INDEX idx_issue_deps_depends_on ON issue_dependencies(depends_on_id);

-- Migrate existing parent_issue_id data into junction table
INSERT INTO issue_dependencies (issue_id, depends_on_id)
SELECT id, parent_issue_id FROM issues WHERE parent_issue_id IS NOT NULL;

-- Drop old column and index
DROP INDEX IF EXISTS idx_issues_parent;
ALTER TABLE issues DROP COLUMN parent_issue_id;
