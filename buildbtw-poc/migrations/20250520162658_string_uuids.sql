drop table gitlab_pipelines;
drop table build_set_iterations;
drop table build_namespaces;

-- Recreate all tables, this time with `text` as the column type for uuids.
create table gitlab_pipelines (
    id text primary key not null,
    build_set_iteration_id text not null 
        references build_set_iterations (id),
    pkgbase text not null,
    project_gitlab_iid integer not null,
    gitlab_iid integer not null,
    architecture text not null
) strict;

create table build_set_iterations (
    id text not null primary key,
    created_at text not null,
    namespace_id text not null references build_namespaces (id),
    origin_changesets text not null,
    packages_to_be_built text not null,
    create_reason text not null
) strict;

create table build_namespaces (
    id text not null primary key,
    name text not null unique,
    origin_changesets text not null,
    created_at text not null, 
    status text default "Active" not null
) strict;