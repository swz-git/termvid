use std::{error::Error, fmt::Display, path::PathBuf, process::Stdio};

use clap::Parser;
use console::Term;
use std::{io::BufReader, process::Command};
use which::which;
use yansi::Paint;

mod display;
use display::display;

const ASCII_BY_BRIGHTNESS: &str =
    r#"$@B%8&WM#*oahkbdpqwmZO0QLCJUYXzcvunxrjft/\|()1{}[]?-_+~<>i!lI;:,"^`'. "#;

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
pub enum PixelStyle {
    Char,
    Pixel,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
pub enum DisplayMode {
    Pad,
    Crop,
}

impl Display for DisplayMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match &self {
                DisplayMode::Crop => "crop",
                DisplayMode::Pad => "pad",
            },
        )
    }
}

/// CLI application to play a video in the terminal
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to video file
    // #[arg(short, long)]
    input: PathBuf,

    /// Enable audio playback
    #[arg(short, long, default_value_t = false)]
    audio: bool,

    /// Enable color
    #[arg(short, long, default_value_t = false)]
    color: bool,

    /// Pixel style
    #[arg(value_enum, short, long, default_value_t = PixelStyle::Char)]
    pixel_style: PixelStyle,

    /// Display mode
    #[arg(value_enum, short, long, default_value_t = DisplayMode::Pad)]
    display_mode: DisplayMode,
}

fn exit_sequence() {
    let term = Term::stdout();
    let (x, y) = (term.show_cursor(), term.clear_screen());
    x.unwrap();
    y.unwrap();
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    if args.audio {
        todo!("audio playback")
    }

    let path = args.input;

    let ffmpeg_path = which("ffmpeg").ok().ok_or_else(|| {
        "Couldn't find ffmpeg in path (get it here: https://ffmpeg.org/download.html)".to_owned()
    })?;

    ctrlc::set_handler(|| {
        exit_sequence();
    })
    .expect("Error setting Ctrl-C handler");

    let mut command = Command::new(ffmpeg_path);

    let term = Term::stdout();

    let term_size = term.size();

    let ffmpeg_res = (term_size.1, term_size.0);

    let command_args = [
        "-re",
        "-i",
        path.to_str().unwrap(),
        "-filter_complex",
        &format!(
            "scale=iw*2:ih,scale={}:{}:force_original_aspect_ratio={},{},format=yuv444p",
            ffmpeg_res.0,
            ffmpeg_res.1,
            match args.display_mode {
                DisplayMode::Crop => "increase",
                DisplayMode::Pad => "decrease",
            },
            match args.display_mode {
                DisplayMode::Pad =>
                    format!("pad={}:{}:-1:-1:color=black", ffmpeg_res.0, ffmpeg_res.1),
                DisplayMode::Crop => format!("crop={}:{}", ffmpeg_res.0, ffmpeg_res.1),
            },
        ),
        "-f",
        "yuv4mpegpipe",
        "-loglevel",
        "quiet",
        "-",
    ];

    // dbg!("ffmpeg ".to_owned() + &command_args.join(" "));

    command.args(command_args);

    command.stdout(Stdio::piped());
    command.stderr(Stdio::inherit());

    let mut proc = command.spawn()?;

    let proc_stdout = proc
        .stdout
        .take()
        .ok_or("Failed to read stdout of ffmpeg")?;
    let reader = BufReader::new(proc_stdout);

    let mut dec = y4m::decode(reader)?;

    // You need to enable ansi stuff on windows smh
    Paint::enable_windows_ascii();

    term.hide_cursor()?;

    let mut i = 0;
    loop {
        let frame = match dec.read_frame() {
            Ok(a) => a,
            _ => break,
        };
        if i == 0 {
            i += 1;
            continue;
        }
        display(
            frame,
            &ASCII_BY_BRIGHTNESS.chars().collect::<Vec<char>>(),
            args.color,
            args.pixel_style,
        )?;

        i += 1;
    }

    let proc_result = proc.wait_with_output()?;

    if !proc_result.status.success() {
        Err("ffmpeg failed")?
    };

    exit_sequence();
    Ok(())
}
