//! Handles the certs used to protect traffic to and from Shoal
use std::path::Path;

use rustls::{Certificate, PrivateKey};

pub fn generate() -> (Vec<Certificate>, PrivateKey) {
    let cert_path = Path::new("cert.der");
    let key_path = Path::new("key.der");
    let (cert, key) =
        match std::fs::read(&cert_path).and_then(|x| Ok((x, std::fs::read(&key_path)?))) {
            Ok(x) => x,
            Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => {
                println!("generating self-signed certificate");
                let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
                let key = cert.serialize_private_key_der();
                let cert = cert.serialize_der().unwrap();
                std::fs::write(&cert_path, &cert).unwrap();
                std::fs::write(&key_path, &key).unwrap();
                (cert, key)
            }
            Err(e) => {
                panic!("failed to read certificate: {}", e);
            }
        };

    let key = rustls::PrivateKey(key);
    let cert = rustls::Certificate(cert);
    (vec![cert], key)
}
