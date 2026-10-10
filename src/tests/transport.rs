use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;

fn request(url: String) -> HttpRequest {
    HttpRequest {
        url,
        key: Zeroizing::new("fixture-key".into()),
        body: Private::new(b"{}".to_vec()),
    }
}
fn exchange(response: Vec<u8>) -> Result<HttpResponse, Error> {
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = server.local_addr().unwrap();
    let thread = std::thread::spawn(move || {
        let (mut socket, _) = server.accept().unwrap();
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut wire = Vec::new();
        let mut chunk = [0; 1024];
        while !wire.ends_with(b"\r\n\r\n{}") {
            let n = socket.read(&mut chunk).unwrap();
            assert!(n > 0);
            wire.extend_from_slice(&chunk[..n]);
        }
        let headers = String::from_utf8(wire).unwrap().to_ascii_lowercase();
        assert!(headers.starts_with("post /face/v1.2/verify http/1.1"));
        assert!(headers.contains("ocp-apim-subscription-key: fixture-key"));
        assert!(headers.contains("content-type: application/json"));
        let _ = socket.write_all(&response);
    });
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let result = runtime.block_on(async {
        // Test-only local HTTP client: production construction remains HTTPS-only.
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .no_proxy()
            .build()
            .unwrap();
        ReqwestTransport(client)
            .send(request(format!("http://{address}/face/v1.2/verify")))
            .await
    });
    thread.join().unwrap();
    result
}
#[test]
fn actual_reqwest_transport_handles_status_bodies_and_truncation_without_provider_calls() {
    let response =
        exchange(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}".to_vec())
            .unwrap();
    assert_eq!(response.status, 200);
    assert_eq!(response.body.expose(), b"{}");
    let headers: Vec<(String, Vec<u8>)> =
        serde_json::from_slice(response.headers.expose()).unwrap();
    assert!(
        headers
            .iter()
            .any(|(name, value)| name == "content-length" && value == b"2")
    );
    let response = exchange(
        b"HTTP/1.1 429 Too Many Requests\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            .to_vec(),
    )
    .unwrap();
    assert_eq!(response.status, 429);
    assert!(matches!(
        exchange(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\n{}".to_vec()),
        Err(Error::Uncertain)
    ));
    let mut response =
        b"HTTP/1.1 200 OK\r\nContent-Length: 1048577\r\nConnection: close\r\n\r\n".to_vec();
    response.resize(response.len() + 1_048_577, b'x');
    assert!(matches!(exchange(response), Err(Error::Uncertain)));
}
#[test]
fn transport_rejects_invalid_credentials_and_unavailable_connections() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let transport = ReqwestTransport::new().unwrap();
        let mut bad = request("https://127.0.0.1:1".into());
        bad.key = Zeroizing::new("invalid\nkey".into());
        assert!(matches!(
            transport.send(bad).await,
            Err(Error::Configuration)
        ));
        assert!(matches!(
            transport.send(request("http://127.0.0.1:1".into())).await,
            Err(Error::Uncertain)
        ));
    });
}
