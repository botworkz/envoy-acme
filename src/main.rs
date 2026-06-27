//! Entry point for `envoy-acme`: a prototype Rust ext_proc + SDS service that
//! obtains and renews Let's Encrypt / ACME certificates for use with Envoy.
mod acme;
mod cert_store;
mod challenge_store;
mod config;
mod errors;
mod ext_proc;
mod sds;

use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;
use tonic::transport::Server;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use crate::acme::AcmeManager;
use crate::cert_store::CertStore;
use crate::challenge_store::ChallengeStore;
use crate::config::{Config, LogConfig};
use crate::ext_proc::ExtProcService;
use crate::sds::SdsService;

#[derive(Parser, Debug)]
#[command(
    name = "envoy-acme",
    about = "Prototype: Rust ext_proc + SDS service for Let's Encrypt / ACME with Envoy"
)]
struct Cli {
    /// Path to the YAML configuration file.
    #[arg(short, long, default_value = "config/example.yaml")]
    config: PathBuf,
}

fn init_tracing(cfg: &LogConfig) -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&cfg.level));
    let builder = tracing_subscriber::fmt().with_env_filter(filter);
    match cfg.format.as_str() {
        "json" => builder.json().init(),
        _ => builder.init(),
    }
    Ok(())
}

/// Resolve when the process receives SIGINT or SIGTERM.
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut term = signal(SignalKind::terminate()).expect("install SIGTERM handler");
        let mut int = signal(SignalKind::interrupt()).expect("install SIGINT handler");
        tokio::select! {
            _ = term.recv() => {},
            _ = int.recv() => {},
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let config = Config::load(&cli.config)
        .with_context(|| format!("loading config from {}", cli.config.display()))?;

    init_tracing(&config.log)?;

    info!(
        config = %cli.config.display(),
        domains = ?config.acme.domains,
        "starting envoy-acme"
    );

    let challenge_store = ChallengeStore::new();
    let cert_store = CertStore::new();

    let ext_proc_addr =
        config.ext_proc.listen.parse().with_context(|| {
            format!("parsing ext_proc listen address {}", config.ext_proc.listen)
        })?;
    let sds_addr = config
        .sds
        .listen
        .parse()
        .with_context(|| format!("parsing sds listen address {}", config.sds.listen))?;

    let ext_proc_service = ExtProcService::new(challenge_store.clone()).into_server();
    let sds_service =
        SdsService::new(cert_store.clone(), config.sds.resource_name.clone()).into_server();
    let acme_manager = AcmeManager::new(
        config.acme.clone(),
        challenge_store.clone(),
        cert_store.clone(),
    );

    info!(%ext_proc_addr, "starting ext_proc gRPC server");
    let ext_proc_task = tokio::spawn(async move {
        Server::builder()
            .add_service(ext_proc_service)
            .serve(ext_proc_addr)
            .await
    });

    info!(%sds_addr, "starting SDS gRPC server");
    let sds_task = tokio::spawn(async move {
        Server::builder()
            .add_service(sds_service)
            .serve(sds_addr)
            .await
    });

    let acme_task = tokio::spawn(async move { acme_manager.run().await });

    tokio::select! {
        res = ext_proc_task => {
            match res {
                Ok(Ok(())) => info!("ext_proc server exited"),
                Ok(Err(err)) => error!(%err, "ext_proc server failed"),
                Err(err) => error!(%err, "ext_proc task panicked"),
            }
        }
        res = sds_task => {
            match res {
                Ok(Ok(())) => info!("SDS server exited"),
                Ok(Err(err)) => error!(%err, "SDS server failed"),
                Err(err) => error!(%err, "SDS task panicked"),
            }
        }
        res = acme_task => {
            match res {
                Ok(Ok(())) => info!("ACME manager exited"),
                Ok(Err(err)) => error!(%err, "ACME manager failed"),
                Err(err) => error!(%err, "ACME task panicked"),
            }
        }
        _ = shutdown_signal() => {
            info!("shutdown signal received; exiting");
        }
    }

    Ok(())
}
