// use nothing::{
//     gametime::{Clock, FrequencyNumExt, FrequencyTicker},
//     winit, Event, EventLoop, EventLoopBuilder,
// };

use airy::game::run_game;

fn main() {
    run_game(|game| async move {});
}
