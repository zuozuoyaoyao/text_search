use anyhow::Result;

fn parse_port_arg() -> Option<u16> {
    let args: Vec<String> = std::env::args().collect();
    let pos = args.iter().position(|a| a == "--port")?;
    match args.get(pos + 1).and_then(|v| v.parse::<u16>().ok()) {
        Some(port) => Some(port),
        None => {
            eprintln!("Error: invalid value for --port");
            std::process::exit(1);
        }
    }
}

fn main() -> Result<()> {
    let (db, config) = text_search::init_app()?;
    tracing::info!("Starting Text Search backend");

    #[cfg(feature = "with-ws-server")]
    {
        let port = parse_port_arg();
        text_search::run_ws_server(db, config, port)?;
    }

    tracing::info!("Application exited");
    Ok(())
}
