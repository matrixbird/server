use matrixbird::*; 
use config::ConfigBuilder;
use server::Server;

use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tracing_appender::non_blocking::WorkerGuard;

use crate::AppState;


#[tokio::main]
async fn main() {

    let _logging_guard = setup_tracing();

    let args = Args::build();

    //let config = Config::new(&args.config)
        //.validate();

    let config = match ConfigBuilder::from_file(&args.config) {
        Ok(builder) => match builder.build() {
            Ok(config) => config,
            Err(e) => {
                eprintln!("Error building configuration: {}", e);
                std::process::exit(1);
            }
        },
        Err(e) => {
            eprintln!("Error loading configuration: {}", e);
            std::process::exit(1);
        }
    };

    let state = AppState::new(config.clone())
        .await
        .unwrap_or_else(|e| {
            eprintln!("Failed to initialize state: {}", e);
            std::process::exit(1);
        });

    match args.command {
        Some(Command::SendEmails { dry_run }) => {
            println!("Sending test emails... {}", dry_run);
        }
        Some(Command::Migrate) => {
            println!("Running database migrations...");
        }
        None => {
            info!("Starting Matrixbird server...");

            Server::new(state)
            .run()
            .await 
            .unwrap_or_else(|e| {
                eprintln!("Server error: {}", e);
                std::process::exit(1);
            }); 
        }
    }


}

pub fn setup_tracing() -> WorkerGuard {
    let env_filter = if cfg!(debug_assertions) {
        "debug,hyper_util=off,tower_http=off,ruma=off,reqwest=off,aws_runtime=off,aws_sdk_s3=off,aws_smithy_runtime=off,aws_smithy_runtime_api=off"
    } else {
        "info"
    };

    let file_appender = tracing_appender::rolling::daily("./logs", "matrixbird.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let console_layer = tracing_subscriber::fmt::layer().pretty();
    
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false); 
    
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(env_filter))
        .with(console_layer)
        .with(file_layer)
        .init();
    
    tracing::info!("Tracing initialized with file logging");
    
    guard
}
