//! QUIC transport protocol support for Tokio
//!
//! [QUIC](https://en.wikipedia.org/wiki/QUIC) is a modern transport protocol addressing
//! shortcomings of TCP, such as head-of-line blocking, poor security, slow handshakes, and
//! inefficient congestion control. This crate provides a portable userspace implementation. It
//! builds on top of quinn-proto, which implements protocol logic independent of any particular
//! runtime.
//!
//! The entry point of this crate is the [`Endpoint`](generic/struct.Endpoint.html).
//!
#![cfg_attr(
    feature = "rustls",
    doc = "```no_run
# use futures::TryFutureExt;
let mut builder = quinn::Endpoint::builder();
// ... configure builder ...
// Ensure you're inside a tokio runtime context
let (endpoint, _) = builder.bind(&\"[::]:0\".parse().unwrap()).unwrap();
// ... use endpoint ...
```"
)]
//! # About QUIC
//!
//! A QUIC connection is an association between two endpoints. The endpoint which initiates the
//! connection is termed the client, and the endpoint which accepts it is termed the server. A
//! single endpoint may function as both client and server for different connections, for example
//! in a peer-to-peer application. To communicate application data, each endpoint may open streams
//! up to a limit dictated by its peer. Typically, that limit is increased as old streams are
//! finished.
//!
//! Streams may be unidirectional or bidirectional, and are cheap to create and disposable. For
//! example, a traditionally datagram-oriented application could use a new stream for every
//! message it wants to send, no longer needing to worry about MTUs. Bidirectional streams behave
//! much like a traditional TCP connection, and are useful for sending messages that have an
//! immediate response, such as an HTTP request. Stream data is delivered reliably, and there is no
//! ordering enforced between data on different streams.
//!
//! By avoiding head-of-line blocking and providing unified congestion control across all streams
//! of a connection, QUIC is able to provide higher throughput and lower latency than one or
//! multiple TCP connections between the same two hosts, while providing more useful behavior than
//! raw UDP sockets.
//!
//! Quinn also exposes unreliable datagrams, which are a low-level primitive preferred when
//! automatic fragmentation and retransmission of certain data is not desired.
//!
//! QUIC uses encryption and identity verification built directly on TLS 1.3. Just as with a TLS
//! server, it is useful for a QUIC server to be identified by a certificate signed by a trusted
//! authority. If this is infeasible--for example, if servers are short-lived or not associated
//! with a domain name--then as with TLS, self-signed certificates can be used to provide
//! encryption alone.
#![warn(missing_docs)]

mod broadcast;
mod builders;
mod connection;
mod endpoint;
mod mutex;
mod platform;
mod recv_stream;
mod send_stream;

pub use proto::{
    crypto, ApplicationClose, Certificate, CertificateChain, Chunk, ConfigError, ConnectError,
    ConnectionClose, ConnectionError, ParseError, PrivateKey, StreamId, Transmit, TransportConfig,
    VarInt,
};

pub use crate::{
    builders::EndpointError,
    connection::{SendDatagramError, ZeroRttAccepted},
    recv_stream::{ReadError, ReadExactError, ReadToEndError},
    send_stream::{StoppedError, WriteError},
};

/// Types that are generic over the crypto protocol implementation
pub mod generic {
    pub use crate::{
        builders::{ClientConfigBuilder, EndpointBuilder, ServerConfigBuilder},
        connection::{
            Connecting, Connection, Datagrams, IncomingBiStreams, IncomingUniStreams,
            NewConnection, OpenBi, OpenUni,
        },
        endpoint::{Endpoint, Incoming},
        recv_stream::{Read, ReadChunk, ReadChunks, ReadExact, ReadToEnd, RecvStream},
        send_stream::SendStream,
    };
    pub use proto::generic::{ClientConfig, ServerConfig};
}

/// Traits and implementations for underlying connection on which QUIC packets transmit.
pub mod transport {
    use crate::platform::SocketCapabilities;
    pub use crate::platform::{RecvMeta, UdpSocket};
    use proto::Transmit;
    use std::{
        io::{IoSliceMut, Result},
        net::SocketAddr,
        task::{Context, Poll},
    };

    /// A socket that abstracts the underlying connection
    pub trait Socket: Send + 'static {
        /// Poll the underlying connection to send `Transmit`, return the number of successfully transmitted `Transmit`.
        fn poll_send(&self, cx: &mut Context, transmits: &mut [Transmit]) -> Poll<Result<usize>>;

        /// Poll the underlying connection to receive, return the number of received bufs.
        fn poll_recv(
            &self,
            cx: &mut Context,
            bufs: &mut [IoSliceMut<'_>],
            meta: &mut [RecvMeta],
        ) -> Poll<Result<usize>>;

        /// The socket address of the local endpoint, return an arbitrary port with the IP address
        /// if the connection doesn't support socket address (e.g. ICMP)
        fn local_addr(&self) -> Result<SocketAddr>;

        /// Returns the platforms (UDP) socket capabilities. Default to 1 for max_gso_segments.
        fn caps() -> SocketCapabilities {
            SocketCapabilities {
                max_gso_segments: 1,
            }
        }
    }
}

#[cfg(feature = "rustls")]
mod rustls_impls {
    use crate::{generic, platform::UdpSocket};
    use proto::crypto::rustls::TlsSession;

    /// A `ClientConfig` using rustls for the cryptography protocol
    pub type ClientConfig = generic::ClientConfig<TlsSession>;
    /// A `ServerConfig` using rustls for the cryptography protocol
    pub type ServerConfig = generic::ServerConfig<TlsSession>;

    /// A `ClientConfigBuilder` using rustls for the cryptography protocol
    pub type ClientConfigBuilder = generic::ClientConfigBuilder<TlsSession>;
    /// An `EndpointBuilder` using rustls for the cryptography protocol and UDP socket for underlying connection.
    pub type EndpointBuilder = generic::EndpointBuilder<TlsSession, UdpSocket>;
    /// A `ServerConfigBuilder` using rustls for the cryptography protocol
    pub type ServerConfigBuilder = generic::ServerConfigBuilder<TlsSession>;

    /// A `Connecting` using rustls for the cryptography protocol
    pub type Connecting = generic::Connecting<TlsSession, UdpSocket>;
    /// A `Connection` using rustls for the cryptography protocol
    pub type Connection = generic::Connection<TlsSession, UdpSocket>;
    /// A `Datagrams` using rustls for the cryptography protocol
    pub type Datagrams = generic::Datagrams<TlsSession, UdpSocket>;
    /// An `IncomingBiStreams` using rustls for the cryptography protocol
    pub type IncomingBiStreams = generic::IncomingBiStreams<TlsSession, UdpSocket>;
    /// An `IncomingUniStreams` using rustls for the cryptography protocol
    pub type IncomingUniStreams = generic::IncomingUniStreams<TlsSession, UdpSocket>;
    /// A `NewConnection` using rustls for the cryptography protocol
    pub type NewConnection = generic::NewConnection<TlsSession, UdpSocket>;
    /// An `OpenBi` using rustls for the cryptography protocol
    pub type OpenBi = generic::OpenBi<TlsSession, UdpSocket>;
    /// An `OpenUni` using rustls for the cryptography protocol
    pub type OpenUni = generic::OpenUni<TlsSession, UdpSocket>;

    /// An `Endpoint` using rustls for the cryptography protocol and UDP socket for underlying connection.
    pub type Endpoint = generic::Endpoint<TlsSession, UdpSocket>;
    /// An `Incoming` using rustls for the cryptography protocol and UDP socket for underlying connection.
    pub type Incoming = generic::Incoming<TlsSession, UdpSocket>;

    /// A `Read` using rustls for the cryptography protocol
    pub type Read<'a> = generic::Read<'a, TlsSession, UdpSocket>;
    /// A `ReadExact` using rustls for the cryptography protocol
    pub type ReadExact<'a> = generic::ReadExact<'a, TlsSession, UdpSocket>;
    /// A `ReadToEnd` using rustls for the cryptography protocol
    pub type ReadToEnd = generic::ReadToEnd<TlsSession, UdpSocket>;
    /// A `RecvStream` using rustls for the cryptography protocol
    pub type RecvStream = generic::RecvStream<TlsSession, UdpSocket>;
    /// A `SendStream` using rustls for the cryptography protocol
    pub type SendStream = generic::SendStream<TlsSession, UdpSocket>;
}

#[cfg(feature = "rustls")]
pub use rustls_impls::*;

#[cfg(test)]
mod tests;

#[derive(Debug)]
enum ConnectionEvent {
    Close {
        error_code: VarInt,
        reason: bytes::Bytes,
    },
    Proto(proto::ConnectionEvent),
}

#[derive(Debug)]
enum EndpointEvent {
    Proto(proto::EndpointEvent),
    Transmit(proto::Transmit),
}

/// Maximum number of send/recv calls to make before moving on to other processing
///
/// This helps ensure we don't starve anything when the CPU is slower than the link. Value selected
/// more or less arbitrarily.
const IO_LOOP_BOUND: usize = 10;
