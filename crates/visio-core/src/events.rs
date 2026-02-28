/// Events emitted by the core to native UI listeners.
#[derive(Debug, Clone)]
pub enum VisioEvent {
    /// Room connection state changed.
    ConnectionStateChanged(ConnectionState),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
}
