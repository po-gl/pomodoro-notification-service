use actix_web::{Responder, HttpResponse, post, get, web::{self}};
use log::{info, debug};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::{models::{RequestData, PushTokenData}, util::get_short_token};
use crate::types::{CancelMap, PushTokenMap};
use crate::authtoken::AuthToken;
use crate::timing::start_timing_loop;


#[post("/pushtoken/{device_token}")]
async fn update_push_token(device_token: web::Path<String>,
    payload: web::Json<PushTokenData>,
    push_token_map: web::Data<Arc<RwLock<PushTokenMap>>>) -> impl Responder {

    push_token_map.write().await.insert(device_token.clone(), payload.push_token.clone());
    
    debug!("push_token:: Updating device_token...{} -> push_token...{} (push_token_map size is now {})",
        get_short_token(device_token.as_ref()),
        get_short_token(&payload.push_token),
        push_token_map.read().await.len());
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

#[get("/health")]
pub async fn health() -> impl Responder {
    info!("Health check");
    HttpResponse::Ok()
}

#[post("/request/{device_token}")]
async fn request(device_token: web::Path<String>,
    payload: web::Json<RequestData>,
    auth_token: web::Data<Arc<RwLock<AuthToken>>>,
    cancel_channels: web::Data<Arc<RwLock<CancelMap>>>,
    push_token_map: web::Data<Arc<RwLock<PushTokenMap>>>) -> impl Responder {

    tokio::spawn(
        start_timing_loop(
            payload.time_intervals.clone(),
            payload.segment_count,
            device_token.to_string(),
            auth_token.as_ref().to_owned(),
            cancel_channels.as_ref().to_owned(),
            push_token_map.as_ref().to_owned()
        )
    );
    HttpResponse::Ok()
}
