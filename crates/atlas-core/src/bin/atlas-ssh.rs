//! Controlled helper: explicit config/alias, fixed connection check only.
use atlas_core::{operations::*, ssh::*, *};
use std::{path::PathBuf, time::Duration};
fn run() -> Result<String, AtlasError> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err(AtlasError::new(ErrorCode::InvalidInput));
    }
    let timeout: u64 = args[2]
        .parse()
        .map_err(|_| AtlasError::new(ErrorCode::InvalidInput))?;
    let broker = OperationBroker::new(Duration::from_millis(timeout), 64 * 1024)?;
    let authority = broker.issue_backend_authority()?;
    let target = ServerTarget::new(ServerId::parse("fixture")?, &args[1])?;
    let ticket = broker.prepare_connection_check(Some(&authority), &target)?;
    #[cfg(windows)]
    let executable =
        PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()))
            .join("System32/OpenSSH/ssh.exe");
    #[cfg(not(windows))]
    let executable = PathBuf::from("ssh");
    let options = SshOptions::configured(executable, PathBuf::from(&args[0]));
    let alias = args[1].clone();
    let result = broker.execute_connection_check(
        Some(&authority),
        &ticket.operation_id,
        &target,
        move |_, context| {
            let result = options.verify(&alias, context)?;
            context.write(
                &serde_json::to_string(&result)
                    .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?,
            )
        },
    );
    while broker.active_workers() != 0 {
        std::thread::sleep(Duration::from_millis(5));
    }
    result
}
fn main() {
    match run() {
        Ok(result) => println!("{result}"),
        Err(error) => {
            eprintln!("{}", serde_json::to_string(&error).expect("fixed error"));
            std::process::exit(1);
        }
    }
}
