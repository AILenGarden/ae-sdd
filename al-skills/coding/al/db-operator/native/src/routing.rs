use std::future::Future;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaemonAttempt<T, E> {
    Success(T),
    Unavailable,
    Fatal(E),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Routed<T> {
    pub value: T,
    pub mode: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutingError<E> {
    DaemonUnavailable,
    Fatal(E),
}

pub async fn execute_daemon_only<T, E, D, DFut>(daemon: D) -> Result<Routed<T>, RoutingError<E>>
where
    D: FnOnce() -> DFut,
    DFut: Future<Output = DaemonAttempt<T, E>>,
{
    match daemon().await {
        DaemonAttempt::Success(value) => Ok(Routed {
            value,
            mode: "daemon",
        }),
        DaemonAttempt::Unavailable => Err(RoutingError::DaemonUnavailable),
        DaemonAttempt::Fatal(error) => Err(RoutingError::Fatal(error)),
    }
}
