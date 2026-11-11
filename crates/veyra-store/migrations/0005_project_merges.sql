-- 0005: projects users rename and merge (ADR-027).
--
-- `usage_events.project_id` is the project usage counts toward: the project itself, or
-- the one it is merged into. `source_project_id` keeps the project the event was
-- recorded in, so a merge can always be undone. Rollups keep reading `project_id`.

ALTER TABLE projects ADD COLUMN merged_into INTEGER REFERENCES projects (id);

ALTER TABLE usage_events ADD COLUMN source_project_id INTEGER REFERENCES projects (id);
UPDATE usage_events SET source_project_id = project_id;

-- Merging and unmerging select a project's events by where they were recorded.
CREATE INDEX usage_events_source_project ON usage_events (source_project_id)
    WHERE source_project_id IS NOT NULL;
