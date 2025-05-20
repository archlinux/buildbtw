alter table build_namespaces 
    add column status 
        text 
        default "Active" 
        not null;