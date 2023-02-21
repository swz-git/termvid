use std::path::PathBuf;

use clap::Parser;
mod ffmpeg;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to video file
    #[arg(short, long)]
    input: PathBuf,

    /// Framerate to playback at
    #[arg(short, long, default_value_t = 1)]
    framerate: u8,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let test = ffmpeg::split_video(args.input, (1920, 1080), true).await;

    dbg!(test.expect("shit"));
}
