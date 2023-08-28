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

use crate::{server, constants::{USER_DATA_EVENT, CAKE_SPAWN_EVENT, START_LOBBY_EVENT, TXT_MESSAGE_CREATE, USER_NAME_EVENT}};

pub struct GameSession {
    // pub peer_map: server::PeerMap,
    pub rooms: server::RoomMap,
    pub addr: SocketAddr,
    pub username: String,
    pub skin: u8,
    pub seed: usize,
    pub room: String,
    pub id: Option<u8>
}

// modern problems require modern solutions
unsafe impl std::marker::Send for GameSession {}
unsafe impl std::marker::Sync for GameSession {}

impl GameSession {
    async fn recv(&self, msg: Message) -> Result<(), Error> {
        // let peers = self.peer_map.lock().await;
        // println!("Receiving");
        let mut rooms = self.rooms.lock().await;
        // println!("Ok(rooms)");
        // println!("{:?}", msg);

        if let Some(room) = rooms.remove(&self.room.clone()) {
            let mut started = room.started;
            let mut results = Vec::new();
            // println!("Room has players {}", room.players.len());
            for recp in room.players {
                let mut v = msg.clone().into_data();
                // println!("recp: {}, self: {}", recp.id, self.id.unwrap());
                if recp.addr != self.addr {
                    // println!("{}", recp.username);
                    // this is a peer functionality...
                    // if the client sent a string, other endpoints will NOT be happy.
                    match msg {
                        // we have to do results.push(Ok(recp)) or else it will lead to undefined behaviour
                        // maybe we can utilize Message::Text
                        Message::Text(_) => {
                            // hopefully nobody is trolling me and this panics
                            let mut txt = msg.clone().into_text().unwrap();
                            // text standard format:
                            // event_code + message
                            // note that both are std::option::Option
                            if txt.chars().next() == char::from_digit(TXT_MESSAGE_CREATE.into(), 10) {
                                // WHO NEEDS JSON WHEN YOU HAVE COMMAS?!?!
                                txt.insert_str(1, &format!(",{},", self.id.unwrap()));
                                results.push(
                                    recp.sender.unbounded_send(Message::Text(txt)).map(|_| recp)
                                        .map_err(|_| println!("Dropping session!")),
                                );
                            } else {
                                results.push(Ok(recp));
                            }
                        },
                        Message::Binary(_) => {
                            // because matching msg.clone() probably takes more time
                            if v.is_empty() {
                                // this shouldn't happen but it will crazily error if it is the case
                                // we are gonna assume this guy is disconnected and deleting him from players list
                                continue;
                            }
                            if v.len() == 4*11 || v.len() == 4*9 || v.len() == 4*2 || v.len() == 4*8 {
                                // this shit too so long to debug, JS ws uses little endian
                                let bytes = (self.id.unwrap() as f32).to_le_bytes();
                                v.insert(4, bytes[0]);
                                v.insert(5, bytes[1]);
                                v.insert(6, bytes[2]);
                                v.insert(7, bytes[3]);
                            } else if v.len() == 4*10 {
                                let user_id = f32::from_le_bytes([v[4], v[5], v[6], v[7]]) as u8;
                                if recp.id == user_id {
                                    let mut new_v: Vec<u8> = Vec::new();
                                    new_v.extend_from_slice(&(CAKE_SPAWN_EVENT as f32).to_le_bytes());
                                    new_v.extend_from_slice(&(self.id.unwrap() as f32).to_le_bytes());
                                    // from 9th and beyond are cake data
                                    new_v.extend_from_slice(&v[8..]);
                                    results.push(
                                        recp.sender.unbounded_send(Message::Binary(new_v)).map(|_| recp)
                                            .map_err(|_| println!("Dropping session!")),
                                    );
                                    continue;
                                }
                            } else if v[0] == START_LOBBY_EVENT && self.id.unwrap() == 0 {
                                started = true;
                            } else {
                                v.insert(1, self.id.unwrap());
                            }
                            // results.push(recp.sender.unbounded_send(Message::Binary(v)));
                            // let res = session.session.text(serde_json::to_string(&msg).unwrap()).await;
                            results.push(
                                recp.sender.unbounded_send(Message::Binary(v)).map(|_| recp)
                                    .map_err(|_| println!("Dropping session!")),
                            );
                        },
                        Message::Close(_) => results.push(Ok(recp)), /* disconnect function should handle this? */
                        Message::Frame(_) => results.push(Ok(recp)),
                        // the connection still works without ping and pong so we are gonna save some bytes here
                        Message::Ping(_) => results.push(Ok(recp)),
                        Message::Pong(_) => results.push(Ok(recp)),
                    }

                } else if !v.is_empty() && v[0] == 7 {
                    started = true;
                    results.push(
                        recp.sender.unbounded_send(msg.clone()).map(|_| recp)
                            .map_err(|_| println!("Dropping session!")),
                    );
                } else {
                    results.push(Ok(recp));
                }
            }
            // no need to do stuff like sending close events, the "disconnect" f'n would've been called already
            // if they actually disconnected
            // println!("Insertion failed here");
            // println!("Hmm, {:?}", results);
            rooms.insert(self.room.clone(), server::Room {
                seed: room.seed,
                started,
                players: results.into_iter().filter_map(|i| i.ok()).collect()
            });
        }

        // let broadcast_recipients = rooms.entry(self.room.clone()).or_insert(server::Room {
        //     seed: self.seed,
        //     players: Vec::new()
        // });
        //     // peers.iter().filter(|(peer_addr, _)| peer_addr != &&self.addr).map(|(_, player)| player);

        // let mut results = Vec::new();
        // for recp in broadcast_recipients.players.iter() {
        //     // println!("{}", recp.username);
        //     // do not send to self
        //     if recp.addr != self.addr {
        //         // this is a peer functionality...
        //         // if the client sent a string, other endpoints will NOT be happy.
        //         let mut v = msg.clone().into_data();
        //         v.insert(1, self.id.unwrap());
        //         results.push(recp.sender.unbounded_send(Message::Binary(v)));
        //     }
        // }
        
        Ok(())
    }

    // had to do this... self.id cannot be assigned otherwise
    pub async fn start(&mut self, ws_stream: WebSocketStream<Upgraded>) {
        println!("WebSocket connection established: {}", self.addr);

        // Insert the write part of this peer to the peer map.
        // tx = send, rx = receive
        let (tx, rx) = unbounded();
        let room_name = self.room.clone();
        let room_lck = self.rooms.lock().await;

        let id: u8 = if room_lck.get(&room_name).is_some() {
            room_lck.len() as u8
        } else { 0 };

        drop(room_lck);
        
        // let mut plock = self.peer_map.lock().await;
        let player = server::Player {
            sender: tx.clone(),
            username: self.username.clone(),
            skin: self.skin,
            addr: self.addr,
            id
        };
        // plock.insert(self.addr, player.clone());

        // drop(plock);

        let mut rooms = self.rooms.lock().await;
        // keep in mind that the first player is the host.
        let entry = if let Some(room) = rooms.get_mut(&room_name) {
            // if room already exists then we completely ignore the seed value
            println!("Just adding");
            // count starts from 0...
            self.id = Some(room.players.len() as u8);
            // send valid seed
            tx.unbounded_send(Message::Binary(vec![42, room.seed as u8])).unwrap();
            room.players.push(player.clone());
            room
        } else {
            // 69 means the client is the host and he is supposed to send in the cake coords
            println!("New");
            // 69 also means the player id is 0
            self.id = Some(0);
            // if this errors then the GameSession might as well close and not do anything
            tx.unbounded_send(Message::Binary(vec![69])).unwrap();
            rooms.entry(room_name).or_insert(server::Room {
                started: false,
                seed: self.seed,
                players: vec![player.clone()]
            })
        };

        // drop(rooms);
        // let entry = rooms
        //     .entry(self.room.clone())
        //     .or_insert_with(Vec::new);

        // entry.push(player);
        
        // from this point on self.id is definitely Some(u8)

        // sending username is no longer a uint8
        // v.extend_from_slice(self.username.as_bytes());

        // send everyone in the room our current information
        // let mut results = Vec::new();
        for recp in entry.players.clone().into_iter() {
            if recp.addr == self.addr {
                continue;
            }
            // v2.extend_from_slice(recp.username.as_bytes());
            recp.sender.unbounded_send(Message::Binary(vec![USER_DATA_EVENT, self.id.unwrap(), self.skin])).unwrap();
            recp.sender.unbounded_send(Message::Text(USER_NAME_EVENT.to_string() + "," + &self.id.unwrap().to_string() + "," + &self.username)).unwrap();
            // send each user OUR username
            // results.push(
            //     recp.sender.unbounded_send(Message::Binary(v.clone())).map(|_| recp)
            //         .map_err(|_| println!("Dropping session")),
            // );
            // send client each user's id, sex, then username
            tx.unbounded_send(Message::Binary(vec![USER_DATA_EVENT, recp.id, recp.skin])).unwrap();
            tx.unbounded_send(Message::Text(USER_NAME_EVENT.to_string() + "," + &recp.id.to_string() + "," + &recp.username)).unwrap();
        }
        // entry.players = results.into_iter().filter_map(|i| i.ok()).collect();
        // entry.players.push(player);

        // for recp in plock.iter().filter(|(peer_addr, _)| peer_addr != &&self.addr).map(|(_, player)| player) {
        //     let mut v2 = vec![PLAYER_JOIN];
        //     v2.extend_from_slice(recp.username.as_bytes());
        //     // send each user OUR username
        //     recp.sender.unbounded_send(Message::Binary(v.clone())).unwrap();
        //     // send client each user's username
        //     tx.unbounded_send(Message::Binary(v2)).unwrap();
        // }
    
        // drop(plock);
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
        // self.peer_map.lock().await.remove(&self.addr);
        let mut rooms = self.rooms.lock().await;

        if self.id.unwrap() == 0 {
            println!("host left room");
            if let Some(room) = rooms.remove(&self.room.clone()) {
                println!("Host's room has {} players", room.players.len());
                for p in room.players {
                    // dont give a shit whether or not they are still connected, close if they are here
                    // no longer using .unwrap_unchecked()
                    // unsafe {
                        p.sender.unbounded_send(Message::Close(None)).unwrap();
                    // }
                }
            }
            return
        }

        // this SHOULD NOT fail
        if let Some(room) = rooms.get_mut(&self.room.clone()) {
            // println!("Before removal: {}", room.players.len());
            room.players.retain(|p| {
                // println!("Addr: {}, {}, eq: {}", p.addr, self.addr, p.addr == self.addr);
                // probably better than using id
                if p.addr != self.addr {
                    // tell them this person left :( also dgaf about whether or not they are connected
                    // the if statement below this is garbage collection, and the recv loop will probably filter anyways
                    // the only possible scenario is when the host somehow got an unwrap_unchecked...
                    // println!("Sending close message to {}", p.id);
                    // no longer using .unwrap_unchecked()
                    // unsafe {
                        p.sender.unbounded_send(Message::Binary(vec![5, self.id.unwrap()])).unwrap();
                    // }
                    true
                } else {
                    // remove this guy
                    false
                }
            });
            // println!("Before removal: {}", room.players.len());
            // Arc will probably pick up all the GameSession & other objects???
            if room.players.is_empty() {
                rooms.remove(&self.room.clone());
            }
        }
        // if the room doesn't exist anymore it is likely that the host left.
    }
}