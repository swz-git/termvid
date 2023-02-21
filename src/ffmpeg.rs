use std::{
    env::temp_dir,
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Stdio,
};

use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
};
use uuid::Uuid;
use which::which;

async fn get_framerate(path: &PathBuf) -> Result<f32, Box<dyn Error>> {
    let ffmpeg_path = which("ffprobe").ok().ok_or(
        "Couldn't find ffprobe in path (get it here: https://ffmpeg.org/download.html)".to_owned(),
    )?;

    let mut command = Command::new(ffmpeg_path);
    let args: Vec<&str> = "-v 0 -of csv=p=0 -select_streams v:0 -show_entries stream=r_frame_rate"
        .split(" ")
        .collect();

    command.args(args);
    command.arg(path.to_str().unwrap());

    // command.stdout(Stdio::null());

    let stdout = command
        .output()
        .await
        .ok()
        .ok_or("ffprobe failed".to_owned())?
        .stdout;
    let stdout_str = std::str::from_utf8(&stdout).unwrap();
    if stdout_str.is_empty() {
        return Err("ffprobe failed")?;
    }
    let split: Vec<&str> = stdout_str.trim_end().split("/").collect();
    let (numerator, denominator): (f32, f32) =
        (split[0].parse().unwrap(), split[1].parse().unwrap());
    Ok((numerator / denominator) as f32)
}

pub async fn split_video(path: PathBuf, scale: (u32, u32)) -> Result<PathBuf, Box<dyn Error>> {
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

    let fps = get_framerate(&path).await?;

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

    // while not finished:
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

        dbg!(clean_str);
    }

    let proc_result = proc.wait_with_output().await?;

    if !proc_result.status.success() {
        print!("{}", std::str::from_utf8(&proc_result.stderr)?);
        Err(format!("ffmpeg failed"))?
    }

    Ok(out_dir)
}
