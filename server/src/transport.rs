//! Implements quinn for glommio sockets

mod certs;

use bytes::BytesMut;
use glommio::net::UdpSocket;
use quinn_proto::{Connection, ConnectionHandle, DatagramEvent, Endpoint, EndpointConfig};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

/// Setup a dequinn endpoint
fn setup_quinn() -> Endpoint {
    // generate our certs
    let (certs, key) = certs::generate();
    // build the crypto for our server
    let server_crypto = rustls::ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .unwrap();
    // build the server config
    let server_config = quinn_proto::ServerConfig::with_crypto(Arc::new(server_crypto));
    // build our quinn endpoint
    Endpoint::new(
        Arc::new(EndpointConfig::default()),
        Some(Arc::new(server_config)),
        false,
    )
}

#[derive(Debug)]
pub struct Transport {
    /// The udp socket we are reading from
    udp_sock: UdpSocket,
    /// The quinn endpoint
    quinn: Endpoint,
    /// The connections we are tracking
    conn_map: HashMap<ConnectionHandle, Connection>,
}

impl Transport {
    /// Create a new Transport object
    pub fn new() -> Self {
        // bind our udp socket
        let udp_sock = UdpSocket::bind("127.0.0.1:12000").unwrap();
        // build our quinn endpoint
        let quinn = setup_quinn();
        // build our transport object
        Transport {
            udp_sock,
            quinn,
            conn_map: HashMap::with_capacity(100),
        }
    }

    /// Transmit any queued data
    ///
    /// # Arguments
    ///
    /// * `conn` - The connection to transmit data for
    /// * `addr` - The addr to transmit data too
    async fn poll_transmit(&mut self, conn: &mut Connection, addr: &SocketAddr) {
        // check if we have any data queued to transmit
        while let Some(transmit) = conn.poll_transmit(Instant::now(), 10) {
            // transmit any queued data for this connection
            self.udp_sock
                .send_to(&transmit.contents, &addr)
                .await
                .unwrap();
        }
    }

    /// Poll This connection for different events to handle
    ///
    /// # Arguments
    ///
    /// * `handle` - The handle to the connection to poll
    /// * `conn` - The connection to poll for events
    /// * `addr` - The address of the connection to poll
    async fn poll(&mut self, handle: ConnectionHandle, conn: &mut Connection, addr: &SocketAddr) {
        // transmit any queued data
        self.poll_transmit(conn, addr).await;
        // check if any timeouts our set
        if let Some(timeout) = conn.poll_timeout() {
            // check if we need to handle this timeout
            if timeout < Instant::now() {
                // handle this timeout
                conn.handle_timeout(timeout);
            }
        }
        // check if we have any endpoint events to handle
        if let Some(endpoint_event) = conn.poll_endpoint_events() {
            // handle this endpoint event and get any resulting connection events
            if let Some(conn_event) = self.quinn.handle_event(handle, endpoint_event) {
                // handle our connection event
                conn.handle_event(conn_event);
                // transmit any queued data
                self.poll_transmit(conn, addr).await;
            }
        }
        // Poll for application events
        if let Some(event) = conn.poll() {
            // break this event into the different types
            // Maybe I need to send an acceptance to the client?
            // I think I should be getting stream events here?
            match event {
                quinn_proto::Event::HandshakeDataReady => {
                    println!("HandShakeDataReady");
                }
                quinn_proto::Event::Connected => {
                    println!("Connected")
                }
                quinn_proto::Event::ConnectionLost { reason } => {
                    println!("Conn List -> {:?}", reason)
                }
                quinn_proto::Event::Stream(stream_event) => {
                    println!("stream -> {:?}", stream_event)
                }
                quinn_proto::Event::DatagramReceived => {
                    println!("datagram rcv")
                }
            }
        }
    }

    /// Accept all brand new streams
    async fn accept_streams(&mut self, conn: &mut Connection) {
        // build a list to store our stream ids
        let mut stream_ids = Vec::default();
        // accept any pending streams and get thier ids
        while let Some(stream_id) = conn.streams().accept(quinn_proto::Dir::Bi) {
            // log our stream and save its id
            println!("STREAM FOUND! -> {}", stream_id);
            stream_ids.push(stream_id);
        }
        // crawl over our streams and read any data from them
        for id in stream_ids {
            // get the reciever for this stream
            let mut reciever = conn.recv_stream(id);
            // get a chunk stream for this reciever
            let mut chunks = reciever.read(false).unwrap();
            // read chunks until no more exist
            while let Some(chunk) = chunks.next(usize::MAX).unwrap() {
                // print our our chunk for now
                println!("chunk -> {:?}", chunk);
            }
        }
    }

    /// Handle a datagram event from quinn
    ///
    /// # Arguments
    ///
    /// * `addr` - The address this datagram event comes from
    /// * `handle` - The connection handle this datagram event is for
    /// * `data_event` - The datagram event to handle
    async fn handle_datagram(
        &mut self,
        addr: &SocketAddr,
        handle: ConnectionHandle,
        data_event: DatagramEvent,
    ) {
        // if we found a datagram event then handle it
        match data_event {
            quinn_proto::DatagramEvent::NewConnection(conn) => {
                self.conn_map.insert(handle, conn);
            }
            quinn_proto::DatagramEvent::ConnectionEvent(event) => {
                // get the connection tied to this handle
                if let Some(mut conn) = self.conn_map.remove(&handle) {
                    // handle this event
                    conn.handle_event(event);
                    // poll this connection for events to handle
                    self.poll(handle, &mut conn, &addr).await;
                    // check if any new streams were found
                    self.accept_streams(&mut conn).await;
                    // add our connection back to our map
                    self.conn_map.insert(handle, conn);
                }
            }
        }
    }

    /// try to read any quic packets from our socket
    pub async fn read(&mut self) {
        // continously read packets from our udp socket
        loop {
            // get a new buffer to write too
            let mut buff = BytesMut::zeroed(8192);
            // read a single datagram from our udp socket
            let (count, addr) = self.udp_sock.recv_from(&mut buff).await.unwrap();
            println!("read {} from {}", count, addr);
            // feed this datagram to quinn
            let read = self
                .quinn
                .handle(Instant::now(), addr, None, None, buff.clone());
            // check if got a quinn datagram event
            if let Some((handle, data_event)) = read {
                // handle this quinn datagram event
                self.handle_datagram(&addr, handle, data_event).await;
            }
        }
    }
}
