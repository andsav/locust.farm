use clap::{Parser, Subcommand};
use locust_farm::{Config, Service};
use std::{error::Error, net::SocketAddr, path::PathBuf};

#[derive(Parser)]
#[command(about = "Restricted public farm publication service")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Serve {
        #[arg(long, default_value = "127.0.0.1:4319")]
        bind: SocketAddr,
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        allowlist: Option<PathBuf>,
        #[arg(long)]
        public_enrollment: bool,
        #[arg(long, default_value_t = 262144)]
        max_body_bytes: usize,
        #[arg(long, default_value_t = 1000)]
        min_mutation_interval_ms: u64,
        #[arg(long, default_value_t = 512)]
        max_streams: usize,
        #[arg(long, default_value_t = 500)]
        max_requests_per_second: u32,
        #[arg(long, default_value_t = 30)]
        retention_days: u64,
        #[arg(long, default_value = "Contact the deployment operator")]
        abuse_contact: String,
    },
    Enroll {
        #[arg(long)]
        database: PathBuf,
        farm_id: String,
    },
    TakeDown {
        #[arg(long)]
        database: PathBuf,
        farm_id: String,
    },
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    match Cli::parse().command {
        Command::Serve {
            bind,
            database,
            allowlist,
            public_enrollment,
            max_body_bytes,
            min_mutation_interval_ms,
            max_streams,
            max_requests_per_second,
            retention_days,
            abuse_contact,
        } => {
            let service = Service::open(
                database,
                Config {
                    max_body_bytes,
                    min_mutation_interval_ms,
                    max_streams,
                    max_requests_per_second,
                    public_enrollment,
                    retention_ms: retention_days.saturating_mul(86400000),
                    abuse_contact,
                },
            )?;
            if let Some(path) = allowlist {
                for line in std::fs::read_to_string(path)?.lines() {
                    let id = line.split('#').next().unwrap_or("").trim();
                    if !id.is_empty() {
                        service.enroll(id)?;
                    }
                }
            }
            let listener = tokio::net::TcpListener::bind(bind).await?;
            eprintln!(
                "locust-farm listening on {bind}; enrollment {}",
                if public_enrollment {
                    "public"
                } else {
                    "restricted"
                }
            );
            let maintenance = service.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
                loop {
                    interval.tick().await;
                    if let Err(error) = maintenance.expire() {
                        eprintln!("farm retention error: {error}");
                    }
                }
            });
            let shutdown_service = service.clone();
            axum::serve(listener, service.router())
                .with_graceful_shutdown(async move {
                    shutdown().await;
                    shutdown_service.shutdown();
                })
                .await?;
        }
        Command::Enroll { database, farm_id } => {
            Service::open(database, Config::default())?.enroll(&farm_id)?;
            println!("enrolled {farm_id}");
        }
        Command::TakeDown { database, farm_id } => {
            Service::open(database, Config::default())?.take_down(&farm_id)?;
            println!("unavailable {farm_id}");
        }
    }
    Ok(())
}

async fn shutdown() {
    #[cfg(unix)]
    {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut terminate) => {
                tokio::select! { _=tokio::signal::ctrl_c()=>{},_=terminate.recv()=>{} }
            }
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
