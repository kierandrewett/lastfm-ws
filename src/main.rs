mod bus;
mod lastfm;
mod model;
mod poller;

use std::sync::Arc;

use actix::*;
use actix_web::{App, Error, HttpRequest, HttpResponse, HttpServer, get, web};
use actix_web_actors::ws;

use bus::NowPlayingBus;
use lastfm::LastFmClient;
use poller::run_poller;
use tokio::sync::RwLock;

use crate::model::{NowPlaying, PlaybackState};

#[derive(Clone)]
struct AppState {
    bus: NowPlayingBus,
    pub now_playing: Arc<RwLock<Option<NowPlaying>>>,
    pub last_playing: Arc<RwLock<Option<NowPlaying>>>,
    pub track_started_at: Arc<RwLock<Option<std::time::Instant>>>,
    pub last_position_ms: Arc<RwLock<u64>>,
    pub playback_state: Arc<RwLock<PlaybackState>>,
}

struct WsConn {
    rx: tokio::sync::broadcast::Receiver<model::NowPlaying>,
    state: Arc<AppState>,
}

#[derive(Message)]
#[rtype(result = "()")]
struct SendText(String);

impl Actor for WsConn {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        // send cached instantly
        {
            let addr = ctx.address();
            let state = self.state.clone();

            actix::spawn(async move {
                if state.playback_state.read().await.clone() != PlaybackState::Playing {
                    return;
                }
                
                state.now_playing.read().await.clone().map(|np| {
                    let msg = serde_json::to_string(&np).unwrap();
                    addr.do_send(SendText(msg));
                });
            });
        }

        // live updates
        {
            let addr = ctx.address();
            let mut rx = self.rx.resubscribe();

            actix::spawn(async move {
                let mut last_sent: Option<String> = None;

                loop {
                    match rx.recv().await {
                        Ok(np) => {
                            let new_json = serde_json::to_string(&np).unwrap();

                            if last_sent.as_ref() != Some(&new_json) {
                                last_sent = Some(new_json.clone());
                                addr.do_send(SendText(new_json));
                            }
                        }
                        Err(_) => break,
                    }
                }
            });
        }

        {
            let addr = ctx.address();
            let state = self.state.clone();

            actix::spawn(async move {
                loop {
                    let playback_state = *state.playback_state.read().await;
                    let playing_state = state.now_playing.read().await.clone();
                    let prev_playing_state = state.last_playing.read().await.clone();
                    let started_at = state.track_started_at.read().await.clone();
                    let frozen_pos = *state.last_position_ms.read().await;

                    let position_ms =
                        if let Some(start) = started_at {
                            if matches!(playback_state, PlaybackState::Playing) {
                                start.elapsed().as_millis() as u64
                            } else {
                                frozen_pos
                            }
                        } else {
                            frozen_pos
                        };

                    let duration_ms = playing_state
                        .clone()
                        .or_else(|| prev_playing_state)
                        .as_ref()
                        .and_then(|np| {
                            np.track_info
                                .as_ref()
                                .and_then(|t| t.duration.parse::<u64>().ok())
                        })
                        .unwrap_or(0);

                    let msg = serde_json::json!({
                        "playing": matches!(playback_state, PlaybackState::Playing),
                        "position_ms": position_ms.min(duration_ms),
                        "duration_ms": duration_ms.max(0),
                    })
                    .to_string();

                    addr.do_send(SendText(msg));

                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            });
        }
    }
}

impl Handler<SendText> for WsConn {
    type Result = ();

    fn handle(&mut self, msg: SendText, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsConn {
    fn handle(&mut self, _msg: Result<ws::Message, ws::ProtocolError>, _ctx: &mut Self::Context) {}
}

#[get("/")]
async fn ws_route(
    req: HttpRequest,
    stream: web::Payload,
    data: web::Data<AppState>,
) -> Result<HttpResponse, Error> {
    let rx = data.bus.subscribe();

    ws::start(
        WsConn {
            rx,
            state: data.get_ref().clone().into(),
        },
        &req,
        stream,
    )
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt::init();

    let api_key = std::env::var("LASTFM_API_KEY").expect("LASTFM_API_KEY not set");
    let user = std::env::var("LASTFM_USER").expect("LASTFM_USER not set");

    let bus = NowPlayingBus::new();

    let state = AppState {
        bus,
        now_playing: Arc::new(RwLock::new(None)),
        last_playing: Arc::new(RwLock::new(None)),
        track_started_at: Arc::new(RwLock::new(None)),
        last_position_ms: Arc::new(RwLock::new(0)),
        playback_state: Arc::new(RwLock::new(PlaybackState::Stopped)),
    };
    tokio::spawn(run_poller(LastFmClient::new(api_key, user), state.clone()));

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .service(ws_route)
    })
    .bind(("0.0.0.0", 8321))?
    .run()
    .await
}
