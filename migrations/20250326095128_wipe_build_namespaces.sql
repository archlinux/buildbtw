-- Since the srcinfo representation has changed, we remove all build iterations.
-- This is only possible because we're still at the PoC stage :)
delete from gitlab_pipelines;
delete from build_set_iterations;

