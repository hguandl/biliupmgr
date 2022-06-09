use chrono::{DateTime, FixedOffset, Timelike};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecorderEvent {
    #[serde(rename = "EventId")]
    pub event_id: String,
    #[serde(rename = "EventType")]
    pub event_type: String,
    #[serde(rename = "EventData")]
    pub event_data: RecorderEventData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecorderEventData {
    #[serde(rename = "RoomId")]
    pub room_id: u64,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Title")]
    pub title: String,
    #[serde(rename = "RelativePath")]
    pub relative_path: String,
    #[serde(rename = "FileOpenTime")]
    pub file_open_time: String,
    #[serde(rename = "FileSize")]
    pub file_size: u64,
    #[serde(rename = "Duration")]
    pub duration: f64,
}

impl RecorderEventData {
    pub fn format(&self, format: &str) -> String {
        Self::format_parse(format)
            .iter()
            .fold(String::new(), |mut acc, s| {
                match s.as_str() {
                    "%T" => acc.push_str(&self.title),
                    "%N" => acc.push_str(&self.name),
                    "%d" => acc.push_str(&self.date_string()),
                    "%t" => acc.push_str(&self.time_string()),
                    _ => acc.push_str(s),
                }
                acc
            })
    }

    fn date_string(&self) -> String {
        let mut date = DateTime::parse_from_rfc3339(&self.file_open_time).unwrap();
        if date.hour() < 4 {
            date = date - chrono::Duration::hours(4);
        }
        let formatted_date = date.format("%Y.%m.%d");
        formatted_date.to_string()
    }

    fn time_string(&self) -> String {
        let date = {
            let date = DateTime::parse_from_rfc3339(&self.file_open_time).unwrap();
            let timezone = FixedOffset::east(8 * 3600);

            date.with_timezone(&timezone)
        };

        let formatted_time = date.format("%Y%m%d-%H%M%S");
        formatted_time.to_string()
    }

    fn format_parse(format: &str) -> Vec<String> {
        let mut result: Vec<String> = Vec::new();
        let mut last = 0;
        for (index, _) in format.match_indices('%') {
            if last != index {
                result.push(format[last..index].to_string());
            };
            if index == format.len() - 1 {
                result.push("%".to_string());
                break;
            }
            result.push(format[index..index + 2].to_string());
            last = index + 2;
        }
        result
    }
}
