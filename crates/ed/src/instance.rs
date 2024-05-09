//! Running instance of the project.

use std::alloc::System;

use arcana::{
    flow::{execute_flows, Flows},
    gametime::{ClockRate, TimeSpan},
    plugin::PluginsHub,
    project::Plugin,
    work::WorkGraph,
    ClockStep, FrequencyTicker, World,
};

use crate::{
    data::ProjectData,
    filters::Filters,
    systems::{self, Schedule, Systems},
};

/// Instance of the project.
pub struct Instance {
    /// Own ECS world.
    world: World,

    /// Plugins initialization hub.
    hub: PluginsHub,

    /// Specifies frequency of fixed updates.
    fix: FrequencyTicker,

    /// Limits variable updates.
    lim: FrequencyTicker,

    /// Instance rate.
    rate: ClockRate,

    /// Flows to run on each tick.
    flows: Flows,

    workgraph: WorkGraph,
}

impl Instance {
    pub fn rate(&self) -> &ClockRate {
        &self.rate
    }

    pub fn rate_mut(&mut self) -> &mut ClockRate {
        &mut self.rate
    }

    pub fn tick(&mut self, span: TimeSpan, schedule: &Schedule) {
        let last_now = self.rate.now();
        let step = self.rate.step(span);

        self.fix.with_ticks(step.step, |fix_now| {
            self.world.insert_resource(ClockStep {
                now: fix_now,
                step: fix_now - last_now,
            });
            schedule.run(systems::Category::Fix, &mut self.world, &mut self.hub);
        });

        self.world.insert_resource(step);
        if self.lim.tick_count(step.step) > 0 {
            schedule.run(systems::Category::Var, &mut self.world, &mut self.hub);
        }

        execute_flows(&mut self.world, &mut self.flows);
    }

    pub fn render(&mut self) {}
}
