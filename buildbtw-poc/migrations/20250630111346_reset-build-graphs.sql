-- Since the JSON representation of our build graphs changed, we
-- drop all of them to make the database compatible with the new code.
delete from gitlab_pipelines;
delete from build_set_iterations;
