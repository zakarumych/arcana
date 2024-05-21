//! This module provides a type that can be used to replace `TypeId` and be guarnateed to be stable.
//!
//! `Stid` is 128bit long semi-random value that is associated with a type
//! via derive macro or manual implementation of `WithStid` trait.
//!

pub use ::arcana_proc::{with_stid, WithStid};

crate::make_id! {
    /// Stable Type Identifier.
    /// Assigned to type by engine and type author in plugin.
    pub Stid;
}

pub trait WithStid {
    fn stid() -> Stid
    where
        Self: Sized;

    fn stid_dyn(&self) -> Stid;
}

impl Stid {
    pub fn of<T>() -> Self
    where
        T: WithStid,
    {
        T::stid()
    }

    pub fn of_val<T>(value: &T) -> Self
    where
        T: WithStid + ?Sized,
    {
        value.stid_dyn()
    }
}

with_stid!(::edict::entity::EntityId);
