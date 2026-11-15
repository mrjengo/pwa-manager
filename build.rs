fn main() {
    // Closest built-in Slint style to each platform's native look.
    let style = match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("macos") => "cupertino",
        Ok("linux") => "cosmic",
        _ => "fluent",
    };
    let config = slint_build::CompilerConfiguration::new().with_style(style.into());
    slint_build::compile_with_config("ui/app.slint", config).expect("Slint UI compiles");
}
