use std::{
    // env,
    net::SocketAddr
};

use hyper::upgrade::Upgraded;

use futures_channel::mpsc::unbounded;
use futures_util::{future, pin_mut, TryStreamExt, StreamExt};

use tokio_tungstenite::{
    tungstenite::{
        Error,
        protocol::Message
    },
    WebSocketStream,
};

use crate::server;

// serversent events
const PLAYER_JOIN: u8 = 0;
// const TEST: f32 = 0.032400325;
// these won't be used since this is basically a peer server
// but here are the specifications:
// const VELOCITY_UPDATE: u8 = 1;
// const POSITION_UPDATE: u8 = 2;
// const ACTION_UPDATE: u8 = 3;
/* Rotation can be calculated using velocity */

pub struct GameSession {
    pub peer_map: server::PeerMap,
    pub rooms: server::RoomMap,
    pub addr: SocketAddr,
    pub username: String,
    pub sex: bool,
    pub seed: usize,
    pub room: String
}

// modern problems require modern solutions
unsafe impl std::marker::Send for GameSession {}
unsafe impl std::marker::Sync for GameSession {}

impl GameSession {
    // pub async fn other(&self) {

    // }

    async fn recv(&self, msg: Message) -> Result<(), Error> {
        // let peers = self.peer_map.lock().await;
        let mut rooms = self.rooms.lock().await;

        let broadcast_recipients = rooms.entry(self.room.clone()).or_insert(server::Room {
            seed: self.seed,
            players: Vec::new()
        });
            // peers.iter().filter(|(peer_addr, _)| peer_addr != &&self.addr).map(|(_, player)| player);

        for recp in broadcast_recipients.players.iter() {
            // println!("{}", recp.username);
            // do not send to self
            if recp.addr != self.addr {
                // this is a peer functionality...
                recp.sender.unbounded_send(msg.clone()).unwrap();
            }
        }
        Ok(())
    }

    pub async fn start(&self, ws_stream: WebSocketStream<Upgraded>) {
        println!("WebSocket connection established: {}", self.addr);

        // Insert the write part of this peer to the peer map.
        // tx = send, rx = receive
        let (tx, rx) = unbounded();
        
        let mut plock = self.peer_map.lock().await;
        let player = server::Player {
            sender: tx.clone(),
            username: self.username.clone(),
            sex: self.sex,
            addr: self.addr
        };
        plock.insert(self.addr, player.clone());

        // drop(plock);

        let mut rooms = self.rooms.lock().await;
        // keep in mind that the first player is the host.
        let room_name = self.room.clone();
        let entry = if let Some(room) = rooms.get_mut(&room_name) {
            // if room already exists then we completely ignore the seed value
            println!("Just adding");
            room.players.push(player.clone());
            room
        } else {
            // 69 means the client is the host and he is supposed to send in the cake coords
            println!("New");
            tx.unbounded_send(Message::Binary(vec![69])).unwrap();
            rooms.entry(room_name).or_insert(server::Room {
                seed: self.seed,
                players: vec![player.clone()]
            })
        };

        // drop(rooms);
        // let entry = rooms
        //     .entry(self.room.clone())
        //     .or_insert_with(Vec::new);

        // entry.push(player);
    
        let mut v = vec![PLAYER_JOIN];

        v.extend_from_slice(self.username.as_bytes());

        // send everyone in the room our current information
        for recp in entry.players.iter() {
            if recp.addr == self.addr {
                continue;
            }
            let mut v2 = vec![PLAYER_JOIN];
            v2.extend_from_slice(recp.username.as_bytes());
            // send each user OUR username
            recp.sender.unbounded_send(Message::Binary(v.clone())).unwrap();
            // send client each user's username
            tx.unbounded_send(Message::Binary(v2)).unwrap();
        }

        // for recp in plock.iter().filter(|(peer_addr, _)| peer_addr != &&self.addr).map(|(_, player)| player) {
        //     let mut v2 = vec![PLAYER_JOIN];
        //     v2.extend_from_slice(recp.username.as_bytes());
        //     // send each user OUR username
        //     recp.sender.unbounded_send(Message::Binary(v.clone())).unwrap();
        //     // send client each user's username
        //     tx.unbounded_send(Message::Binary(v2)).unwrap();
        // }
    
        drop(plock);
        drop(rooms);
    
        let (outgoing, incoming) = ws_stream.split();
    
        // let broadcast_incoming = incoming.try_for_each(|msg| {
        //     let pmap = self.peer_map.clone();
        //     async move {
        //         // println!("Received a message from {}: {}", addr, msg.to_text().unwrap());
        //         println!("Received a message from {}", self.addr);
        //         // current sends to every peer
        //         let peers = pmap.lock().await;
    
        //         // We want to broadcast the message to everyone except ourselves.
        //         let broadcast_recipients =
        //             peers.iter().filter(|(peer_addr, _)| peer_addr != &&self.addr).map(|(_, player)| player);
    
        //         for recp in broadcast_recipients {
        //             recp.sender.unbounded_send(msg.clone()).unwrap();
        //         }
    
        //         Ok(())
        //         // future::ok(())
        //     }
        // });

        let broadcast_incoming = incoming.try_for_each(|msg| self.recv(msg));
    
        let receive_from_others = rx.map(Ok).forward(outgoing);
    
        pin_mut!(broadcast_incoming, receive_from_others);
        future::select(broadcast_incoming, receive_from_others).await;
        self.disonnect().await;
    }

    pub async fn disonnect(&self) {
        // super scuffed code
        println!("{} disconnected", &self.addr);
        self.peer_map.lock().await.remove(&self.addr);
        let mut rooms = self.rooms.lock().await;
        // this SHOULD NOT fail
        if let Some(room) = rooms.get_mut(&self.room.clone()) {
            // println!("Before removal: {}", room.players.len());
            room.players.retain(|p| {
                // println!("Addr: {}, {}, eq: {}", p.addr, self.addr, p.addr == self.addr);
                p.addr != self.addr
            });
            // println!("Before removal: {}", room.players.len());
            if room.players.is_empty() {
                rooms.remove(&self.room.clone());
            }
        }
    }
}