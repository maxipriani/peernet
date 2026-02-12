use clap::Parser;
use peernet_core::{DhtKey, DhtValue, GossipPayload, NetworkEvent, PeernetError, PeernetResult};
use peernet_network::{NetworkConfig, NetworkHandle};
use std::io::{self, BufRead, Write};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{Level, info, warn};
use tracing_subscriber::{EnvFilter, fmt};

#[derive(Parser, Debug)]
#[command(name = "peernet")]
struct Args {
    #[arg(short, long, default_value = "0")]
    port: u16,

    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[derive(Debug)]
enum InputEvent {
    Send(GossipPayload),
    Put { key: DhtKey, value: DhtValue },
    Get { key: DhtKey },
    Quit,
}

#[tokio::main]
async fn main() -> PeernetResult<()> {
    let args = Args::parse();
    init_tracing(args.verbose);

    let cancel_token = CancellationToken::new();
    spawn_signal_handler(cancel_token.clone());

    let config = NetworkConfig {
        port: args.port,
        ..Default::default()
    };
    let mut network = peernet_network::spawn(config, cancel_token.clone());

    match network.recv().await {
        Some(NetworkEvent::Started {
            local_peer_id,
            listening_on,
        }) => {
            println!();
            println!("PEERNET");
            println!("  PeerId:    {local_peer_id}");
            println!("  Listening: {listening_on}");
            println!("  Type 'help' for commands");
            println!();
        }
        Some(other) => {
            warn!(?other, "unexpected startup event");
        }
        None => {
            return Err(PeernetError::ChannelClosed {
                actor: "network",
                reason: "closed before startup",
            });
        }
    }

    let (input_tx, input_rx) = mpsc::channel::<InputEvent>(32);
    let input_handle = spawn_input_handler(cancel_token.clone(), input_tx);

    run_main_loop(&mut network, input_rx, cancel_token.clone()).await;

    let _ = input_handle.await;
    Ok(())
}

fn init_tracing(verbose: u8) {
    let level = match verbose {
        0 => Level::INFO,
        1 => Level::DEBUG,
        _ => Level::TRACE,
    };

    let filter = EnvFilter::from_default_env()
        .add_directive(format!("peernet={level}").parse().unwrap())
        .add_directive("libp2p=warn".parse().unwrap());

    fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_file(false)
        .with_line_number(false)
        .init();
}

fn spawn_signal_handler(cancel_token: CancellationToken) {
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        println!();
        info!("shutting down...");
        cancel_token.cancel();
    });
}

fn spawn_input_handler(
    cancel_token: CancellationToken,
    input_tx: mpsc::Sender<InputEvent>,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        let stdin = io::stdin();
        let mut stdout = io::stdout();

        loop {
            if cancel_token.is_cancelled() {
                break;
            }

            let _ = write!(stdout, "peernet> ");
            let _ = stdout.flush();

            let mut line = String::new();
            match stdin.lock().read_line(&mut line) {
                Ok(0) => {
                    cancel_token.cancel();
                    break;
                }
                Ok(_) => {
                    let input = line.trim();
                    if input.is_empty() {
                        continue;
                    }
                    if let Some(event) = handle_input(input, &cancel_token)
                        && input_tx.blocking_send(event).is_err()
                    {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    })
}

fn handle_input(input: &str, cancel_token: &CancellationToken) -> Option<InputEvent> {
    let parts: Vec<&str> = input.splitn(3, ' ').collect();
    let cmd = parts.first()?;

    match *cmd {
        "quit" | "exit" | "q" => {
            cancel_token.cancel();
            Some(InputEvent::Quit)
        }

        "help" | "?" => {
            println!();
            println!("  send <message>      broadcast message");
            println!("  put <key> <value>   store in DHT");
            println!("  get <key>           retrieve from DHT");
            println!("  quit                exit");
            println!();
            None
        }

        "send" | "s" => {
            let msg = input.split_once(' ').map(|x| x.1).unwrap_or("");
            if msg.is_empty() {
                println!("usage: send <message>");
                return None;
            }
            match GossipPayload::from_text(msg) {
                Ok(payload) => Some(InputEvent::Send(payload)),
                Err(e) => {
                    println!("error: {e}");
                    None
                }
            }
        }

        "put" => match (parts.get(1), parts.get(2)) {
            (Some(k), Some(v)) => {
                let key = match DhtKey::new(*k) {
                    Ok(k) => k,
                    Err(e) => {
                        println!("error: {e}");
                        return None;
                    }
                };
                let value = match DhtValue::new(v.as_bytes().to_vec()) {
                    Ok(v) => v,
                    Err(e) => {
                        println!("error: {e}");
                        return None;
                    }
                };
                Some(InputEvent::Put { key, value })
            }
            _ => {
                println!("usage: put <key> <value>");
                None
            }
        },

        "get" => match parts.get(1) {
            Some(k) => match DhtKey::new(*k) {
                Ok(key) => Some(InputEvent::Get { key }),
                Err(e) => {
                    println!("error: {e}");
                    None
                }
            },
            None => {
                println!("usage: get <key>");
                None
            }
        },

        _ => {
            println!("unknown command: {cmd}");
            None
        }
    }
}

async fn run_main_loop(
    network: &mut NetworkHandle,
    mut input_rx: mpsc::Receiver<InputEvent>,
    cancel_token: CancellationToken,
) {
    loop {
        tokio::select! {
            () = cancel_token.cancelled() => {
                let _ = network.shutdown().await;
                break;
            }

            Some(input) = input_rx.recv() => {
                match input {
                    InputEvent::Send(payload) => {
                        let _ = network.publish(payload).await;
                    }
                    InputEvent::Put { key, value } => {
                        let _ = network.put(key, value).await;
                    }
                    InputEvent::Get { key } => {
                        let _ = network.get(key).await;
                    }
                    InputEvent::Quit => {}
                }
            }

            event = network.recv() => {
                match event {
                    Some(NetworkEvent::ShutdownComplete) => break,

                    Some(NetworkEvent::PeerDiscovered { peer_id }) => {
                        println!("[discovered] {}...", &peer_id.to_string()[..12]);
                    }
                    Some(NetworkEvent::PeerConnected { peer_id }) => {
                        println!("[connected] {}...", &peer_id.to_string()[..12]);
                    }
                    Some(NetworkEvent::PeerDisconnected { peer_id }) => {
                        println!("[disconnected] {}...", &peer_id.to_string()[..12]);
                    }

                    Some(NetworkEvent::GossipMessage { source, payload, .. }) => {
                        let from = source
                            .map(|p| format!("{}...", &p.to_string()[..12]))
                            .unwrap_or_else(|| "unknown".into());
                        let text = payload.as_str().unwrap_or("<binary>");
                        println!("[message] {from}: {text}");
                    }

                    Some(NetworkEvent::RecordStored { key }) => {
                        println!("[stored] {key}");
                    }
                    Some(NetworkEvent::RecordStoreFailed { key, reason }) => {
                        println!("[store failed] {key}: {reason}");
                    }
                    Some(NetworkEvent::RecordFound { key, value }) => {
                        let text = std::str::from_utf8(value.as_bytes())
                            .unwrap_or("<binary>");
                        println!("[found] {key} = {text}");
                    }
                    Some(NetworkEvent::RecordNotFound { key }) => {
                        println!("[not found] {key}");
                    }

                    Some(NetworkEvent::CommandFailed { reason }) => {
                        println!("[error] {reason}");
                    }

                    Some(_) => {}
                    None => break,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_parses_defaults() {
        let args = Args::parse_from(["peernet"]);
        assert_eq!(args.port, 0);
        assert_eq!(args.verbose, 0);
    }

    #[test]
    fn cli_parses_port() {
        let args = Args::parse_from(["peernet", "-p", "4001"]);
        assert_eq!(args.port, 4001);
    }
}
