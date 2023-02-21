use std::{
    env::temp_dir,
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Stdio,
};

use ffprobe::Stream;
use indicatif::ProgressBar;
use regex::Regex;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
};
use uuid::Uuid;
use which::which;

fn get_framerate(stream: &Stream) -> f32 {
    let split: Vec<&str> = stream.r_frame_rate.split("/").collect();
    let (numerator, denominator): (f32, f32) =
        (split[0].parse().unwrap(), split[1].parse().unwrap());
    (numerator / denominator) as f32
}

pub async fn split_video(
    path: PathBuf,
    scale: (u32, u32),
    progress_bar: bool,
) -> Result<PathBuf, Box<dyn Error>> {
    if !path.as_path().exists() {
        return Err(format!("File at `{}` does not exist", path.display()))?;
    }

    let out_dir = Path::join(&temp_dir(), format!("termvid-{}", Uuid::new_v4()));
    let out_files = Path::join(&out_dir, "img%08d.png");

    let ffmpeg_path = which("ffmpeg").ok().ok_or(
        "Couldn't find ffmpeg in path (get it here: https://ffmpeg.org/download.html)".to_owned(),
    )?;

    fs::create_dir(&out_dir)
        .ok()
        .ok_or("Couldn't create temporary directory")?;

    let ffprobe_info = ffprobe::ffprobe(&path)?;

    dbg!(&ffprobe_info.streams[0]);

    let fps: f32 = get_framerate(&ffprobe_info.streams[0]);
    let frames_count: u32 = ffprobe_info.streams[0]
        .nb_frames
        .as_ref()
        .ok_or("Couldn't read frame count of video")?
        .parse()?;

    dbg!(fps);

    let filter = format!("fps={},scale={}:{}", fps.round(), scale.0, scale.1);

    let mut command = Command::new(ffmpeg_path);

    command.args([
        "-i",
        path.to_str().unwrap(),
        "-vf",
        &filter,
        out_files.to_str().unwrap(),
    ]);

    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    dbg!(&command);

    let mut proc = command.spawn()?;

    dbg!(proc.id());

    let stderr = proc
        .stderr
        .take()
        .ok_or("Failed to read stdout of ffmpeg")?;
    let mut reader = BufReader::new(stderr);

    let maybe_bar: Option<ProgressBar> = match progress_bar {
        true => Some(ProgressBar::new(frames_count as u64).with_message("Converting video")),
        false => None,
    };

    // while not finished
    while proc.try_wait()?.is_none() {
        let mut buf: Vec<u8> = vec![];
        let _num_bytes = reader.read_until(b'\r', &mut buf).await?;
        let clean_str = std::str::from_utf8(&buf)?
            .split("\n")
            .last()
            .unwrap()
            .trim();

        if clean_str.is_empty() {
            continue;
        }

        let frames_re = Regex::new(r"^frame= *(\d+)").unwrap();
        let current_frame: u32 = frames_re
            .captures_iter(clean_str)
            .next()
            .ok_or("Couldn't read current frame count")?[1]
            .parse()?;

        match maybe_bar {
            None => &(),
            Some(ref bar) => &bar.set_position(current_frame as u64),
        };
    }

    let proc_result = proc.wait_with_output().await?;

    if !proc_result.status.success() {
        print!("{}", std::str::from_utf8(&proc_result.stderr)?);
        Err(format!("ffmpeg failed"))?
    }

    match maybe_bar {
        None => &(),
        Some(ref bar) => &bar.finish(),
    };

    Ok(out_dir)
}
