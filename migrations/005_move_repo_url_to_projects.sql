-- Move repo_url from issues to projects
ALTER TABLE issues DROP COLUMN IF EXISTS repo_url;
ALTER TABLE projects ADD COLUMN repo_url TEXT;
