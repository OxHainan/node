use std::{pin::Pin, sync::Arc};

use crate::{event::Event, out_events};
use futures::{
    channel::{mpsc, oneshot},
    Stream,
};
use serde_json::Value;
use uuid::Uuid;

/// Error in a request.
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum RequestFailure {
    #[error("We are not currently connected to the requested peer.")]
    NotConnected,
    #[error("Given protocol hasn't been registered.")]
    UnknownProtocol,
    #[error("Remote has closed the substream before answering, thereby signaling that it considers the request as valid, but refused to answer it.")]
    Refused,
    #[error("The remote replied, but the local node is no longer interested in the response.")]
    Obsolete,
}

pub enum ServiceToWorkerMsg {
    EventStream(out_events::Sender),
    Request {
        request_id: Uuid,
        remote: Uuid,
        request: Value,
        fallback_request: Option<Vec<u8>>,
        pending_response: oneshot::Sender<Result<Value, RequestFailure>>,
    },
}

pub struct EngineService {
    to_worker: mpsc::UnboundedSender<ServiceToWorkerMsg>,
}

impl EngineService {
    pub fn new(to_worker: mpsc::UnboundedSender<ServiceToWorkerMsg>) -> Self {
        Self { to_worker }
    }
}

#[async_trait::async_trait]
impl EngineRequest for EngineService {
    async fn request(
        &self,
        request_id: Uuid,
        remote: &Uuid,
        request: Value,
        fallback_request: Option<Vec<u8>>,
    ) -> Result<Value, RequestFailure> {
        let (tx, rx) = oneshot::channel();

        self.start_request(request_id, remote, request, fallback_request, tx);
        match rx.await {
            Ok(v) => v,
            Err(_) => Err(RequestFailure::NotConnected),
        }
    }

    fn start_request(
        &self,
        request_id: Uuid,
        remote: &Uuid,
        request: Value,
        fallback_request: Option<Vec<u8>>,
        tx: oneshot::Sender<Result<Value, RequestFailure>>,
    ) {
        let _ = self.to_worker.unbounded_send(ServiceToWorkerMsg::Request {
            request_id,
            remote: remote.clone(),
            request,
            fallback_request,
            pending_response: tx,
        });
    }
}

pub trait EventStream {
    fn event_stream(&self, remote: &Uuid) -> Pin<Box<dyn Stream<Item = Event> + Send>>;
}

impl<T> EventStream for Arc<T>
where
    T: EventStream + ?Sized,
{
    fn event_stream(&self, remote: &Uuid) -> Pin<Box<dyn Stream<Item = Event> + Send>> {
        T::event_stream(self, remote)
    }
}

#[async_trait::async_trait]
pub trait EngineRequest {
    async fn request(
        &self,
        request_id: Uuid,
        remote: &Uuid,
        request: Value,
        fallback_request: Option<Vec<u8>>,
    ) -> Result<Value, RequestFailure>;

    fn start_request(
        &self,
        request_id: Uuid,
        remote: &Uuid,
        request: Value,
        fallback_request: Option<Vec<u8>>,
        tx: oneshot::Sender<Result<Value, RequestFailure>>,
    );
}

#[async_trait::async_trait]
impl<T> EngineRequest for Arc<T>
where
    T: EngineRequest + ?Sized + Send + Sync,
{
    async fn request(
        &self,
        request_id: Uuid,
        remote: &Uuid,
        request: Value,
        fallback_request: Option<Vec<u8>>,
    ) -> Result<Value, RequestFailure> {
        T::request(self, request_id, remote, request, fallback_request).await
    }

    fn start_request(
        &self,
        request_id: Uuid,
        remote: &Uuid,
        request: Value,
        fallback_request: Option<Vec<u8>>,
        tx: oneshot::Sender<Result<Value, RequestFailure>>,
    ) {
        T::start_request(self, request_id, remote, request, fallback_request, tx);
    }
}
