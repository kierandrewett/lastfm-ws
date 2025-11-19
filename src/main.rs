mod bus;
mod lastfm;
mod model;
mod poller;
mod redis;

use actix::*;
use actix_web::{App, Error, HttpRequest, HttpResponse, HttpServer, get, web};
use actix_web_actors::ws;

use bus::NowPlayingBus;
use lastfm::LastFmClient;
use poller::run_poller;
use redis::RedisStore;

#[derive(Clone)]
struct AppState {
    bus: NowPlayingBus,
    redis: RedisStore,
}

struct WsConn {
    rx: tokio::sync::broadcast::Receiver<model::NowPlaying>,
    redis: RedisStore,
}

#[derive(Message)]
#[rtype(result = "()")]
struct SendText(String);

impl Actor for WsConn {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        // send cached instantly
        {
            let redis = self.redis.clone();
            let addr = ctx.address();

            actix::spawn(async move {
                if let Ok(Some(cached)) = redis.get_now_playing().await {
                    if cached.now_playing {
                        let msg = serde_json::to_string(&cached).unwrap();
                        addr.do_send(SendText(msg));
                    }
                }
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
            let redis = self.redis.clone();
            let addr = ctx.address();

            actix::spawn(async move {
                loop {
                    let is_playing = redis.get_now_playing().await
                        .map(|opt| opt.is_some())
                        .unwrap_or(false);

                    let msg = serde_json::json!({
                        "playing": is_playing
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
            redis: data.redis.clone(),
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

    let redis_url = std::env::var("REDIS_URL").unwrap_or("redis://127.0.0.1/".into());
    let redis = RedisStore::new(&redis_url);

    redis.clear_now_playing().await.unwrap();

    let bus = NowPlayingBus::new();

    tokio::spawn(run_poller(
        LastFmClient::new(api_key, user),
        bus.clone(),
        redis.clone(),
    ));

    let state = AppState { bus, redis };

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .service(ws_route)
    })
    .bind(("0.0.0.0", 8321))?
    .run()
    .await
}
