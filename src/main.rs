use std::{
    collections::HashMap,
    convert::Infallible,
    // env,
};

use hyper::{
    server::conn::AddrStream,
    service::{make_service_fn, service_fn},
    Server
};

use tokio::sync::Mutex;

mod server;
mod session;
mod constants;

#[tokio::main]
async fn main() -> Result<(), hyper::Error> {
    // DO NOT move this is where app states are supposed to be declared
    // let state = server::PeerMap::new(Mutex::new(HashMap::new()));
    let rooms = server::RoomMap::new(Mutex::new(HashMap::new()));

    let addr = "127.0.0.1:8080".to_string().parse().unwrap();
    // let addr = "0.0.0.0:443".to_string().parse().unwrap();

    let make_svc = make_service_fn(move |conn: &AddrStream| {
        let remote_addr = conn.remote_addr();
        // let state = state.clone();
        let rooms = rooms.clone();
        let service = service_fn(move |req| server::handle_request(/* state.clone(), */ req, remote_addr, rooms.clone()));
        async { Ok::<_, Infallible>(service) }
    });

    let server = Server::bind(&addr).serve(make_svc);

    server.await?;

    Ok::<_, hyper::Error>(())
}