use std::{
    borrow::Cow,
    collections::HashMap,
    convert::Infallible,
    // env,
    net::SocketAddr,
    sync::Arc,
};

use hyper::{
    header::{
        HeaderValue, CONNECTION, SEC_WEBSOCKET_ACCEPT, SEC_WEBSOCKET_KEY, SEC_WEBSOCKET_VERSION,
        UPGRADE,
    },
    Body, Method, Request, Response, StatusCode, Version,
};

use futures_channel::mpsc::UnboundedSender;

use tokio_tungstenite::{
    tungstenite::{
        handshake::derive_accept_key,
        protocol::{frame::coding::CloseCode, CloseFrame, Message, Role},
    },
    WebSocketStream,
};

use sysinfo::{ProcessExt, System, SystemExt};

use crate::session;
use tokio::sync::Mutex;
use url::Url;

// SERVERSIDE cake validation
// the room HOST will spawn the cakes and send the positions over.
// the room

#[derive(Clone, Debug)]
pub struct Player {
    pub addr: SocketAddr,
    pub sender: Tx,
    pub username: String,
    // 0 is male, 1 is female, 2 is Jamal
    pub skin: u8,
    // hard limit max room size 255, fps will probably crash before this is hit
    pub id: u8,
}

pub type Tx = UnboundedSender<Message>;
// although SocketAddr is unique (even if localhost)
// peer map is useless
// pub type PeerMap = Arc<Mutex<HashMap<SocketAddr, Player>>>;
// Vec<Player> should be the best since it contains three times information... also don't think we should use references
// Cannot use HashSet... although I don't think the same player can join twice

// host is ghost atm
#[derive(Debug)]
pub struct Room {
    pub seed: usize,
    pub started: bool,
    pub nextid: u8,
    // more game settings
    pub players: Vec<Player>,
    pub barricade_counter: f32,
}
pub type RoomMap = Arc<Mutex<HashMap<String, Room>>>;

pub async fn handle_request(
    // peer_map: PeerMap,
    mut req: Request<Body>,
    addr: SocketAddr,
    rooms: RoomMap,
) -> Result<Response<Body>, Infallible> {
    // health check stuff
    if req.uri() == "/healthz" {
        let mut res = Response::new(Body::empty());
        *res.status_mut() = StatusCode::OK;
        *res.version_mut() = req.version();
        let mut system = System::new_all();
        system.refresh_all();

        if let Some(process) = system.process(sysinfo::get_current_pid().unwrap()) {
            let memory = process.memory() / 1024; // Memory usage in KB
            let cpu_usage = process.cpu_usage();
            let disk_usage = process.disk_usage();

            println!(
                "Memory usage: {} KB | CPU usage: {}% | DISK usage: {}/{}",
                memory, cpu_usage, disk_usage.written_bytes, disk_usage.total_written_bytes
            );
        }

        // println!("=> disks:");
        // for disk in system.disks() {
        //     println!("{:?}", disk);
        // }

        // println!("total memory: {} bytes", system.total_memory());
        // println!("used memory : {} bytes", system.used_memory());
        // println!("total swap  : {} bytes", system.total_swap());
        // println!("used swap   : {} bytes", system.used_swap());

        // for cpu in system.cpus() {
        //     print!("CPU USAGE: {}% ", cpu.cpu_usage());
        // }

        return Ok(res);
    }
    println!("Received a new, potentially ws handshake");
    println!("The request's path is: {}", req.uri().path());
    println!("The request's headers are:");
    for (ref header, _value) in req.headers() {
        println!("* {}", header);
    }
    let upgrade = HeaderValue::from_static("Upgrade");
    let websocket = HeaderValue::from_static("websocket");
    let headers = req.headers();
    let key = headers.get(SEC_WEBSOCKET_KEY);
    let derived = key.map(|k| derive_accept_key(k.as_bytes()));
    /* \begin{condition} */
    if req.method() != Method::GET
        || req.version() < Version::HTTP_11
        || !headers
            .get(CONNECTION)
            .and_then(|h| h.to_str().ok())
            .map(|h| {
                h.split(|c| c == ' ' || c == ',')
                    .any(|p| p.eq_ignore_ascii_case(upgrade.to_str().unwrap()))
            })
            .unwrap_or(false)
        || !headers
            .get(UPGRADE)
            .and_then(|h| h.to_str().ok())
            .map(|h| h.eq_ignore_ascii_case("websocket"))
            .unwrap_or(false)
        || !headers
            .get(SEC_WEBSOCKET_VERSION)
            .map(|h| h == "13")
            .unwrap_or(false)
        || key.is_none()
    // || req.uri() != "/socket"
    /* \end{condition} */
    {
        return Ok(Response::new(Body::from(
            "Yur version or browser is outdated innit bruv (If you are pinging this website, it means the multiplayer engine is running!",
        )));
    }
    // println!("{:?}", req.uri());
    // println!("{:?}", req.uri().query());
    // Dumbass lib, have to concat smh. I cannot get full url otherwise
    println!(
        "{:?}",
        ("https://example.org".to_string() + &req.uri().to_string())
    );
    let queries = Url::parse(&("https://example.org".to_string() + &req.uri().to_string()))
        .expect("Failed to parse URI")
        .query_pairs()
        .map(|(k, v)| (k.into(), v.into_owned()))
        // .filter_map(|(k, v)| {
        //     let name = k.into_owned();
        //     if name == "username" {
        //         Some((name, v.into_owned()))
        //     } else {
        //         None
        //     }
        // })
        .collect::<HashMap<String, String>>();
    // let username =  match req.uri().query() {
    //     Some(query) if query.starts_with("username") => {
    //         query[9..].to_string()
    //     },
    //     _ => "balls_eater".to_string(),
    // };
    // println!("{:?}", queries.get("username"));
    println!("{:?}", queries);
    // println!("{:?}", queries.get("room"));
    let ver = req.version();
    if let Some(room) = queries.get("room") {
        // kinda dumb that I have to do this
        let room = room.to_string();
        let rooms_lock = rooms.lock().await;
        if let Some(curr) = rooms_lock.get(&room) {
            if curr.started {
                tokio::task::spawn(async move {
                    match hyper::upgrade::on(&mut req).await {
                        Ok(upgraded) => {
                            let mut ws =
                                WebSocketStream::from_raw_socket(upgraded, Role::Server, None)
                                    .await;
                            ws.close(Some(CloseFrame {
                                code: CloseCode::from(4001),
                                reason: Cow::Owned("Room already started innit bruv".to_string()),
                            }))
                            .await
                            .unwrap();
                            // when the function finishes running, the game_session object
                            // should automatically get dropped once out of scope
                        }
                        Err(e) => println!("upgrade error: {}", e),
                    }
                });
                let mut res = Response::new(Body::empty());
                *res.status_mut() = StatusCode::SWITCHING_PROTOCOLS;
                *res.version_mut() = ver;
                res.headers_mut().append(CONNECTION, upgrade);
                res.headers_mut().append(UPGRADE, websocket);
                res.headers_mut()
                    .append(SEC_WEBSOCKET_ACCEPT, derived.unwrap().parse().unwrap());
                return Ok(res);
            }
        }
        drop(rooms_lock);
        tokio::task::spawn(async move {
            match hyper::upgrade::on(&mut req).await {
                Ok(upgraded) => {
                    let mut game_session = session::GameSession {
                        rooms,
                        addr,
                        username: queries
                            .get("username")
                            .unwrap_or(&"balls_eater".to_string())
                            .to_string(),
                        skin: queries
                            .get("skin")
                            .unwrap_or(&"0".to_string())
                            .parse::<u8>()
                            .unwrap(),
                        seed: (*queries.get("seed").unwrap_or(&"0".to_string()))
                            .parse()
                            .unwrap_or(0),
                        room: room.clone(),
                        // id will be calculated
                        id: None,
                    };
                    game_session
                        .start(WebSocketStream::from_raw_socket(upgraded, Role::Server, None).await)
                        .await;
                    // when the function finishes running, the game_session object
                    // should automatically get dropped once out of scope
                }
                Err(e) => println!("upgrade error: {}", e),
            }
        });
        let mut res = Response::new(Body::empty());
        *res.status_mut() = StatusCode::SWITCHING_PROTOCOLS;
        *res.version_mut() = ver;
        res.headers_mut().append(CONNECTION, upgrade);
        res.headers_mut().append(UPGRADE, websocket);
        res.headers_mut()
            .append(SEC_WEBSOCKET_ACCEPT, derived.unwrap().parse().unwrap());
        // Let's add an additional header to our response to the client.
        // res.headers_mut().append("MyCustomHeader", ":)".parse().unwrap());
        // res.headers_mut().append("SOME_TUNGSTENITE_HEADER", "header_value".parse().unwrap());
        Ok(res)
    } else {
        // let mut res = Response::new(Body::empty());
        // *res.status_mut() = StatusCode::BAD_REQUEST;
        // *res.version_mut() = ver;
        // // how do I specify close code??
        // Ok(res)
        tokio::task::spawn(async move {
            match hyper::upgrade::on(&mut req).await {
                Ok(upgraded) => {
                    let mut ws =
                        WebSocketStream::from_raw_socket(upgraded, Role::Server, None).await;
                    ws.close(Some(CloseFrame {
                        code: CloseCode::Invalid,
                        reason: Cow::Owned("You're gay".to_string()),
                    }))
                    .await
                    .unwrap();
                    // when the function finishes running, the game_session object
                    // should automatically get dropped once out of scope
                }
                Err(e) => println!("upgrade error: {}", e),
            }
        });
        let mut res = Response::new(Body::empty());
        *res.status_mut() = StatusCode::SWITCHING_PROTOCOLS;
        *res.version_mut() = ver;
        res.headers_mut().append(CONNECTION, upgrade);
        res.headers_mut().append(UPGRADE, websocket);
        res.headers_mut()
            .append(SEC_WEBSOCKET_ACCEPT, derived.unwrap().parse().unwrap());
        Ok(res)
    }
}
