use std::env;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=assets/brand/agenticpdf.ico");
    if env::var("CARGO_CFG_TARGET_OS")? != "windows" {
        return Ok(());
    }
    winresource::WindowsResource::new()
        .set_icon("assets/brand/agenticpdf.ico")
        .compile()
        .map_err(|error| {
            format!(
                "failed to compile Windows icon resource from assets/brand/agenticpdf.ico: {error}"
            )
        })?;
    Ok(())
}
