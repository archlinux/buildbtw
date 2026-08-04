pub mod builds;
pub mod buildspaces;
pub mod users;

fn garde_report(path: garde::Path, error: garde::Error) -> garde::Report {
    let mut report = garde::Report::new();

    report.append(path, error);

    report
}
