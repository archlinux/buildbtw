use axum::{
    Router,
    body::Body,
    http::{self, HeaderName, HeaderValue, Request, Response, StatusCode, request},
};
use tower::{Service, ServiceExt};

pub struct RequestBuilder {
    router: Router,
    /// This is the HTTP status that we expect the backend to return.
    /// If it returns a different status, we'll panic.
    expected_status: StatusCode,
    request: request::Builder,
}

impl RequestBuilder {
    pub fn new(router: &Router) -> Self {
        RequestBuilder {
            router: router.clone(),
            expected_status: StatusCode::OK,
            request: Request::builder(),
        }
    }

    pub fn expect_status(mut self, expected: StatusCode) -> Self {
        self.expected_status = expected;
        self
    }

    #[expect(dead_code)]
    pub fn header<V>(mut self, key: HeaderName, val: V) -> Self
    where
        HeaderValue: TryFrom<V>,
        <HeaderValue as TryFrom<V>>::Error: Into<http::Error>,
    {
        self.request = self.request.header(key, val);
        self
    }

    pub async fn get(mut self, url: &str) -> Response<Body> {
        let request = self.request.uri(url).body(Body::empty()).unwrap();

        let response = ServiceExt::<Request<Body>>::ready(&mut self.router)
            .await
            .unwrap()
            .call(request)
            .await
            .unwrap();

        tracing::debug!("{:?}", response.headers());

        Self::assert_expected_status(self.expected_status, &response, "GET", url);

        response
    }

    fn assert_expected_status(
        expected_status: StatusCode,
        response: &Response<Body>,
        method: &str,
        url: &str,
    ) {
        assert_eq!(
            response.status(),
            expected_status,
            "expected {expected_status}: {method} {url}"
        );
    }
}
