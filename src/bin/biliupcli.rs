use biliupmgr::recorder::{RecorderEvent, RecorderEventData};
use byteorder::{BigEndian, ByteOrder};
use clap::Parser;
use regex::Regex;
use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::PathBuf;
use uuid::Uuid;

trait UploadFileParser {
    fn from_file<P>(path: P) -> Option<Self>
    where
        P: AsRef<std::path::Path>,
        Self: Sized;
}

impl UploadFileParser for RecorderEvent {
    fn from_file<P>(path: P) -> Option<Self>
    where
        P: AsRef<std::path::Path>,
    {
        let path = path.as_ref();
        let re = Regex::new(r"录制-(\d+)-(\d{4})(\d{2})(\d{2})-(\d{2})(\d{2})(\d{2})-(\d{3})-(.+)")
            .ok()?;

        let (room_id, file_open_time, title) = path.file_stem().and_then(|stem| {
            let stem_str = stem.to_string_lossy();
            let caps = re.captures(&stem_str)?;

            Some((
                caps.get(1)?.as_str().parse::<u64>().ok()?,
                format!(
                    "{}-{}-{}T{}:{}:{}.{}+08:00",
                    caps.get(2)?.as_str(),
                    caps.get(3)?.as_str(),
                    caps.get(4)?.as_str(),
                    caps.get(5)?.as_str(),
                    caps.get(6)?.as_str(),
                    caps.get(7)?.as_str(),
                    caps.get(8)?.as_str()
                ),
                caps.get(9)?.as_str().to_string(),
            ))
        })?;

        let name = get_streamer_name(room_id);
        let relative_path = PathBuf::from(format!("{}-{}", room_id, name))
            .join(path.file_name()?)
            .to_string_lossy()
            .to_string();

        let file = File::open(path).ok()?;
        let file_size = file.metadata().ok()?.len();
        let duration = get_duration(&file)?;

        Some(RecorderEvent {
            event_id: Uuid::new_v4().to_string(),
            event_type: "FileClosed".to_string(),
            event_data: RecorderEventData {
                room_id,
                name,
                title,
                relative_path,
                file_open_time,
                file_size,
                duration,
            },
        })
    }
}

fn get_duration(file: &File) -> Option<f64> {
    let mut reader = BufReader::new(file);

    let metadata_buf = {
        let metadata_size = {
            let mut header_buf = vec![0u8; 24];
            reader.read_exact(&mut header_buf).ok()?;
            BigEndian::read_u24(&header_buf[14..17])
        };

        let mut metadata_buf = vec![0u8; metadata_size as usize];
        reader.read_exact(&mut metadata_buf).ok()?;

        metadata_buf
    };

    let needle = b"duration\x00";

    let duration = match metadata_buf
        .windows(needle.len())
        .position(|window| window == needle)
    {
        Some(pos) => {
            let duration_pos = pos + needle.len();
            BigEndian::read_f64(&metadata_buf[duration_pos..duration_pos + 8])
        }
        None => return None,
    };

    Some(duration)
}

#[derive(Parser, Debug)]
#[clap(version)]
struct Args {
    video_file: PathBuf,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let args = Args::parse();

    let event = RecorderEvent::from_file(&args.video_file).ok_or(io::Error::new(
        io::ErrorKind::InvalidData,
        "Cannot parse video file",
    ))?;

    let client = reqwest::Client::new();

    let request = client.post("http://127.0.0.1:23380/recorder");
    match request.json(&event).send().await.ok() {
        Some(response) => {
            println!("{:?}", response.text().await.ok());
        }
        None => {
            println!("Failed to send request");
        }
    }

    Ok(())
}

// FIXME: complete this
fn get_streamer_name(room_id: u64) -> String {
    assert!(false);
    match room_id {
        3 => "3号直播间",
        _ => "",
    }
    .to_string()
}
