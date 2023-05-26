pub fn main() -> miette::Result<()> {
    let root = env!("CARGO_MANIFEST_DIR");
    ed::run(root.as_ref())
}
