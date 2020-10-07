use crate::common::server::Server;
use async_trait::async_trait;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Response};
use rand::Rng;
use std::sync::{atomic, Arc};
use tokio::sync::oneshot;

#[derive(Debug)]
struct ServerState {
    pub requests_received: atomic::AtomicUsize,
}

#[allow(dead_code)]
async fn return_error() -> Result<Response<Body>, hyper::Error> {
    Ok(Response::builder()
        .status(http::StatusCode::INTERNAL_SERVER_ERROR)
        .body(Body::empty())
        .unwrap())
}

pub struct ErrorServer {
    shutdown_signal_sender: oneshot::Sender<()>,
    server_task: tokio::task::JoinHandle<()>,
    pub address: String,
    state: Arc<ServerState>,
}

impl ErrorServer {
    #[allow(dead_code)]
    pub async fn new() -> ErrorServer {
        let mut rng = rand::thread_rng();
        ErrorServer::new_at_address(format!("127.0.0.1:{}", rng.gen_range(1024, 65535))).await
    }

    #[allow(dead_code)]
    pub async fn new_at_address(bind_addr_string: String) -> ErrorServer {
        let bind_addr = bind_addr_string.parse().unwrap();
        // Create a one-shot channel that can be used to tell the server to shut down
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        // Start a separate server task
        let server_state = Arc::new(ServerState {
            requests_received: atomic::AtomicUsize::new(0),
        });
        let server_task_state = server_state.clone();
        let server_task = tokio::spawn(async move {
            let service = make_service_fn(|_| {
                let server_task_state = server_task_state.clone();
                async move {
                    Ok::<_, hyper::Error>(service_fn(move |_req| {
                        server_task_state
                            .requests_received
                            .fetch_add(1, atomic::Ordering::SeqCst);
                        return_error()
                    }))
                }
            });
            let server = hyper::Server::bind(&bind_addr)
                .serve(service)
                .with_graceful_shutdown(async {
                    shutdown_rx.await.ok();
                });
            // Start serving and wait for the server to exit
            if let Err(e) = server.await {
                log::error!("Error in ErrorServer: {}", e);
            }
        });

        ErrorServer {
            shutdown_signal_sender: shutdown_tx,
            server_task,
            state: server_state,
            address: bind_addr_string,
        }
    }
}

#[async_trait]
impl Server for ErrorServer {
    async fn stop(self: Box<Self>) -> usize {
        // Tell the hyper server to stop
        let _ = self.shutdown_signal_sender.send(());
        // Wait for it to stop
        self.server_task
            .await
            .expect("ErrorServer server task panicked");

        self.state.requests_received.load(atomic::Ordering::SeqCst)
    }

    fn address(&self) -> String {
        self.address.clone()
    }
}
