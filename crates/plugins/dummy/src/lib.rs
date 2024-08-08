use arcana::input::Input;

arcana::declare_plugin!();

mod system;

#[arcana::filter]
fn dummy_filter(input: &Input) -> bool {
    false
}
