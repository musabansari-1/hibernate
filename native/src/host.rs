use hibernate_native::{store, wire};
use serde_json::json;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().nth(1).as_deref() == Some("--service-break") {
        let args: Vec<String> = std::env::args().collect();
        if args.len() != 4 {
            return Err("Usage: --service-break SERVICE_ID SHUTDOWN_ID".into());
        }
        hibernate_native::services::signal_pending_shutdown(&args[2], &args[3])?;
        return Ok(());
    }
    if std::env::args().nth(1).as_deref() == Some("--restore-folder") {
        let args: Vec<String> = std::env::args().collect();
        if args.len() != 4 {
            return Err("Usage: --restore-folder explorer|terminal ABSOLUTE_LOCAL_FOLDER".into());
        }
        println!(
            "{}",
            hibernate_native::context::restore_folder(&args[2], &args[3])?
        );
        return Ok(());
    }
    if std::env::args().nth(1).as_deref() == Some("--terminal-report") {
        let args: Vec<String> = std::env::args().collect();
        if args.len() != 5 {
            return Err("Invalid terminal report".into());
        }
        hibernate_native::terminal::report(&args[2], args[3].parse()?, &args[4])?;
        return Ok(());
    }
    if matches!(
        std::env::args().nth(1).as_deref(),
        Some("--review-close" | "--quick-sleep")
    ) {
        let args: Vec<String> = std::env::args().collect();
        if args.len() != 4 || args[3] != "--confirm-saved-work" {
            return Err("Usage: --review-close SNAPSHOT --confirm-saved-work (or --quick-sleep). Review the captured state and save application work first.".into());
        }
        println!(
            "{}",
            store::close_reviewed(args[2].clone(), true, args[1] == "--quick-sleep")?
        );
        return Ok(());
    }
    if std::env::args().nth(1).as_deref() == Some("--memory") {
        println!("{}", hibernate_native::platform::memory()?);
        return Ok(());
    }
    if std::env::args().nth(1).as_deref() == Some("--diagnose") {
        println!("{}", store::dashboard()?);
        return Ok(());
    }
    if std::env::args().nth(1).as_deref() == Some("--enqueue") {
        let args: Vec<String> = std::env::args().collect();
        if args.len() < 4 {
            return Err("Usage: --enqueue CLIENT ACTION [TARGET|-] [SNAPSHOT]".into());
        }
        println!(
            "{}",
            store::enqueue(
                args[2].clone(),
                args[3].clone(),
                args.get(4).filter(|s| s.as_str() != "-").cloned(),
                args.get(5).cloned()
            )?
        );
        return Ok(());
    }
    let mut db = store::open()?;
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    let mut connection: Option<(String, String)> = None;
    let mut reported: Option<(String, String)> = None;
    let outcome = (|| -> Result<(), Box<dyn std::error::Error>> {
        while let Some(request) = wire::read(&mut input)? {
            let operation = if request["op"] == "beginVscodeClose" {
                (|| -> Result<serde_json::Value, String> {
                    let (client, instance) = connection
                        .as_ref()
                        .ok_or("Connect the VS Code window before close observation")?;
                    if request["client"] != *client {
                        return Err(
                            "Another window or close observation owns this connection".into()
                        );
                    }
                    hibernate_native::native_close::begin(
                        &mut db,
                        client,
                        instance,
                        request["jobId"].as_str().ok_or("Missing close job")?,
                        request["snapshotId"].as_str().ok_or("Missing snapshot")?,
                    )
                })()
            } else {
                store::handle(&mut db, &request)
            };
            let result = match operation {
                Ok(value) => {
                    if request["op"] == "poll" {
                        if let (Some(client), Some(adapter)) =
                            (request["client"].as_str(), request["adapter"].as_str())
                        {
                            reported = Some((client.into(), adapter.into()));
                        }
                        if request["adapter"] == "vscode" {
                            if let (Some(client), Some(instance)) = (
                                request["client"].as_str(),
                                request["body"]["instance"].as_str(),
                            ) {
                                connection = Some((client.into(), instance.into()));
                            }
                        }
                    }
                    json!({"requestId":request["requestId"],"ok":true,"value":value})
                }
                Err(e) => json!({"requestId":request["requestId"],"ok":false,"error":e}),
            };
            wire::write(&mut output, &result)?;
        }
        Ok(())
    })();
    if let Some((client, instance)) = connection {
        store::integration_disconnected(&mut db, &client, &instance)?;
    } else if let Some((client, _adapter)) = reported {
        store::integration_report_disconnected(&mut db, &client)?;
    }
    outcome
}
