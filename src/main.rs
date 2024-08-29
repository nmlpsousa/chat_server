use async_channel::Sender;
use dotenvy::dotenv;
use futures::{SinkExt, StreamExt};
use log::{debug, error, info};
use std::collections::HashMap;
use std::error::Error;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tokio_util::codec::{Framed, LinesCodec};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();

    env_logger::init();

    let socket_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 8080));
    let listener = TcpListener::bind(socket_addr).await?;

    info!("Listening to {socket_addr}");

    let shared = Arc::new(RwLock::new(Shared::new()));

    loop {
        let (stream, socket_addr) = listener.accept().await?;
        handle_client(stream, socket_addr, shared.clone()).await;
    }
}

async fn handle_client(stream: TcpStream, socket_addr: SocketAddr, shared: Arc<RwLock<Shared>>) {
    let mut stream = Framed::new(stream, LinesCodec::new());
    let (tx, rx) = async_channel::unbounded();

    shared.write().await.clients.insert(socket_addr, tx.clone());

    debug!("{shared:?}");

    tokio::spawn(async move {
        if let Err(error) = stream.send("Hello from the server!").await {
            error!("Error while sending message: {error}");
        }

        loop {
            tokio::select! {
                Ok(s) = rx.recv() => {
                    if let Err(err) = stream.send(s).await {
                        error!("{err:?}");
                    }
                }
                result = stream.next() => match result {
                    Some(Ok(msg)) => {
                        shared.read().await.broadcast_message(msg, socket_addr).await;
                    }
                    Some(Err(_)) | None => break,
                }
            }
        }

        let mut shared_lock = shared.write().await;
        shared_lock.clients.remove(&socket_addr);
        debug!("{:?}", shared_lock.clients);
    });
}

#[derive(Debug)]
struct Shared {
    clients: HashMap<SocketAddr, Sender<String>>,
}

impl Shared {
    fn new() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }
    async fn broadcast_message(&self, message: String, sender: SocketAddr) {
        for (entry, tx) in &self.clients {
            if entry == &sender {
                continue;
            }

            if let Err(err) = tx.send(message.clone()).await {
                error!("{err:?}");
            }
        }
    }
}
