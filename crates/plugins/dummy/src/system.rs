use arcana::{
    ecs::{resources::Res, system::State},
    gametime::{ClockStep, TimeSpan, TimeStamp},
    tracing,
};

#[arcana::system]
fn dummy_system(clock: Res<ClockStep>, mut last: State<Option<TimeStamp>>) {
    let last = last.get_or_insert(TimeStamp::start());

    if *last + TimeSpan::SECOND <= clock.now {
        tracing::info!("Dummy system: {}", clock.now);
        *last = clock.now;
    }
}
