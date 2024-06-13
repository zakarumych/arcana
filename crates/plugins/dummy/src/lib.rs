use arcana::input::Input;

arcana::plugin_declare!();

mod system;

#[arcana::filter]
fn dummy_filter(input: &Input) -> bool {
    false
}
