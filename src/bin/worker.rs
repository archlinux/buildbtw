//! Local build executor that processes packages using `pkgctl`.
//!
//! The buildbtw worker receives build requests from the backend server and
//! processes them locally using `pkgctl build`.
//!
//! Workers can run on the same machine as the server or on separate build
//! nodes. They report results to the backend via its JSON API.
fn main() {}

