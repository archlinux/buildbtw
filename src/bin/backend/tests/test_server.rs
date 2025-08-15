use axum::Router;

use crate::{router, tests::request_builder::RequestBuilder};

pub struct TestServer {
    pub router: Router,
}

impl TestServer {
    pub fn new() -> Self {
        let router = router::new();

        TestServer { router }
    }

    pub fn req(&self) -> RequestBuilder {
        RequestBuilder::new(&self.router)
    }
}
