use atlas_core::*;
use std::path::Path;

fn run(args: &[String]) -> Result<serde_json::Value, AtlasError> {
    let invalid = || AtlasError::new(ErrorCode::InvalidInput);
    let arg = |n: usize| args.get(n).map(String::as_str).ok_or_else(invalid);
    match arg(0)? {
        "create" => {
            Store::create(Path::new(arg(1)?))?;
            Ok(serde_json::json!({"created":true}))
        }
        "migrate" => serde_json::to_value(migrate_legacy(
            Path::new(arg(1)?),
            Path::new(arg(2)?),
            Path::new(arg(3)?),
        )?)
        .map_err(|_| invalid()),
        "restore" => {
            restore_legacy(Path::new(arg(1)?), Path::new(arg(2)?))?;
            Ok(serde_json::json!({"restored":true}))
        }
        "state" => serde_json::to_value(Store::open(Path::new(arg(1)?))?.snapshot()?)
            .map_err(|_| invalid()),
        "import" => Ok(
            serde_json::json!({"imported":Store::open(Path::new(arg(1)?))?.import_reports(Path::new(arg(2)?))?}),
        ),
        "hold-lock" => {
            use std::io::Write;
            let _owner = Store::open(Path::new(arg(1)?))?;
            println!("{}", serde_json::json!({"locked":true}));
            std::io::stdout()
                .flush()
                .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?;
            let mut release = String::new();
            std::io::stdin()
                .read_line(&mut release)
                .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?;
            Ok(serde_json::json!({"released":true}))
        }
        "add-server" => {
            let input = serde_json::from_str(arg(2)?).map_err(|_| invalid())?;
            Ok(serde_json::json!({"id":Store::open(Path::new(arg(1)?))?.add_server(input)?}))
        }
        "update-server" => {
            let input = serde_json::from_str(arg(3)?).map_err(|_| invalid())?;
            Store::open(Path::new(arg(1)?))?.update_server(&ServerId::parse(arg(2)?)?, input)?;
            Ok(serde_json::json!({"updated":true}))
        }
        "remove-server" => {
            Store::open(Path::new(arg(1)?))?.remove_server(&ServerId::parse(arg(2)?)?)?;
            Ok(serde_json::json!({"removed":true}))
        }
        "add-edge" => {
            let input = serde_json::from_str(arg(2)?).map_err(|_| invalid())?;
            Ok(serde_json::json!({"id":Store::open(Path::new(arg(1)?))?.add_relationship(input)?}))
        }
        "remove-edge" => {
            Store::open(Path::new(arg(1)?))?.remove_relationship(&EdgeId::parse(arg(2)?)?)?;
            Ok(serde_json::json!({"removed":true}))
        }
        _ => Err(invalid()),
    }
}
fn main() {
    match run(&std::env::args().skip(1).collect::<Vec<_>>()) {
        Ok(value) => println!("{value}"),
        Err(error) => {
            eprintln!(
                "{}",
                serde_json::to_string(&error).expect("safe error serialization")
            );
            std::process::exit(1);
        }
    }
}
