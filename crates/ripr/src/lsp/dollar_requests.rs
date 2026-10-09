//! Answers unhandled `$/` requests with `MethodNotFound` (#4456).
//!
//! LSP 3.17 lets a server ignore a `$/` *notification* but requires it to
//! error a `$/` *request* with `MethodNotFound`. tower-lsp-server 0.23
//! (`LspService::call`) turns every `$/` `MethodNotFound` into no response,
//! requests included, so a client that sent one would wait on it forever.
//! This layer sits between the transport and `LspService` and restores the
//! error for requests only. It acts on the inner service's answer rather than
//! short-circuiting the call, so a registered `$/` handler, the exited state
//! and cancellation all keep their existing behavior.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use tower::Service;
use tower_lsp_server::jsonrpc::{Error, Id, Request, Response};
use tower_lsp_server::ls_types::LSPAny;

pub(super) struct AnswerDollarRequests<S>(pub(super) S);

impl<S> Service<Request> for AnswerDollarRequests<S>
where
    S: Service<Request, Response = Option<Response>>,
    S::Future: Send + 'static,
{
    type Response = Option<Response>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Option<Response>, S::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.0.poll_ready(cx)
    }

    fn call(&mut self, request: Request) -> Self::Future {
        let dollar_request = dollar_request(&request);
        let response = self.0.call(request);
        Box::pin(async move {
            let response = response.await?;
            Ok(match (response, dollar_request) {
                (None, Some((id, method))) => Some(method_not_found(id, method)),
                (response, _) => response,
            })
        })
    }
}

/// The id and method of a `$/` request; `None` for notifications and for
/// every other method.
fn dollar_request(request: &Request) -> Option<(Id, String)> {
    let method = request.method();
    if !method.starts_with("$/") {
        return None;
    }
    request.id().map(|id| (id.clone(), method.to_owned()))
}

/// Same shape as the router's own unknown-method error: the method name
/// rides in `data`.
fn method_not_found(id: Id, method: String) -> Response {
    let mut error = Error::method_not_found();
    error.data = Some(LSPAny::String(method));
    Response::from_error(id, error)
}
