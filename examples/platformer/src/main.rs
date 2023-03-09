// use nothing::{
//     gametime::{Clock, FrequencyNumExt, FrequencyTicker},
//     winit, Event, EventLoop, EventLoopBuilder,
// };

use nothing::{run_game, Game};

fn main() {
    run_game(|| async { Game {} });
}
