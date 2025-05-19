use std::{collections::HashMap, future::Future, pin::Pin, time::Instant};

use futures::{channel::oneshot, stream};
use serde_json::Value;
use uuid::Uuid;

use crate::{service::RequestFailure, Params};

pub struct PendingRequest {
    started_at: Instant,
    response_tx: Option<oneshot::Sender<Result<Value, RequestFailure>>>,
    fallback_request: Option<Vec<u8>>,
}

pub struct RequestResponses {
    pending_requests: HashMap<Uuid, PendingRequest>,
    pending_response:
        stream::FuturesUnordered<Pin<Box<dyn Future<Output = Value> + Send + 'static>>>,
}

impl RequestResponses {
    pub fn new() -> Self {
        Self {
            pending_requests: HashMap::new(),
            pending_response: stream::FuturesUnordered::new(),
        }
    }

    pub fn on_request_response(&mut self, params: Params) {
        if let Some(mut request) = self.pending_requests.remove(&params.id) {
            if let Some(tx) = request.response_tx.take() {
                let _ = tx.send(Ok(params.data));
            }
        }
    }

    pub fn pending_requests(
        &mut self,
        request_id: Uuid,
        fallback_request: Option<Vec<u8>>,
        pending_response: oneshot::Sender<Result<Value, RequestFailure>>,
    ) {
        self.pending_requests.insert(
            request_id,
            PendingRequest {
                started_at: Instant::now(),
                response_tx: Some(pending_response),
                fallback_request,
            },
        );
    }
}
