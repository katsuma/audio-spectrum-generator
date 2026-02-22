//! Spectrum drawing with rounded bars (image)

use image::{ImageBuffer, Rgba};

/// Draw one frame: white background, black bars.
/// `bar_heights`: height per bar (0.0–1.0, assumed normalized).
/// Spectrum is drawn in the bottom `spectrum_height` pixels with bars vertically centered.
pub fn draw_spectrum_frame(
    width: u32,
    height: u32,
    spectrum_height: u32,
    bar_heights: &[f32],
    bar_color: [u8; 4],
    bg_color: [u8; 4],
) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let mut img = ImageBuffer::from_fn(width, height, |_, _| Rgba(bg_color));

    if bar_heights.is_empty() {
        return img;
    }

    let usable_height = spectrum_height.saturating_sub(4);
    let y_center = height.saturating_sub(spectrum_height / 2);

    let total_bars = bar_heights.len() as u32;
    let gap = 1u32;
    let total_gaps = total_bars.saturating_sub(1) * gap;
    let bar_width = if total_bars > 0 {
        (width.saturating_sub(total_gaps)) / total_bars
    } else {
        0
    };
    let radius = (bar_width / 2).min(4).max(1);
    let start_x = (width.saturating_sub(total_bars * bar_width + total_gaps)) / 2;

    for (i, &h) in bar_heights.iter().enumerate() {
        let bar_height_f = h.clamp(0.0, 1.0) * usable_height as f32;
        let bar_height = bar_height_f as u32;
        if bar_height == 0 {
            continue;
        }

        let x0 = start_x + i as u32 * (bar_width + gap);
        let y_top = y_center.saturating_sub(bar_height / 2);

        draw_rounded_rect(
            &mut img,
            x0,
            y_top,
            bar_width,
            bar_height,
            radius,
            bar_color,
        );
    }

    img
}

/// Draw a rounded rectangle (all four corners rounded).
fn draw_rounded_rect(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    x0: u32,
    y0: u32,
    w: u32,
    h: u32,
    r: u32,
    color: [u8; 4],
) {
    let (width, height) = img.dimensions();
    let r = r.min(w / 2).min(h / 2);
    let x1 = x0 + w;
    let y1 = y0 + h;

    for y in y0..y1 {
        for x in x0..x1 {
            if !point_in_rounded_rect(x, y, x0, y0, w, h, r) {
                continue;
            }
            if x < width && y < height {
                img.put_pixel(x, y, Rgba(color));
            }
        }
    }
}

fn point_in_rounded_rect(px: u32, py: u32, x0: u32, y0: u32, w: u32, h: u32, r: u32) -> bool {
    if r == 0 {
        return px >= x0 && px < x0 + w && py >= y0 && py < y0 + h;
    }
    let x1 = x0 + w;
    let y1 = y0 + h;
    let px = px as i64;
    let py = py as i64;
    let x0 = x0 as i64;
    let y0 = y0 as i64;
    let x1 = x1 as i64;
    let y1 = y1 as i64;
    let r = r as i64;

    let in_center = px >= x0 + r && px < x1 - r && py >= y0 && py < y1;
    let in_middle_vertical = px >= x0 && px < x1 && py >= y0 + r && py < y1 - r;
    if in_center || in_middle_vertical {
        return true;
    }
    let corners = [
        (x0 + r, y0 + r, r),
        (x1 - r - 1, y0 + r, r),
        (x0 + r, y1 - r - 1, r),
        (x1 - r - 1, y1 - r - 1, r),
    ];
    for (cx, cy, cr) in corners {
        if (px - cx).pow(2) + (py - cy).pow(2) <= cr * cr {
            return true;
        }
    }
    false
}
