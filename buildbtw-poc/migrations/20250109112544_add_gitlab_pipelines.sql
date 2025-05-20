create table gitlab_pipelines (
    id blob not null primary key,

    build_set_iteration_id blob not null 
        references build_set_iterations (id),
    pkgbase text not null,

    project_gitlab_iid integer not null,
    gitlab_iid integer not null
);
