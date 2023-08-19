use quinn::{ClientConfig, Endpoint};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    // build the addr to connect too
    let addr_v4 = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 0);
    let addr = std::net::SocketAddr::V4(addr_v4);
    // build our root cert store
    let mut roots = rustls::RootCertStore::empty();
    // add our ca cert
    roots
        .add(&rustls::Certificate(
            std::fs::read("../server/cert.der").unwrap(),
        ))
        .unwrap();
    // configure our client
    let client_crypto = rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(roots)
        .with_no_client_auth();
    // build the client config
    let config = ClientConfig::new(Arc::new(client_crypto));
    // build a deafult quinn endpoint
    let mut client = Endpoint::client(addr.clone()).unwrap();
    // set our client config
    client.set_default_client_config(config);
    let addr_v4 = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 12000);
    let addr = std::net::SocketAddr::V4(addr_v4);
    // connect to our server
    let conn = client.connect(addr, "localhost").unwrap().await.unwrap();
    // open a bidirectional stream
    let (mut send, _recv) = conn.open_bi().await.unwrap();
    // send hello world over our stream
    send.write_all("Hello, World!".as_bytes()).await.unwrap();
    println!("Timeout happens here");
    // close our stream
    send.finish().await.unwrap();
    // close our connection
    conn.close(0u32.into(), b"done");
    // wait for everything to close
    client.wait_idle().await;
}
