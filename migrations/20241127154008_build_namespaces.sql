create table build_namespaces (
    id text not null primary key,
    name text not null unique,
    origin_changesets text not null,
    created_at text not null
) strict;
