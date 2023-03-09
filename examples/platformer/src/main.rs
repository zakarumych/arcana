use nothing::{
    gametime::{Clock, FrequencyNumExt, FrequencyTicker},
    winit, Event, EventLoop, EventLoopBuilder,
};

fn main() {
    EventLoopBuilder::new().run(|events| async move {
        let window = winit::window::Window::new(&events).unwrap();

        let mut clocks = Clock::new();
        let mut ticker = 7u32.hz().ticker(clocks.now());

        loop {
            let events = events.next_rate(&clocks, &ticker).await;
            let mut events = events.peekable();
            if events.peek().is_none() {
                println!("No events");
            }
            for event in events {
                println!("{:?}", event);

                match event {
                    Event::WindowEvent { event, .. } => match event {
                        winit::event::WindowEvent::CloseRequested => {
                            drop(window);
                            return;
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
            clocks.step();
            for tick in ticker.ticks(clocks.now()) {
                println!("Tick {tick:?}");
            }
        }
    });
}
