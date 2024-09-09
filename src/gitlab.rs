use regex::Regex;

/// Convert arbitrary project names to GitLab valid path names.
///
/// GitLab has several limitations on project and group names and also maintains
/// a list of reserved keywords as documented on their docs.
/// https://docs.gitlab.com/ee/user/reserved_names.html
///
/// 1. replace single '+' between word boundaries with '-'
/// 2. replace any other '+' with literal 'plus'
/// 3. replace any special chars other than '_', '-' and '.' with '-'
/// 4. replace consecutive '_-' chars with a single '-'
/// 5. replace 'tree' with 'unix-tree' due to GitLab reserved keyword
pub fn gitlab_project_name_to_path(project_name: &str) -> String {
    if project_name == "tree" {
        return "unix-tree".to_string();
    }
    let project_name = Regex::new(r"([a-zA-Z0-9]+)\+([a-zA-Z]+)")
        .unwrap()
        .replace_all(project_name, "$1-$2")
        .to_string();
    let project_name = Regex::new(r"\+")
        .unwrap()
        .replace_all(&project_name, "plus")
        .to_string();
    let project_name = Regex::new(r"[^a-zA-Z0-9_\-.]")
        .unwrap()
        .replace_all(&project_name, "-")
        .to_string();
    let project_name = Regex::new(r"[_\\-]{2,}")
        .unwrap()
        .replace_all(&project_name, "-")
        .to_string();
    project_name
}

#[test]
fn gitlab_project_name_to_path_plus_signs() {
    let project_name = "archlinux++";
    assert_eq!(
        gitlab_project_name_to_path(project_name),
        "archlinuxplusplus".to_string()
    );
}

#[test]
fn gitlab_project_name_to_path_plus_signs_with_suffix() {
    let project_name = "archlinux++-5.0";
    assert_eq!(
        gitlab_project_name_to_path(project_name),
        "archlinuxplusplus-5.0".to_string()
    );
}

#[test]
fn gitlab_project_name_to_path_plus_tree() {
    let project_name = "tree";
    assert_eq!(
        gitlab_project_name_to_path(project_name),
        "unix-tree".to_string()
    );
}

#[test]
fn gitlab_project_name_to_path_plus_word_separator() {
    let project_name = "arch+linux";
    assert_eq!(
        gitlab_project_name_to_path(project_name),
        "arch-linux".to_string()
    );
}
