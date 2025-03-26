delete from gitlab_pipelines;

alter table gitlab_pipelines
    add column architecture
        text
        not null;
