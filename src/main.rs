// A simple service to message Apple Push Notifications service (APNs) in
// a series of intervals for an iOS Pomodoro app -- particularly for liveactivites.
mod authtoken;
mod util;

use actix_web::{Responder, HttpResponse, HttpServer, App, post, web::{self, Data}, error};
use dotenv::dotenv;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::Deserialize;
use serde_json::json;
use util::{VAR_APNS_HOST_NAME, VAR_TOPIC, VAR_TEAM_ID, VAR_AUTH_KEY_ID, VAR_TOKEN_KEY_PATH};
use std::{sync::Arc, time::{SystemTime, UNIX_EPOCH}, process::exit, env, collections::HashMap};
use tokio::sync::{mpsc, RwLock};
use tokio::sync::mpsc::Sender;
use tokio::time::Duration;
use authtoken::AuthToken;

const STRESS_TEST: bool = false;

const AUTH_TOKEN_REFRESH_RATE_S: u64 = 60 * 50; // Needs refresh between 20-60 minutes
const HOST_ADDR: &str = "127.0.0.1:9797";

type CancelMap = HashMap<String, Sender<bool>>;

/// <device_token, push_token>
type PushTokenMap = HashMap<String, String>;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    let check = util::check_environment_vars();
    if check.is_err() {
        eprintln!("Missing environment variable");
        eprintln!("Required environment variables: {VAR_TOPIC} {VAR_TEAM_ID} {VAR_AUTH_KEY_ID} {VAR_TOKEN_KEY_PATH} {VAR_APNS_HOST_NAME}");
        exit(1)
    }

    let auth_token = Arc::new(RwLock::new(AuthToken::new()));
    let auth_data = Data::new(auth_token.clone());
    println!("Initial auth token: {}", &auth_token.read().await.token);

    let push_token_map: PushTokenMap = HashMap::new();
    let push_token_map_data = Data::new(Arc::new(RwLock::new(push_token_map)));

    let cancel_channels: CancelMap = HashMap::new();
    let cancel_channels_data = Data::new(Arc::new(RwLock::new(cancel_channels)));

    let refresh_loop_handle = tokio::spawn(auth_token_refresh_loop(Arc::clone(&auth_token)));

    let server_handle = HttpServer::new(move || {
        let json_cfg = web::JsonConfig::default()
            .error_handler(|err, _req| {
                println!("Json config error: {}", err);
                error::InternalError::from_response(err, HttpResponse::Conflict().into()).into()
            });
        App::new()
            .app_data(Data::clone(&auth_data))
            .app_data(Data::clone(&push_token_map_data))
            .app_data(Data::clone(&cancel_channels_data))
            .app_data(json_cfg)
            .service(request)
            .service(update_push_token)
            .service(cancel_request)
    })
        .bind(HOST_ADDR)?
        .run();

    tokio::select! {
        _ = server_handle => {}
        _ = refresh_loop_handle => {},
    }
    Ok(())
}

async fn auth_token_refresh_loop(auth_token: Arc<RwLock<AuthToken>>) {
    loop {
        tokio::time::sleep(Duration::from_secs(AUTH_TOKEN_REFRESH_RATE_S)).await;
        let result = auth_token.write().await.refresh();
        match result {
            // TODO: change prints to logs w/ timestamps
            Ok(_) => println!("AuthToken refreshed sucessfully"),
            Err(e) => println!("AuthToken refresh error {:?}", e),
        }
    }
}


#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct RequestData {
    time_intervals: Vec<TimerInterval>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct TimerInterval {
    status: String,
    task: String,
    starts_at: f64,
    current_segment: u32,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct PushTokenData {
    push_token: String,
}

#[post("/request/{device_token}")]
async fn request(device_token: web::Path<String>,
    payload: web::Json<RequestData>,
    auth_token: web::Data<Arc<RwLock<AuthToken>>>,
    cancel_channels: web::Data<Arc<RwLock<CancelMap>>>,
    push_token_map: web::Data<Arc<RwLock<PushTokenMap>>>) -> impl Responder {

    tokio::spawn(async move {
        let time_intervals = payload.time_intervals.clone();
        let mut current_time: Duration;
        let mut target_time: Duration;
        let mut wait_time: Duration;

        let (tx, mut rx) = mpsc::channel(1);
        { 
            let mut cancel_map = cancel_channels.write().await;
            if let Some(existing_cancel) = cancel_map.get(device_token.as_ref()) {
                existing_cancel.send(true).await.ok();
            }
            cancel_map.insert(device_token.clone(), tx);
        }

        for time_interval in time_intervals {
            target_time = Duration::from_secs_f64(time_interval.starts_at);
            current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
            if target_time < current_time {
                wait_time = Duration::from_secs(0)
            } else {
                wait_time = target_time - current_time
            }

            let sleep_handle = tokio::time::sleep(dbg!(wait_time));
            let cancel_handle = rx.recv();

            tokio::select! {
                _ = sleep_handle => {}
                _ = cancel_handle => {
                    println!("Request canceled for device: {}", device_token.as_ref());
                    break;
                }
            }

            let auth = &auth_token.read().await.token;
            if let Some(push_token) = push_token_map.read().await.get(device_token.as_ref()) {
                send_la_update_to_apns(push_token, auth, &time_interval).await;
            }
        }
    });
    HttpResponse::Ok()
}

#[post("/pushtoken/{device_token}")]
async fn update_push_token(device_token: web::Path<String>,
    payload: web::Json<PushTokenData>,
    push_token_map: web::Data<Arc<RwLock<PushTokenMap>>>) -> impl Responder {

    push_token_map.write().await.insert(device_token.clone(), payload.push_token.clone());
    println!("Updating push_token (push_token_map size is now {}) {}", push_token_map.read().await.len(), payload.push_token);
    HttpResponse::Ok()
}

#[post("/cancel/{device_token}")]
async fn cancel_request(device_token: web::Path<String>,
    cancel_channels: web::Data<Arc<RwLock<CancelMap>>>) -> impl Responder {

    if let Some(cancel) = cancel_channels.read().await.get(device_token.as_ref()) {
        cancel.send(true).await.ok();
    }
    cancel_channels.write().await.remove(device_token.as_ref());
    HttpResponse::Ok()
}

/// Send a live activity update to APNs
async fn send_la_update_to_apns(token: &String, auth_token: &String, timer_interval: &TimerInterval) {
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
                "startTimestamp": now,
                "timeRemaining": 0,
                "isFullSegment": true,
                "isPaused": false,
            },
            "alert": {
                "title": format!("Time to {}", timer_interval.status),
                "body": "test body",
            }
        }
    }).to_string();
    println!("Body: {body}");

    headers.insert("content-length", HeaderValue::from_str(body.as_bytes().len().to_string().as_str()).unwrap());

    let result = client.post(url)
        .headers(headers)
        .body(body)
        .send()
    .await;

    match result {
        Ok(res) => {
            let blank_header = HeaderValue::from_static("");
            let apns_id = res.headers().get("apns-id").unwrap_or(&blank_header).to_str().unwrap_or_default();
            let apns_unique_id = res.headers().get("apns-unique-id").unwrap_or(&blank_header).to_str().unwrap_or_default();
            println!("APNs response: status={}, apns-id={}, apns-unique-id={}",res.status(), apns_id, apns_unique_id);
        },
        Err(e) => eprintln!("APNs error: {e}"),
    }
}
