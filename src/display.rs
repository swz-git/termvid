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
    let y = y * 255.0;
    let u = u - 128.0;
    let v = v - 128.0;

    let r = (y + 1.13983 * v).round().max(0.0).min(255.0) as u8;
    let g = (y - 0.39465 * u - 0.58060 * v).round().max(0.0).min(255.0) as u8;
    let b = (y + 2.03211 * u).round().max(0.0).min(255.0) as u8;

    (r, g, b)
}

pub fn display(
    frame: Frame,
    display_chars: &[char],
    colored: bool,
    pixel_style: PixelStyle,
) -> Result<(), Box<dyn Error>> {
    let mut term = Term::stdout();
    term.clear_screen()?;
    let mut buf = String::new();

    let term_size = term.size();

    //
    let last_color = Color::Unset;

    let y_plane = frame.get_y_plane();
    let u_plane = frame.get_u_plane();
    let v_plane = frame.get_v_plane();

    for (i, y_value) in y_plane.iter().enumerate() {
        let rgb = yuv_to_rgb(
            *y_value as f64 / u8::MAX as f64,
            u_plane[i] as f64,
            v_plane[i] as f64,
        );
        // dbg!(rgb, (*y_value, u_plane[i], v_plane[i]));
        let mut new_color = match colored {
            false => Color::RGB(*y_value, *y_value, *y_value),
            true => Color::RGB(rgb.0, rgb.1, rgb.2),
        };
        if new_color == last_color {
            new_color = Color::Unset
        }

        match pixel_style {
            PixelStyle::Char => {
                Style::new(Color::Unset)
                    .bg(Color::Black)
                    .fg(new_color)
                    .fmt_prefix(&mut buf)?;
            }
            PixelStyle::Pixel => {
                Style::new(Color::Unset)
                    .fg(new_color)
                    .bg(new_color)
                    .fmt_prefix(&mut buf)?;
            }
        }

        let color = match pixel_style {
            PixelStyle::Char => get_item_from_brightness(y_value, display_chars),
            PixelStyle::Pixel => ' ',
        }
        .to_string();
        buf.write_str(&color)?;
    }

    std::io::Write::write(&mut term, &buf.bytes().collect::<Vec<u8>>())?;

    Ok(())
}
