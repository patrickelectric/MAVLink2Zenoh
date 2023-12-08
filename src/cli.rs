use std::sync::Arc;

use clap::{self, Parser};

#[derive(Parser, Debug)]
#[command(name = "zenoh pub example")]
pub struct Args {
    /// MAVLink connection string
    #[arg(short, long)]
    pub connect: String,

    /// Zenoh configuration file
    #[arg(short = 'z', long)]
    pub config: Option<String>,

    /// Path to publish data
    #[arg(short, long, default_value = "mavlink")]
    pub path: String,
}

lazy_static! {
    pub static ref App: Arc<Args> = Arc::new(Args::parse());
}
