mod shard;
mod transport;

use glommio::LocalExecutorBuilder;
use transport::Transport;

fn main() {
    // build a default glommio executor and then spawn it
    let executor = LocalExecutorBuilder::default()
        .spawn(|| async move {
            //build a new transport object
            let mut transport = Transport::new();
            // start reading from our quic based transport
            transport.read().await;
        })
        .unwrap();
    // wait for all threads on our executor to finish
    executor.join().unwrap();
}
