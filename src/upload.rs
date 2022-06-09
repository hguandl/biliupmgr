use std::path::Path;

use actix_web::web;
use anyhow::{anyhow, bail, Result};
use biliup::{
    client,
    video::{BiliBili, Studio, Subtitle, Vid},
    VideoFile,
};
use futures::StreamExt;
use log::info;

use crate::{
    config::{ManagerConfig, RoomConfig},
    recorder::{RecorderEvent, RecorderEventData},
    webhook::AppState,
};

use crate::db::BiliupDao;

fn make_studio(data: &RecorderEventData, config: &RoomConfig) -> Studio {
    Studio {
        copyright: 2,
        source: format!("https://live.bilibili.com/{}", data.room_id),
        tid: config.tid,
        cover: config.cover.clone(),
        title: data.format(&config.studio_title),
        desc_format_id: 0,
        desc: config.description.clone(),
        dynamic: "".to_string(),
        subtitle: Subtitle::default(),
        tag: config.tags.clone(),
        videos: Vec::new(),
        dtime: None,
        open_subtitle: true,
        interactive: 0,
        mission_id: None,
        dolby: 0,
        no_reprint: Some(0),
        aid: None,
        up_selection_reply: false,
        up_close_reply: false,
        up_close_danmu: false,
        open_elec: Some(0),
    }
}

pub async fn upload(
    config: &ManagerConfig,
    dao: &BiliupDao,
    event: &RecorderEvent,
    state: &web::Data<AppState>,
) -> Result<u64> {
    let data = &event.event_data;
    info!("New event <{}>: {}", data.room_id, event.event_id);

    info!("Pre: Updating current state");
    {
        let mut current = state.current.write().unwrap();
        *current = Some(event.event_id.clone());
    }

    let room_config = config
        .rooms
        .get(&event.event_data.room_id)
        .ok_or(anyhow!("Cannot find room <{}>", event.event_data.room_id))?;

    info!("Create client and login");
    let client = client::Client::default();
    let login_info = {
        let cookies_file = std::fs::File::options()
            .read(true)
            .write(true)
            .open(&room_config.user_cookie)?;
        client.login_by_cookies(cookies_file).await?
    };

    info!("Upload video file");
    let mut video = {
        info!("File information");
        let video_file = {
            let video_path = Path::new(config.rec_dir.as_str()).join(&data.relative_path);
            VideoFile::new(&video_path)?
        };

        info!("Create uploader");
        let line = match config.line.as_str() {
            "bda2" => biliup::line::bda2(),
            "kodo" => biliup::line::kodo(),
            "ws" => biliup::line::ws(),
            "qn" => biliup::line::qn(),
            "cos" => biliup::line::cos(),
            "cos-internal" => biliup::line::cos_internal(),
            "AUTO" => biliup::line::Probe::probe().await?,
            _ => bail!("Unknown line: {}", config.line),
        };
        let uploader = line.to_uploader(video_file);

        info!("Uploading {}", data.relative_path);
        uploader
            .upload(&client, config.limit, |vs| {
                vs.map(|chunk| {
                    let (chunk, len) = chunk?;
                    let mut uploaded = state.uploaded.write().unwrap();
                    *uploaded += len;
                    Ok((chunk, len))
                })
            })
            .await?
    };
    video.title = Some(data.format(&room_config.part_title));
    let mut uploaded_videos = vec![video];

    info!("Submit video");
    let (studio_title, ret) = match dao.find_existing_upload(data).await? {
        Some(aid) => {
            info!("Appending to av{}", aid);
            let mut studio = BiliBili::new(&login_info, &client)
                .studio_data(Vid::Aid(aid))
                .await?;
            studio.videos.append(&mut uploaded_videos);
            (studio.title.clone(), studio.edit(&login_info).await?)
        }
        None => {
            let mut studio = make_studio(data, room_config);

            if !studio.cover.starts_with("http") {
                let cover_url = BiliBili::new(&login_info, &client)
                    .cover_up(&std::fs::read(Path::new(&studio.cover))?)
                    .await?;
                studio.cover = cover_url;
            }

            info!("Submitting a new archive: {}", studio.title);
            studio.videos = uploaded_videos;
            (studio.title.clone(), studio.submit(&login_info).await?)
        }
    };
    let aid = ret["data"]["aid"].as_u64().unwrap();

    info!("Uploading finished: av{}", aid);
    dao.finish_upload(&event.event_id, aid, &studio_title)
        .await?;

    {
        let mut uploaded = state.uploaded.write().unwrap();
        let mut current = state.current.write().unwrap();

        *uploaded = 0;
        *current = None;
    }
    Ok(aid)
}
