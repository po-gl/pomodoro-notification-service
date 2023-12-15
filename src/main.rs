// A simple service to message Apple Push Notifications service (APNs) in
// a series of intervals for an iOS Pomodoro app -- particularly for liveactivites.
mod authtoken;
mod util;
mod types;
mod apns;
mod models;
mod routes;
mod timing;

use actix_web::{HttpResponse, HttpServer, App, web::{self, Data}, error};
use dotenv::dotenv;
use log::{info, error};
use std::{sync::Arc, process::exit, env, collections::HashMap};
use tokio::{sync::RwLock, time::Duration};

use util::{HOST, PORT, VAR_APNS_HOST_NAME, VAR_TOPIC, VAR_TEAM_ID, VAR_AUTH_KEY_ID, VAR_TOKEN_KEY_PATH};
use types::{CancelMap, PushTokenMap};
use routes::{request, update_push_token, cancel_request, health};
use authtoken::AuthToken;

pub const STRESS_TEST: bool = false;

pub const LOG_CONFIG_PATH: &str = "log4rs.yaml";

const AUTH_TOKEN_REFRESH_RATE_S: u64 = 60 * 50; // Needs refresh between 20-60 minutes

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    let check = util::check_environment_vars();
    if check.is_err() {
        eprintln!("Missing environment variable");
        eprintln!("Required environment variables: {VAR_TOPIC} {VAR_TEAM_ID} {VAR_AUTH_KEY_ID} {VAR_TOKEN_KEY_PATH} {VAR_APNS_HOST_NAME}");
        exit(1)
    }
    util::init_logging();

    let auth_token = Arc::new(RwLock::new(AuthToken::new()));
    let auth_data = Data::new(auth_token.clone());
    info!("Initial auth token: {}", &auth_token.read().await.token);

    let push_token_map: PushTokenMap = HashMap::new();
    let push_token_map_data = Data::new(Arc::new(RwLock::new(push_token_map)));

    let cancel_channels: CancelMap = HashMap::new();
    let cancel_channels_data = Data::new(Arc::new(RwLock::new(cancel_channels)));

    let refresh_loop_handle = tokio::spawn(auth_token_refresh_loop(Arc::clone(&auth_token)));

    let host = env::var(HOST).unwrap_or(String::from("127.0.0.1"));
    let port = env::var(PORT).unwrap_or(String::from("9898"));

    let server_handle = HttpServer::new(move || {
        let json_cfg = web::JsonConfig::default()
            .error_handler(|err, _req| {
                error!("Json config error: {}", err);
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
            .service(health)
    })
        .bind(format!("{}:{}", host, port))?
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
            Ok(_) => info!("AuthToken refreshed sucessfully"),
            Err(e) => error!("AuthToken refresh error {:?}", e),
        }
    }
}

