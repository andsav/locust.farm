//! Farm uploads run outside the engine thread. The engine owns durable intent;
//! dropping an HTTP future during shutdown leaves that intent available to retry.

use std::collections::BTreeMap;
use std::sync::mpsc::Sender;
use std::time::Duration;

use locust_proto::farm::{FarmOperation, FarmReceipt, FarmUpload, FarmUploadResult};
use reqwest::{Client, Method, Url};
use tokio::sync::{oneshot, watch};
use tokio::task::JoinSet;

use super::worker::Job;

// These bound one service exchange, never agent work. A failed exchange remains
// durable in the engine and follows its retry schedule.
const EXCHANGE_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_RECEIPT_BYTES: usize = 64 * 1024;

pub(super) async fn serve(jobs: Sender<Job>, mut stop: watch::Receiver<bool>) {
    let client = match Client::builder()
        .timeout(EXCHANGE_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .build()
    {
        Ok(client) => client,
        Err(_) => {
            super::log(format_args!(
                "locust: farm HTTP client could not initialize"
            ));
            return;
        }
    };
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut running = JoinSet::new();
    let mut active = BTreeMap::new();
    loop {
        tokio::select! {
            biased;
            changed = stop.changed() => {
                if changed.is_err() || *stop.borrow() { break; }
            }
            result = running.join_next_with_id(), if !running.is_empty() => {
                match result {
                    Some(Ok((id, result))) => {
                        active.retain(|_, task| *task != id);
                        if jobs.send(Job::FarmComplete { result }).is_err() { break; }
                    }
                    Some(Err(error)) => {
                        active.retain(|_, task| *task != error.id());
                        super::log(format_args!("locust: farm exchange task ended before acknowledgment"));
                    }
                    None => {}
                }
            }
            _ = tick.tick() => {
                if *stop.borrow() { break; }
                let (reply, received) = oneshot::channel();
                if jobs.send(Job::FarmPoll { reply }).is_err() { break; }
                let uploads = tokio::select! {
                    biased;
                    _ = stop.changed() => break,
                    result = received => match result {
                        Ok(uploads) => uploads,
                        Err(_) => break,
                    }
                };
                for upload in uploads {
                    let key = upload.request.farm_id.clone();
                    if active.contains_key(&key) { continue; }
                    let client = client.clone();
                    let handle = running.spawn(async move { exchange(&client, upload).await });
                    active.insert(key, handle.id());
                }
            }
        }
    }
    running.abort_all();
    while running.join_next().await.is_some() {}
}

fn target(upload: &FarmUpload) -> Result<(Method, Url), String> {
    let mut base = locust_proto::farm::service_origin(&upload.base_url)?;
    let (method, suffix) = match upload.request.operation {
        FarmOperation::Upload => (Method::PUT, ""),
        FarmOperation::CheckIn => (Method::POST, "/check-in"),
        FarmOperation::Suspend => (Method::POST, "/suspend"),
        FarmOperation::Delete => (Method::DELETE, ""),
    };
    base.set_path(&format!("/api/farms/{}{suffix}", upload.request.farm_id));
    Ok((method, base))
}

async fn exchange(client: &Client, upload: FarmUpload) -> FarmUploadResult {
    let outcome = send(client, &upload).await;
    FarmUploadResult {
        goal: upload.goal,
        farm_id: upload.request.farm_id,
        sequence: upload.request.sequence,
        outcome,
    }
}

async fn send(client: &Client, upload: &FarmUpload) -> Result<FarmReceipt, String> {
    let (method, url) = target(upload)?;
    let mut response = client
        .request(method, url)
        .json(&upload.request)
        .send()
        .await
        .map_err(|_| "farm service exchange failed".to_owned())?;
    if !response.status().is_success() {
        return Err(format!(
            "farm service returned HTTP {}",
            response.status().as_u16()
        ));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "farm service receipt could not be read".to_owned())?
    {
        if bytes.len().saturating_add(chunk.len()) > MAX_RECEIPT_BYTES {
            return Err("farm service receipt exceeds its size limit".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    let receipt: FarmReceipt = serde_json::from_slice(&bytes)
        .map_err(|_| "farm service returned an invalid receipt".to_owned())?;
    if receipt.farm_id != upload.request.farm_id
        || receipt.sequence != upload.request.sequence
        || receipt.request_digest != upload.request.request_digest()
    {
        return Err("farm service returned a receipt for another request".into());
    }
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use locust_proto::{crypto::Keypair, farm::SignedFarmRequest, id::GoalId};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn upload(base_url: &str, operation: FarmOperation) -> FarmUpload {
        FarmUpload {
            goal: GoalId([7; 32]),
            base_url: base_url.to_owned(),
            request: SignedFarmRequest::sign(
                &Keypair::from_seed([9; 32]),
                operation,
                3,
                "{}".into(),
            ),
        }
    }

    #[test]
    fn only_explicit_secure_origins_or_loopback_are_used() {
        for origin in [
            "https://locust.farm",
            "http://127.0.0.1:4319",
            "http://[::1]:4319",
            "http://localhost:4319",
        ] {
            assert!(
                target(&upload(origin, FarmOperation::Upload)).is_ok(),
                "{origin}"
            );
        }
        for origin in [
            "http://example.com",
            "https://user:secret@example.com",
            "https://example.com/path",
            "https://example.com?token=secret",
            "https://example.com/#fragment",
            "file:///tmp/service",
        ] {
            assert!(
                target(&upload(origin, FarmOperation::Upload)).is_err(),
                "{origin}"
            );
        }
        for (operation, method, suffix) in [
            (FarmOperation::Upload, Method::PUT, ""),
            (FarmOperation::CheckIn, Method::POST, "/check-in"),
            (FarmOperation::Suspend, Method::POST, "/suspend"),
            (FarmOperation::Delete, Method::DELETE, ""),
        ] {
            let value = upload("https://locust.farm", operation);
            let (actual_method, url) = target(&value).unwrap();
            assert_eq!(actual_method, method);
            assert_eq!(
                url.path(),
                format!("/api/farms/{}{suffix}", value.request.farm_id)
            );
        }
    }

    async fn mock(
        status: &str,
        response: String,
    ) -> (String, tokio::task::JoinHandle<SignedFarmRequest>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let status = status.to_owned();
        let task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let (body_start, length) = loop {
                let mut chunk = [0; 4096];
                let read = stream.read(&mut chunk).await.unwrap();
                assert_ne!(read, 0);
                bytes.extend_from_slice(&chunk[..read]);
                if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..end]);
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            let (key, value) = line.split_once(':')?;
                            key.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().unwrap())
                        })
                        .unwrap();
                    break (end + 4, length);
                }
            };
            while bytes.len() < body_start + length {
                let mut chunk = [0; 4096];
                let read = stream.read(&mut chunk).await.unwrap();
                assert_ne!(read, 0);
                bytes.extend_from_slice(&chunk[..read]);
            }
            let request = serde_json::from_slice(&bytes[body_start..body_start + length]).unwrap();
            stream.write_all(format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}", response.len()).as_bytes()).await.unwrap();
            request
        });
        (format!("http://{address}"), task)
    }

    #[tokio::test]
    async fn sends_exact_signed_intent_and_accepts_only_its_receipt() {
        let client = Client::builder().no_proxy().build().unwrap();
        let mut value = upload("http://localhost", FarmOperation::Delete);
        let receipt = FarmReceipt {
            farm_id: value.request.farm_id.clone(),
            sequence: 3,
            request_digest: value.request.request_digest(),
            received_at_ms: 400,
            stream_version: 2,
        };
        let (origin, task) = mock("200 OK", serde_json::to_string(&receipt).unwrap()).await;
        value.base_url = origin;
        assert_eq!(send(&client, &value).await.unwrap(), receipt);
        assert_eq!(task.await.unwrap(), value.request);
        for changed in [
            FarmReceipt {
                sequence: 4,
                ..receipt.clone()
            },
            FarmReceipt {
                request_digest: "0".repeat(64),
                ..receipt.clone()
            },
        ] {
            let (origin, task) = mock("200 OK", serde_json::to_string(&changed).unwrap()).await;
            value.base_url = origin;
            assert!(
                send(&client, &value)
                    .await
                    .unwrap_err()
                    .contains("another request")
            );
            task.await.unwrap();
        }
    }

    #[tokio::test]
    async fn service_errors_do_not_copy_private_response_text() {
        let client = Client::builder().no_proxy().build().unwrap();
        let (origin, task) = mock(
            "413 Content Too Large",
            "untrusted private diagnostics".into(),
        )
        .await;
        let error = send(&client, &upload(&origin, FarmOperation::Upload))
            .await
            .unwrap_err();
        assert_eq!(error, "farm service returned HTTP 413");
        task.await.unwrap();
    }
}
