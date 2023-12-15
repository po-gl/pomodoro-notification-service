use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::json;
use std::{time::{SystemTime, UNIX_EPOCH}, env};
use tokio::time::Duration;

use crate::util::{VAR_APNS_HOST_NAME, VAR_TOPIC};
use crate::models::TimerInterval;

use crate::STRESS_TEST;

/// Send a live activity update to APNs
pub async fn send_la_update_to_apns(token: &String, auth_token: &String, timer_interval: &TimerInterval, segment_count: u32) {
    if STRESS_TEST {
        println!("Simulated request to apns (STRESS_TEST=true)");
        tokio::time::sleep(Duration::from_secs(1)).await;
        return;
    }

    let client = reqwest::Client::builder()
        .http2_prior_knowledge()
        .build().unwrap();
    let url = format!("https://{}/3/device/{}", env::var(VAR_APNS_HOST_NAME).unwrap(), token);

    let mut headers = HeaderMap::new();
    headers.insert("apns-topic", HeaderValue::from_str(format!("{}.push-type.liveactivity", env::var(VAR_TOPIC).unwrap()).as_str()).unwrap());
    headers.insert("apns-push-type", HeaderValue::from_static("liveactivity"));
    headers.insert("apns-priority", HeaderValue::from_static("10"));
    headers.insert("authorization", HeaderValue::from_str(format!("bearer {}", auth_token).as_str()).unwrap());
    headers.insert("content-type", HeaderValue::from_static("application/json"));

    let body = get_apns_body(timer_interval, segment_count);
    println!("Body: {body}");

    let content_length = body.as_bytes().len();
    headers.insert("content-length", HeaderValue::from_str(content_length.to_string().as_str()).unwrap());

    let result = client.post(url)
        .headers(headers)
        .body(body)
        .send()
    .await;

    match result {
        Ok(res) => {
            let status = res.status().clone();
            let headers = res.headers().clone();

            let blank_header = HeaderValue::from_static("");
            let apns_id = headers.get("apns-id").unwrap_or(&blank_header).to_str().unwrap_or_default();
            let apns_unique_id = headers.get("apns-unique-id").unwrap_or(&blank_header).to_str().unwrap_or_default();

            let body = res.text().await.unwrap_or_default();
            println!("APNs response: status={}, apns-id={}, apns-unique-id={}, {}", status, apns_id, apns_unique_id, body);
        },
        Err(e) => eprintln!("APNs error: {e}"),
    }
}

fn get_apns_body(timer_interval: &TimerInterval, segment_count: u32) -> String {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let body = json!({
        "aps": {
            "timestamp": now,
            "event": "update",
            "dismissal-date": now + 5 * 60,
            "content-state": {
                "status": timer_interval.status,
                "task": timer_interval.task,
                "currentSegment": timer_interval.current_segment,
                "segmentCount": segment_count,
                "startTimestamp": timer_interval.starts_at,
                "timeRemaining": 0,
                "isFullSegment": true,
                "isPaused": false,
            },
            "alert": {
                "title": timer_interval.alert.title,
                "body": timer_interval.alert.body,
                "sound": timer_interval.alert.sound,
            }
        }
    }).to_string();
    return body;
}
