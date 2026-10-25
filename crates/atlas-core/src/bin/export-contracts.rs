use atlas_core::typescript_contracts;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let destination = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/contracts.gen.ts");
    let generated = typescript_contracts();
    if std::env::args().any(|arg| arg == "--write") {
        std::fs::write(&destination, generated)?;
        println!("Generated {}", destination.display());
    } else {
        let current = std::fs::read_to_string(&destination)?;
        if current.replace("\r\n", "\n") != generated {
            return Err("Contract bindings are stale; run mise run contracts:generate".into());
        }
        println!("Generated TypeScript contracts match Rust definitions");
    }
    Ok(())
}
