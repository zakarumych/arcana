bitflags::bitflags! {
    /// Set of features that can be requested from the device.
    /// The device creation will fail if the device does not support all of the requested features.
    /// Check the capabilities of the device to see which features are supported.
    pub struct Features: u32 {}
}
