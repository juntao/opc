-- Add repo_url column to issues for automatic git workflow instructions
ALTER TABLE issues ADD COLUMN repo_url TEXT;
