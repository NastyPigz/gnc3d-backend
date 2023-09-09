// u16 is used to distinguish txt and binary events
pub const USER_DATA_EVENT: u8 = 0;
// pub const USER_STATE_UPDATE: u8 = 1;
pub const DIED_OF_DEATH: u8 = 2;
pub const CAKE_SPAWN_EVENT: u8 = 3;
// pub const CAKE_COLLIDE_EVENT: u8 = 4;
// pub const PLAYER_LEAVE_EVENT: u8 = 5;
// pub const CAKES_UPDATE_EVENT: u8 = 6;
pub const START_LOBBY_EVENT: u8 = 7;
pub const TXT_MESSAGE_CREATE: u16 = 8;
pub const USER_NAME_EVENT: u16 = 9;
// pub const CAKE_FINAL_EVENT: u8 = 10;
// pub const CAKE_GONE_EVENT: u8 = 11;
pub const BARRICADE_SPAWN_EVENT: u8 = 12;
// pub const BARRICADE_FINAL_EVENT: u8 = 13;
pub const BARRICADE_GONE_EVENT: u8 = 14;

pub const GERMINATION_EVENT: u8 = 42;
pub const IS_HOST_EVENT: u8 = 69;