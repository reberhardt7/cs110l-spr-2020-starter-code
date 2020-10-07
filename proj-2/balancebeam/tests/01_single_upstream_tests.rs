mod common;

use common::{init_logging, BalanceBeam, EchoServer, Server};
use std::sync::Arc;

async fn setup() -> (BalanceBeam, EchoServer) {
    init_logging();
    let upstream = EchoServer::new().await;
    let balancebeam = BalanceBeam::new(&[&upstream.address], None, None).await;
    (balancebeam, upstream)
}

/// Test the simple case: open a few connections, each with only a single request, and make sure
/// things are delivered correctly.
#[tokio::test]
async fn test_simple_connections() {
    let (balancebeam, upstream) = setup().await;

    log::info!("Sending a GET request");
    let response_text = balancebeam
        .get("/first_url")
        .await
        .expect("Error sending request to balancebeam");
    assert!(response_text.contains("GET /first_url HTTP/1.1"));
    assert!(response_text.contains("x-sent-by: balancebeam-tests"));
    assert!(response_text.contains("x-forwarded-for: 127.0.0.1"));

    log::info!("Sending a POST request");
    let response_text = balancebeam
        .post("/first_url", "Hello world!")
        .await
        .expect("Error sending request to balancebeam");
    assert!(response_text.contains("POST /first_url HTTP/1.1"));
    assert!(response_text.contains("x-sent-by: balancebeam-tests"));
    assert!(response_text.contains("x-forwarded-for: 127.0.0.1"));
    assert!(response_text.contains("\n\nHello world!"));

    log::info!("Checking that the origin server received 2 requests");
    let num_requests_received = Box::new(upstream).stop().await;
    assert_eq!(
        num_requests_received, 2,
        "Upstream server did not receive the expected number of requests"
    );

    log::info!("All done :)");
}

/// Test handling of multiple HTTP requests per connection to the server. Open three concurrent
/// connections, and send four requests on each.
#[tokio::test]
async fn test_multiple_requests_per_connection() {
    let num_connections = 3;
    let requests_per_connection = 4;

    let (balancebeam, upstream) = setup().await;
    let balancebeam_shared = Arc::new(balancebeam);

    let mut tasks = Vec::new();
    for task_num in 0..num_connections {
        let balancebeam_shared = balancebeam_shared.clone();
        tasks.push(tokio::task::spawn(async move {
            let client = reqwest::Client::new();
            for req_num in 0..requests_per_connection {
                log::info!(
                    "Task {} sending request {} (connection {})",
                    task_num,
                    req_num,
                    task_num
                );
                let path = format!("/conn-{}/req-{}", task_num, req_num);
                let response_text = client
                    .get(&format!("http://{}{}", balancebeam_shared.address, path))
                    .header("x-sent-by", "balancebeam-tests")
                    .send()
                    .await
                    .expect("Failed to connect to balancebeam")
                    .text()
                    .await
                    .expect("Balancebeam replied with a malformed response");
                assert!(response_text.contains(&format!("GET {} HTTP/1.1", path)));
            }
        }));
    }

    for join_handle in tasks {
        join_handle.await.expect("Task panicked");
    }

    log::info!(
        "Checking that the origin server received {} requests",
        num_connections * requests_per_connection
    );
    let num_requests_received = Box::new(upstream).stop().await;
    assert_eq!(
        num_requests_received,
        num_connections * requests_per_connection,
        "Upstream server did not receive the expected number of requests"
    );

    log::info!("All done :)");
}
