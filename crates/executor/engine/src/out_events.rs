use std::{backtrace::Backtrace, fmt, pin::Pin, task::Poll};

use futures::{ready, stream::FusedStream, FutureExt, Stream, StreamExt};
use log::debug;
use uuid::Uuid;

use crate::event::Event;

pub fn channel(remote: Uuid, queue_size_warning: usize) -> (Sender, Receiver) {
    let (tx, rx) = async_channel::unbounded();
    (
        Sender {
            inner: tx,
            remote,
            queue_size_warning,
            warning_fired: SenderWarningState::NotFired,
            creation_backtrace: Backtrace::force_capture(),
        },
        Receiver { inner: rx, remote },
    )
}

/// A state of a sender warning that is used to avoid spamming the logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SenderWarningState {
    /// The warning has not been fired yet.
    NotFired,
    /// The warning has been fired, and the channel is full
    FiredFull,
    /// The warning has been fired and the channel is not full anymore.
    FiredFree,
}

pub struct Sender {
    inner: async_channel::Sender<Event>,
    remote: Uuid,
    queue_size_warning: usize,
    warning_fired: SenderWarningState,
    creation_backtrace: Backtrace,
}

impl fmt::Debug for Sender {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Sender").finish()
    }
}

pub struct Receiver {
    inner: async_channel::Receiver<Event>,
    remote: Uuid,
}

impl Stream for Receiver {
    type Item = Event;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        if let Some(ev) = ready!(Pin::new(&mut self.inner).poll_next(cx)) {
            Poll::Ready(Some(ev))
        } else {
            Poll::Ready(None)
        }
    }
}

impl fmt::Debug for Receiver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Receiver").finish()
    }
}

impl Drop for Receiver {
    fn drop(&mut self) {
        if !self.inner.is_terminated() {
            while let Some(Some(_)) = self.next().now_or_never() {}
        }
    }
}

// 主要用于订阅消息使用
// 如docker可能主动发现消息，此消息将被订阅的模块接收
pub struct Channels {
    event_streams: Vec<Sender>,
}

impl Channels {
    pub fn new() -> Self {
        Self {
            event_streams: Vec::new(),
        }
    }

    pub fn push(&mut self, sender: Sender) {
        self.event_streams.push(sender);
    }

    pub fn send(&mut self, event: Event) {
        self.event_streams.retain_mut(|sender| {
            let current_pending = sender.inner.len();
            if current_pending >= sender.queue_size_warning {
                if sender.warning_fired == SenderWarningState::NotFired {
                    log::error!(
                        "The number of unprocessed events in channel `{}` exceeded {}.\n\
						 The channel was created at:\n{:}\n
						 The last event was sent from:\n{:}",
                        sender.remote,
                        sender.queue_size_warning,
                        sender.creation_backtrace,
                        Backtrace::force_capture(),
                    );
                } else if sender.warning_fired == SenderWarningState::FiredFree {
                    // We don't want to spam the logs, so we only log on debug level
                    debug!(
                        "Channel `{}` is overflowed again. Number of events: {}",
                        sender.remote, current_pending
                    );
                }
                sender.warning_fired = SenderWarningState::FiredFull;
            } else if sender.warning_fired == SenderWarningState::FiredFull
                && current_pending < sender.queue_size_warning.wrapping_div(2)
            {
                sender.warning_fired = SenderWarningState::FiredFree;
                debug!(
                    "Channel `{}` is no longer overflowed. Number of events: {}",
                    sender.remote, current_pending
                );
            }
            sender.inner.try_send(event.clone()).is_ok()
        });
    }
}

impl fmt::Debug for Channels {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Channels")
            .field("num_channels", &self.event_streams.len())
            .finish()
    }
}
