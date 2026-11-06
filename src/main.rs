use clap::Parser;
use log::LevelFilter;

use crate::cli::Cli;

mod analyzer;
mod cli;
mod extractor;
mod pdf;
mod utils;

fn main() {
    let cli = Cli::parse();

    env_logger::builder()
        .filter_level(if cli.debug {
            LevelFilter::Debug
        } else {
            LevelFilter::Info
        })
        .format_file(false)
        .format_module_path(false)
        .format_timestamp(None)
        .filter_module("reqwest", LevelFilter::Warn)
        .filter_module("ureq", LevelFilter::Warn)
        .filter_module("ureq_proto", LevelFilter::Warn)
        .filter_module("rustls", LevelFilter::Warn)
        .init();

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to build tokio runtime")
        .block_on(async {
            cli.run().await;
        });
}
