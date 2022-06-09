use std::sync::RwLock;

use actix_web::{get, post, web, Responder};
use chrono::NaiveDateTime;
use futures::StreamExt;
use log::{debug, info};
use serde::Serialize;
use tokio::sync::mpsc;

use crate::db::BiliupDao;
use crate::recorder::RecorderEvent;

pub(crate) type RecorderEventSender = mpsc::Sender<RecorderEvent>;

#[derive(Debug, Default)]
pub struct AppState {
    pub(crate) current: RwLock<Option<String>>,
    pub(crate) uploaded: RwLock<usize>,
}

#[derive(Debug, Serialize)]
pub(crate) struct UploadState {
    pub(crate) event_id: String,

    #[serde(serialize_with = "dt_to_ts")]
    pub(crate) created_at: chrono::NaiveDateTime,

    pub(crate) relative_path: String,
    pub(crate) file_size: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct UploadHistory {
    #[serde(serialize_with = "some_dt_to_ts")]
    pub(crate) finished_at: Option<chrono::NaiveDateTime>,

    pub(crate) relative_path: String,
    pub(crate) file_size: i64,

    pub(crate) avid: Option<i64>,
}

#[derive(Debug, Serialize)]
pub(crate) struct StateResponse {
    pub current: Option<String>,
    pub uploaded: usize,
    pub uploads: Vec<UploadState>,
}

#[get("/stat")]
pub(crate) async fn status(
    state: web::Data<AppState>,
    dao: web::Data<BiliupDao>,
) -> impl Responder {
    debug!("Received status request");

    let uploads = match dao.get_unfinished_uploads().await {
        Ok(uploads) => uploads,
        Err(_) => return web::Json(None),
    };
    let response = StateResponse {
        current: (*state.current.read().unwrap()).clone(),
        uploaded: *state.uploaded.read().unwrap(),
        uploads,
    };

    web::Json(Some(response))
}

#[get("/history")]
pub(crate) async fn status_ok(dao: web::Data<BiliupDao>) -> impl Responder {
    debug!("Received history request");

    let uploads = match dao.get_finished_uploads().await {
        Ok(uploads) => Some(uploads),
        Err(_) => None,
    };

    web::Json(uploads)
}

#[post("/recorder")]
pub(crate) async fn recorder(
    mut payload: web::Payload,
    tx: web::Data<RecorderEventSender>,
    dao: web::Data<BiliupDao>,
) -> &'static str {
    let event = {
        let mut body = web::BytesMut::new();
        while let Some(chunk) = payload.next().await {
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(_) => return "Failed to read payload",
            };
            // limit max size of in-memory payload
            body.extend_from_slice(&chunk);
        }

        match serde_json::from_slice::<RecorderEvent>(&body) {
            Ok(event) => event,
            Err(_) => return "OK",
        }
    };
    info!("Received recorder event");

    if event.event_type != "FileClosed" {
        return "OK";
    }

    if event.event_data.duration < 10.0 {
        return "OK";
    }

    match dao.add_event(&event).await {
        Ok(_) => (),
        Err(_) => return "Failed",
    }

    match dao.add_upload(&event).await {
        Ok(_) => (),
        Err(_) => return "Failed",
    }

    match tx.send(event.clone()).await {
        Ok(_) => "OK",
        Err(_) => "Failed",
    }
}

#[post("/retry/{event_id}")]
pub(crate) async fn retry(
    tx: web::Data<RecorderEventSender>,
    dao: web::Data<BiliupDao>,
    path: web::Path<(String,)>,
    state: web::Data<AppState>,
) -> &'static str {
    let current = state.current.read().unwrap();
    if (*current).is_some() {
        return "Busy";
    }

    let event_id = &path.0;
    let event = match dao.get_event(event_id).await {
        Ok(event) => event,
        Err(_) => return "Failed",
    };
    let event = match event {
        Some(event) => event,
        None => return "No such event",
    };

    debug!("Retrying event: {:?}", event);

    match tx.send(event).await {
        Ok(_) => "OK",
        Err(_) => "Failed",
    }
}

pub(crate) fn dt_to_ts<S>(dt: &NaiveDateTime, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_i64(dt.timestamp())
}

pub(crate) fn some_dt_to_ts<S>(dt: &Option<NaiveDateTime>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_i64(dt.unwrap().timestamp())
}
