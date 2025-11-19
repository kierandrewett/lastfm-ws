use crate::model::NowPlaying;
use anyhow::Result;
use reqwest::Client;
use serde_json::Value;

pub struct LastFmClient {
    http: Client,
    api_key: String,
    user: String,
}

impl LastFmClient {
    pub fn new(api_key: String, user: String) -> Self {
        Self {
            http: Client::new(),
            api_key,
            user,
        }
    }

    pub async fn get_now_playing(&self) -> Result<Option<NowPlaying>> {
        let url = "https://ws.audioscrobbler.com/2.0/";
        let res = self.http
            .get(url)
            .query(&[
                ("method", "user.getrecenttracks"),
                ("user", self.user.as_str()),
                ("api_key", self.api_key.as_str()),
                ("format", "json"),
                ("limit", "1")
            ])
            .send()
            .await?
            .error_for_status()?
            .json::<Value>()
            .await?;

        let track = res["recenttracks"]["track"]
            .get(0)
            .cloned()
            .unwrap_or(Value::Null);

        if track.is_null() {
            return Ok(None);
        }

        let artist = track["artist"]["#text"].as_str().unwrap_or("").to_string();
        let name = track["name"].as_str().unwrap_or("").to_string();
        let album = track["album"]["#text"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        let now_playing = track["@attr"]["nowplaying"]
            .as_str()
            .map(|v| v == "true")
            .unwrap_or(false);

        let mut album_art = track["image"]
            .as_array()
            .and_then(|imgs| imgs.last())
            .and_then(|img| img["#text"].as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        if album_art.as_ref().map(|s| s.contains("2a96cbd8b46e442fc41c2b86b821562f")).unwrap_or(false) {
            album_art = None;
        }

        Ok(Some(NowPlaying {
            artist,
            track: name,
            album,
            album_art,
            now_playing,
        }))
    }
}
