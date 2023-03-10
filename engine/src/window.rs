//! Basic windowing.

use edict::World;
use winit::{event::WindowEvent, window::WindowId};

use crate::{events::Event, funnel::Filter, game::Quit};
