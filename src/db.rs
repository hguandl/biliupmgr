use sqlx::{self, SqlitePool};
use anyhow::Result;
use chrono::{NaiveDateTime, DateTime, Utc};

use crate::{recorder::{RecorderEvent, RecorderEventData}, webhook::{UploadState, UploadHistory}};

#[derive(Clone)]
pub struct BiliupDao {
    pool: SqlitePool
}

impl BiliupDao {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

// Table `events`
impl BiliupDao {
    pub async fn add_event(&self, event: &RecorderEvent) -> Result<()> {
        let mut conn = self.pool.acquire().await?;

        let room_id = event.event_data.room_id as i64;
        let file_size = event.event_data.file_size as i64;
        let file_open_time = chrono::DateTime::parse_from_rfc3339(&event.event_data.file_open_time).unwrap();
        sqlx::query!(
            "
            INSERT INTO events (event_type, event_id, room_id, name, title, relative_path, file_size, duration, file_open_time)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ",
            event.event_type, event.event_id, room_id,
            event.event_data.name, event.event_data.title, 
            event.event_data.relative_path, file_size, 
            event.event_data.duration, file_open_time
        )
        .execute(&mut conn)
        .await?;

        Ok(())
    }

    pub async fn get_event(&self, event_id: &str) -> Result<Option<RecorderEvent>> {
        struct _EventRow {
            event_id: String,
            event_type: String,
            room_id: i64,
            name: String,
            title: String,
            relative_path: String,
            file_open_time: NaiveDateTime,
            file_size: i64,
            duration: f32,
        }

        let event_row = sqlx::query_as!(
            _EventRow,
            "
            SELECT event_id, event_type, room_id, name, title, relative_path, file_open_time, file_size, duration
            FROM events
            WHERE event_id = ?1
            ",
            event_id
        )
        .fetch_optional(&self.pool)
        .await?;

        let event = match event_row {
            Some(event_row) => {
                let file_open_time = DateTime::<Utc>::from_utc(event_row.file_open_time, Utc);
                Some(RecorderEvent {
                    event_id: event_row.event_id,
                    event_type: event_row.event_type,
                    event_data: RecorderEventData {
                        room_id: event_row.room_id as u64,
                        name: event_row.name,
                        title: event_row.title,
                        relative_path: event_row.relative_path,
                        file_open_time: file_open_time.to_rfc3339(),
                        file_size: event_row.file_size as u64,
                        duration: event_row.duration as f64
                    }
                })},
            None => None,
        };

        Ok(event)
    }
}

// Table `uploads`
impl BiliupDao {
    pub async fn add_upload(&self, event: &RecorderEvent) -> Result<u64> {
        let mut conn = self.pool.acquire().await?;

        let now = chrono::Utc::now();
        let id = sqlx::query!(
            "
            INSERT INTO uploads (event_id, created_at)
            VALUES (?1, ?2)
            ",
            event.event_id, now
        )
        .execute(&mut conn)
        .await?
        .last_insert_rowid();

        Ok(id as u64)
    }

    pub async fn finish_upload(&self, event_id: &str, aid: u64, title: &str) -> Result<()> {
        let mut conn = self.pool.acquire().await?;

        let aid = aid as i64;
        let now = chrono::Utc::now();
        sqlx::query!(
            "
            UPDATE uploads
            SET uploaded = 1, finished_at = ?1, avid = ?2, archive = ?3
            WHERE event_id = ?4
            ",
            now, aid, title, event_id
        )
        .execute(&mut conn)
        .await?;

        Ok(())
    }

    pub async fn find_existing_upload(&self, data: &RecorderEventData) -> Result<Option<u64>> {
        let room_id = data.room_id as i64;

        struct _Upload { avid: Option<i64> }
        let aid = match sqlx::query_as!(
            _Upload,
            "
            SELECT avid
            FROM uploads
            JOIN events ON events.event_id = uploads.event_id
            WHERE room_id = ?1 AND file_open_time = ?2 AND uploaded = 1 AND avid IS NOT NULL
            ",
            room_id, data.file_open_time
        )
        .fetch_optional(&self.pool)
        .await? {
            Some(upload) => upload.avid.unwrap(),
            None => return Ok(None)
        };
        
        Ok(Some(aid as u64))
    }

    pub(crate) async fn get_unfinished_uploads(&self) -> Result<Vec<UploadState>> {
        let uploads = sqlx::query_as!(
            UploadState,
            "
            SELECT uploads.event_id, uploads.created_at, events.relative_path, events.file_size
            FROM uploads
            JOIN events ON events.event_id = uploads.event_id
            WHERE uploaded = 0
            "
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(uploads)
    }

    pub(crate) async fn get_finished_uploads(&self) -> Result<Vec<UploadHistory>> {
        let uploads = sqlx::query_as!(
            UploadHistory,
            "
            SELECT uploads.finished_at, events.relative_path, events.file_size, uploads.avid
            FROM uploads
            JOIN events ON events.event_id = uploads.event_id
            WHERE uploaded = 1 AND uploads.created_at >= DATE('now', '-1 day')
            "
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(uploads)
    }
}
