use axum::{http::StatusCode, response::IntoResponse, routing::get, Router};
use backoff::{future::retry, ExponentialBackoffBuilder};
use clap::Parser;
use dsmr5_exporter::{decoder, metrics::METRICS_TTL, Metrics};
use futures::StreamExt;
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
use tap::Tap;
use tokio::{
    signal::unix::{signal, SignalKind},
    sync::{broadcast, RwLock},
};
use tokio_serial::SerialPortBuilderExt;
use tokio_util::codec::Decoder;

#[derive(Parser, Debug)]
#[clap(name = "dsmr5-exporter", version, author)]
struct Cli {
    serial_device_path: PathBuf,

    #[arg(long, default_value_t = Ipv4Addr::new(127, 0, 0, 1))]
    host: Ipv4Addr,

    #[arg(long, default_value_t = 3000)]
    port: u16,
}

#[tokio::main]
pub async fn main() {
    let cli = Cli::parse();

    let mut sigterm = signal(SignalKind::terminate()).unwrap();
    let (notify_shutdown, _) = broadcast::channel::<()>(1);
    let metrics = Arc::new(RwLock::new(Metrics::new()));

    let server_task = tokio::spawn(serve(
        SocketAddr::new(IpAddr::V4(cli.host), cli.port),
        Arc::clone(&metrics),
        notify_shutdown.subscribe(),
    ));

    tokio::select! {
        _ = read(cli.serial_device_path.to_str().unwrap(), Arc::clone(&metrics)) => {},
        _ = sigterm.recv() => {
            println!("received sigterm, stopping");
        },
        _ = tokio::signal::ctrl_c() => {
            println!("received ctrl-c, stopping");
        }
    }

    drop(notify_shutdown);
    let _ = server_task.await;
}

async fn read(
    serial_device: &str,
    metrics: Arc<RwLock<Metrics>>,
) -> Result<(), tokio_serial::Error> {
    let backoff = ExponentialBackoffBuilder::default()
        .with_max_interval(Duration::from_millis(5000))
        .with_max_elapsed_time(None)
        .build();

    retry::<(), _, _, _, _>(backoff, || async {
        println!("opening serial port");
        let port = tokio_serial::new(serial_device, 115200)
            .open_native_async()
            .tap(|result| {
                if let Err(error) = result {
                    println!("failed to open serial port: {:?}", error);
                }
            })?;

        let mut reader = decoder::Dsmr5Codec::new().framed(port);
        println!("port open");

        while let Some(frame) = reader.next().await {
            match frame {
                Ok(frame) => {
                    println!("frame received: {:?}", frame);
                    let mut metrics = metrics.write().await;
                    metrics.update(&frame);
                }
                Err(error) => println!("error reading frame: {:?}", error),
            }
        }

        Err(backoff::Error::transient(tokio_serial::Error::new(
            tokio_serial::ErrorKind::Io(std::io::ErrorKind::ConnectionReset),
            "serial read stream ended",
        )))
    })
    .await
}

async fn serve(
    addr: SocketAddr,
    metrics: Arc<RwLock<Metrics>>,
    mut notify_shutdown: broadcast::Receiver<()>,
) {
    let app = Router::new().route(
        "/metrics",
        get({
            let metrics = Arc::clone(&metrics);
            move || handler(metrics)
        }),
    );

    let app = app.fallback(handler_404);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(async move {
            let _ = notify_shutdown.recv().await;
            println!("stopping server");
        })
        .await
        .unwrap();
    println!("server stopped");
}

async fn handler(metrics: Arc<RwLock<Metrics>>) -> Result<String, StatusCode> {
    let metrics = metrics.read().await;

    if metrics.last_update.elapsed() > METRICS_TTL {
        return Ok(String::new());
    }

    metrics.encode().map_err(|error| {
        println!("Error while encoding metrics: {:?}", error);
        StatusCode::INTERNAL_SERVER_ERROR
    })
}

async fn handler_404() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "not found")
}
