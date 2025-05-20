delete from gitlab_pipelines;

alter table gitlab_pipelines
    add column gitlab_url text not null;
