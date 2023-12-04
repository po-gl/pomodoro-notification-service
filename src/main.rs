// A simple service to message Apple Push Notifications service (APNs) in
// a series of intervals for an iOS Pomodoro app -- particularly for liveactivites.
mod authtoken;
mod util;

use actix_web::{Responder, HttpResponse, HttpServer, App, post, web::{self, Data}, error};
use dotenv::dotenv;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::Deserialize;
use util::{VAR_APNS_HOST_NAME, VAR_TOPIC, VAR_TEAM_ID, VAR_AUTH_KEY_ID, VAR_TOKEN_KEY_PATH};
use std::{sync::Arc, time::{SystemTime, UNIX_EPOCH}, process::exit, env};
use tokio::sync::RwLock;
use tokio::time::Duration;
use authtoken::AuthToken;

const AUTH_TOKEN_REFRESH_RATE_S: u64 = 60 * 50; // Needs refresh between 20-60 minutes
const HOST_ADDR: &str = "127.0.0.1:9797";

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

    let refresh_loop_handle = tokio::spawn(auth_token_refresh_loop(Arc::clone(&auth_token)));

    let server_handle = HttpServer::new(move || {
        let json_cfg = web::JsonConfig::default()
            .error_handler(|err, _req| {
                println!("Json config error: {}", err);
                error::InternalError::from_response(err, HttpResponse::Conflict().into()).into()
            });
        App::new()
            .app_data(Data::clone(&auth_data))
            .app_data(json_cfg)
            .service(request)
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
    starts_at: f64,
}

#[post("/request/{device_token}")]
async fn request(device_token: web::Path<String>,
    payload: web::Json<RequestData>,
    auth_token: web::Data<Arc<RwLock<AuthToken>>>) -> impl Responder {

    println!("payload: {:#?}", payload);
    println!("auth_token used: {}", &auth_token.read().await.token);

    tokio::spawn(async move {
        let time_intervals = payload.clone().time_intervals;
        let mut current_time: Duration;
        let mut target_time: Duration;

        let mut i = 0;
        for time_interval in time_intervals {

            target_time = Duration::from_secs_f64(time_interval.starts_at);
            current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

            if target_time < current_time { continue; }
            tokio::time::sleep(dbg!(target_time - current_time)).await;

            let auth = &auth_token.read().await.token;
            send_request_to_apns(&device_token, auth, format!("{} {}", i, time_interval.status)).await;
            i += 1;
        }
    });
    HttpResponse::Ok()
}

#[post("/cancel")]
async fn cancel_request() -> impl Responder {
    "todo"
}

async fn send_request_to_apns(device_token: &String, auth_token: &String, msg: String) {
    let client = reqwest::Client::builder()
        .http2_prior_knowledge()
        .build().unwrap();
    let url = format!("https://{}/3/device/{}", env::var(VAR_APNS_HOST_NAME).unwrap(), device_token);

    let mut headers = HeaderMap::new();
    headers.insert("apns-topic", HeaderValue::from_str(env::var(VAR_TOPIC).unwrap().as_str()).unwrap());
    headers.insert("apns-push-type", HeaderValue::from_static("alert"));
    headers.insert("authorization", HeaderValue::from_str(format!("bearer {}", auth_token).as_str()).unwrap());
    headers.insert("content-type", HeaderValue::from_static("application/json"));

    // let body = "{\"aps\":{\"alert\":\"testing testing 1 5 7\"}}";
    let body = format!("{{\"aps\":{{\"alert\":\"test {}\"}}}}", msg);
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
