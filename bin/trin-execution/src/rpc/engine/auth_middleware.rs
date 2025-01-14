use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use alloy_rpc_types_engine::JwtSecret;
use http::{header, StatusCode};
use jsonrpsee::http_client::{HttpRequest, HttpResponse};
use pin_project::pin_project;
use tower::{Layer, Service};

pub struct JwtAuthLayer {
    jwt_secret: JwtSecret,
}

impl JwtAuthLayer {
    pub fn new(jwt_secret: JwtSecret) -> Self {
        Self { jwt_secret }
    }
}

impl<S> Layer<S> for JwtAuthLayer {
    type Service = JwtAuthService<S>;

    fn layer(&self, service: S) -> Self::Service {
        JwtAuthService {
            jwt_secret: self.jwt_secret,
            service,
        }
    }
}

#[derive(Clone)]
pub struct JwtAuthService<S> {
    jwt_secret: JwtSecret,
    service: S,
}

impl<S> Service<HttpRequest> for JwtAuthService<S>
where
    S: Service<HttpRequest, Response = HttpResponse>,
{
    type Response = HttpResponse;
    type Error = S::Error;
    type Future = ResponseFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, request: HttpRequest) -> Self::Future {
        // get jwt from the header
        let jwt_str = match extract_jwt_from_header(request.headers()) {
            Some(jwt) => jwt,
            None => {
                return ResponseFuture::response(build_unauthorized_response("No JWT provided"))
            }
        };

        // validate jwt
        match self.jwt_secret.validate(jwt_str) {
            Ok(_) => ResponseFuture::future(self.service.call(request)),
            Err(_) => ResponseFuture::response(build_unauthorized_response("Invalid JWT")),
        }
    }
}

#[pin_project(project = ResponseKindProject)]
enum ResponseKind<F> {
    Future {
        #[pin]
        future: F,
    },
    Response {
        response: Option<HttpResponse>,
    },
}

#[pin_project]
pub struct ResponseFuture<F> {
    #[pin]
    response_future: ResponseKind<F>,
}

impl<F> ResponseFuture<F> {
    pub fn future(future: F) -> Self {
        Self {
            response_future: ResponseKind::Future { future },
        }
    }

    pub fn response(response: HttpResponse) -> Self {
        Self {
            response_future: ResponseKind::Response {
                response: Some(response),
            },
        }
    }
}

impl<F, Error> Future for ResponseFuture<F>
where
    F: Future<Output = Result<HttpResponse, Error>>,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project().response_future.project() {
            ResponseKindProject::Future { future } => future.poll(cx),
            ResponseKindProject::Response { response } => {
                Poll::Ready(Ok(response.take().expect("Response already taken")))
            }
        }
    }
}

fn extract_jwt_from_header(header: &http::HeaderMap) -> Option<&str> {
    let authentication_header = header.get(header::AUTHORIZATION)?.to_str().ok()?;
    let token = authentication_header.split_whitespace().nth(1)?;
    Some(token)
}

fn build_unauthorized_response(err: &str) -> HttpResponse {
    HttpResponse::builder()
        .status(StatusCode::UNAUTHORIZED)
        .body(format!("Unauthorized: {}", err).into())
        .expect("Failed to build response this should never happen")
}
