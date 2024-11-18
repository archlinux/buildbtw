create table global_state (
    -- The time of the latest source repo change queried from gitlab.
    -- Stored as ISO 8601 date.
    gitlab_last_updated text default null
) strict;
