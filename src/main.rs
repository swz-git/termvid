use std::{error::Error, path::PathBuf, process::Stdio};

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

/// CLI application to play a video in the terminal
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to video file
    #[arg(short, long)]
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

    let ffmpeg_path = which("ffmpeg").ok().ok_or(
        "Couldn't find ffmpeg in path (get it here: https://ffmpeg.org/download.html)".to_owned(),
    )?;

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
            "scale=iw*2:ih,scale=-1:{},pad=width={}:height={}:x=({}-iw)/2:y=({}-ih)/2:color=black,format=yuv444p",
            ffmpeg_res.1, ffmpeg_res.0, ffmpeg_res.1,ffmpeg_res.0, ffmpeg_res.1
        ),
        "-f",
        "yuv4mpegpipe",
        "-loglevel",
        "quiet",
        "-",
    ];

    dbg!("ffmpeg ".to_owned() + &command_args.join(" "));

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
        Err(format!("ffmpeg failed"))?
    };

    exit_sequence();
    Ok(())
}
