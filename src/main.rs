use std::{
    env::temp_dir,
    error::Error,
    fmt::Display,
    fs::OpenOptions,
    path::{Path, PathBuf},
    process::Stdio,
    sync::mpsc,
    thread,
};

use clap::Parser;
use console::Term;
use rodio::{OutputStream, Source};
use std::{io::BufReader, process::Command};
use uuid::Uuid;
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

    /// Loud ffmpeg
    #[arg(short, long, default_value_t = false)]
    loud_ffmpeg: bool,

    /// Overwrite default ffmpeg path
    #[arg(short, long)]
    ffmpeg_path: Option<PathBuf>,
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
        #[cfg(not(unix))]
        {
            Err("Audio playback is only supported on unix")?
        }
    }

    let path = args.input;

    if let Some(user_ffmpeg_path) = &args.ffmpeg_path {
        if !user_ffmpeg_path.exists() {
            Err("Invalid ffmpeg path")?
        }
    }

    let ffmpeg_path = args
        .ffmpeg_path
        .unwrap_or(which("ffmpeg").ok().ok_or_else(|| {
            "Couldn't find ffmpeg in path (get it here: https://ffmpeg.org/download.html or specify a custom binary with --ffmpeg-path flag)"
                .to_owned()
        })?);

    ctrlc::set_handler(|| {
        exit_sequence();
    })
    .expect("Error setting Ctrl-C handler");

    let mut command = Command::new(ffmpeg_path);

    let term = Term::stdout();

    let term_size = term.size();

    let ffmpeg_res = (term_size.1, term_size.0);

    let maybe_audio_pipe_path: Option<PathBuf> = if cfg!(unix) && args.audio {
        Some(Path::join(
            &temp_dir(),
            format!("termvid-audio-pipe-{}", Uuid::new_v4()),
        ))
    } else {
        None
    };

    if let Some(audio_pipe_path) = &maybe_audio_pipe_path {
        unix_named_pipe::create(&audio_pipe_path, Some(0o777))?
    };

    let command_args: String = format!(
        "-re -i {} {} -map 0:v -filter {} -vcodec wrapped_avframe -f yuv4mpegpipe {} -",
        path.to_str().unwrap(),
        if let Some(audio_pipe_path) = &maybe_audio_pipe_path {
            format!(
                "-map 0:a -async 1 -vsync 1 -acodec pcm_s16le -f wav {} -y",
                &audio_pipe_path
                    .to_str()
                    .ok_or("couldn't convert audio pipe path to string")?
            )
        } else {
            "".into()
        },
        format!(
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
        match args.loud_ffmpeg {
            false => "-loglevel quiet",
            true => "",
        },
    );

    let (audio_tx, audio_rx) = mpsc::channel::<()>();

    if let Some(audio_pipe_path) = maybe_audio_pipe_path {
        thread::spawn(move || {
            || -> Result<(), Box<dyn Error>> {
                let (_stream, stream_handle) = OutputStream::try_default().unwrap();

                let fifo = OpenOptions::new()
                    .read(true)
                    .write(false)
                    .open(audio_pipe_path)?;

                let stream = BufReader::new(fifo);

                let source = rodio::Decoder::new(stream).unwrap();

                stream_handle.play_raw(source.convert_samples())?;

                audio_rx.recv()?;
                Ok(())
            }()
            .expect("Audio streaming thread failed");
        });
    }

    let clean_command_args: Vec<&str> = command_args.split(" ").filter(|x| !x.is_empty()).collect();

    // dbg!(&command_args);

    command.args(clean_command_args);

    command.stdout(Stdio::piped());
    command.stderr(Stdio::inherit());
    command.stdin(Stdio::piped());

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

    audio_tx.send(())?;

    let proc_result = proc.wait_with_output()?;

    if !proc_result.status.success() {
        Err("ffmpeg failed")?
    };

    exit_sequence();
    Ok(())
}
