use std::{error::Error, fmt::Write};

use console::Term;
use y4m::Frame;
use yansi::{Color, Style};

use crate::PixelStyle;

fn get_item_from_brightness<T>(brightness: &u8, display_chars: &[T]) -> T
where
    T: Copy,
{
    let len = display_chars.len();
    display_chars[(*brightness as f32 / u8::MAX as f32 * (len - 1) as f32).floor() as usize]
}

// thank chat gpt for this
fn yuv_to_rgb(y: f64, u: f64, v: f64) -> (u8, u8, u8) {
    // let y = y * 255.0;
    let u = u - 128.0;
    let v = v - 128.0;

    let r = (y + 1.13983 * v).round().clamp(0.0, 255.0) as u8;
    let g = (y - 0.39465 * u - 0.58060 * v).round().clamp(0.0, 255.0) as u8;
    let b = (y + 2.03211 * u).round().clamp(0.0, 255.0) as u8;

    (r, g, b)
}

// https://gitlab.com/gnachman/iterm2/-/wikis/synchronized-updates-spec
const DECSET: [u8; 8] = [27, 91, 63, 50, 48, 50, 54, 104];
const DECRESET: [u8; 8] = [27, 91, 63, 50, 48, 50, 54, 108];

const CURSOR_TOP_LEFT: [u8; 6] = [27, 91, 48, 59, 48, 72];

pub fn display(
    frame: Frame,
    display_chars: &[char],
    colored: bool,
    pixel_style: PixelStyle,
    cols: u16,
) -> Result<(), Box<dyn Error>> {
    let mut term = Term::stdout();
    let mut buf = String::new();

    let y_plane = frame.get_y_plane();
    let u_plane = frame.get_u_plane();
    let v_plane = frame.get_v_plane();

    for (i, y_value) in y_plane.iter().enumerate() {
        let row = i / (cols as usize);
        if row % 2 != 1 && pixel_style == PixelStyle::DoublePixel {
            continue; // when pixel_style is DoublePixel, ffmpeg outputs double the rows
        }

        let color_above = match pixel_style {
            PixelStyle::DoublePixel => Some({
                let index_of_above = (row - 1) * cols as usize + (i % cols as usize);
                let y_value = &y_plane[index_of_above];
                match colored {
                    false => Color::RGB(*y_value, *y_value, *y_value),
                    true => {
                        let rgb = yuv_to_rgb(
                            *y_value as f64,
                            u_plane[index_of_above] as f64,
                            v_plane[index_of_above] as f64,
                        );
                        Color::RGB(rgb.0, rgb.1, rgb.2)
                    }
                }
            }),
            _ => None,
        };

        let color = match colored {
            false => Color::RGB(*y_value, *y_value, *y_value),
            true => {
                let rgb = yuv_to_rgb(*y_value as f64, u_plane[i] as f64, v_plane[i] as f64);
                Color::RGB(rgb.0, rgb.1, rgb.2)
            }
        };

        match pixel_style {
            PixelStyle::Char => {
                Style::new(Color::Unset)
                    .bg(Color::Black)
                    .fg(color)
                    .fmt_prefix(&mut buf)?;
            }
            PixelStyle::Pixel => {
                Style::new(Color::Unset)
                    .fg(color)
                    .bg(color)
                    .fmt_prefix(&mut buf)?;
            }
            PixelStyle::DoublePixel => {
                Style::new(Color::Unset)
                    .fg(color)
                    .bg(color_above.unwrap_or(color))
                    .fmt_prefix(&mut buf)?;
            }
        }

        let color = match pixel_style {
            PixelStyle::Char => get_item_from_brightness(y_value, display_chars),
            PixelStyle::Pixel => ' ',
            PixelStyle::DoublePixel => '▄',
        }
        .to_string();
        buf.write_str(&color)?;
    }

    std::io::Write::write(&mut term, &DECSET)?;
    std::io::Write::write(&mut term, &CURSOR_TOP_LEFT)?;
    std::io::Write::write(&mut term, &buf.bytes().collect::<Vec<u8>>())?;
    std::io::Write::write(&mut term, &DECRESET)?;

    Ok(())
}
