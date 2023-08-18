use std::{
    collections::HashMap,
    convert::Infallible,
    // env,
    net::SocketAddr,
    sync::Arc
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
        protocol::{Message, Role},
    },
    WebSocketStream,
};

use tokio::sync::Mutex;
use url::Url;
use crate::session;

// SERVERSIDE cake validation
// the room HOST will spawn the cakes and send the positions over.
// the room 


pub struct Player {
    pub sender: Tx,
    pub username: String,
}

pub type Tx = UnboundedSender<Message>;
pub type PeerMap = Arc<Mutex<HashMap<SocketAddr, Player>>>;


pub async fn handle_request(
    peer_map: PeerMap,
    mut req: Request<Body>,
    addr: SocketAddr,
) -> Result<Response<Body>, Infallible> {
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
        || !headers.get(SEC_WEBSOCKET_VERSION).map(|h| h == "13").unwrap_or(false)
        || key.is_none()
        // || req.uri() != "/socket"
    /* \end{condition} */
    {
        return Ok(Response::new(Body::from("Hello World!")));
    }
    // println!("{:?}", req.uri());
    // println!("{:?}", req.uri().query());
    let queries = Url::parse(&("https://example.org".to_string() + &req.uri().to_string()))
        .expect("Failed to parse URI")
        .query_pairs()
        .filter_map(|(k, v)| {
            let name = k.into_owned();
            if name == "username" {
                Some((name, v.into_owned()))
            } else {
                None
            }
        })
        .collect::<HashMap<String, String>>();
    // let username =  match req.uri().query() {
    //     Some(query) if query.starts_with("username") => {
    //         query[9..].to_string()
    //     },
    //     _ => "balls_eater".to_string(),
    // };
    println!("{:?}", queries["username"]);
    let ver = req.version();
    tokio::task::spawn(async move {
        match hyper::upgrade::on(&mut req).await {
            Ok(upgraded) => {
                // session::handle_connection(
                //     peer_map,
                //     WebSocketStream::from_raw_socket(upgraded, Role::Server, None).await,
                //     addr,
                //     queries.get("username").unwrap_or(&"balls_eater".to_string()).to_string()
                // )
                // .await;
                let game_session = session::GameSession {
                    peer_map,
                    addr,
                    username: queries.get("username").unwrap_or(&"balls_eater".to_string()).to_string()
                };
                game_session.start(WebSocketStream::from_raw_socket(upgraded, Role::Server, None).await).await;
            }
            Err(e) => println!("upgrade error: {}", e),
        }
    });
    let mut res = Response::new(Body::empty());
    *res.status_mut() = StatusCode::SWITCHING_PROTOCOLS;
    *res.version_mut() = ver;
    res.headers_mut().append(CONNECTION, upgrade);
    res.headers_mut().append(UPGRADE, websocket);
    res.headers_mut().append(SEC_WEBSOCKET_ACCEPT, derived.unwrap().parse().unwrap());
    // Let's add an additional header to our response to the client.
    // res.headers_mut().append("MyCustomHeader", ":)".parse().unwrap());
    // res.headers_mut().append("SOME_TUNGSTENITE_HEADER", "header_value".parse().unwrap());
    Ok(res)
}