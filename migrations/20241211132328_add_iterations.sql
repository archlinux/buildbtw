create table build_set_iterations (
    id blob not null primary key,
    created_at text not null,
    namespace_id blob not null references build_namespaces (id),
    origin_changesets text not null,
    packages_to_be_built text not null,
    create_reason text not null
) strict;
