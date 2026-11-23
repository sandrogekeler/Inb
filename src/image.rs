//! Turning a captured screen buffer into a `Frame`. Both systems hand back
//! BGRA in physical pixels; the GIF is made at the area's size in points.

/// A rectangle in physical pixels inside a captured buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// Crops `crop` out of a BGRA buffer with rows `stride` bytes apart, scales it
/// to `out_w` by `out_h` by averaging the source pixels under each output
/// pixel, and returns straight RGBA without row padding.
///
/// Panics if `crop` reaches outside the buffer: callers clamp it first.
pub fn crop_scale_bgra(src: &[u8], stride: usize, crop: PixelRect, out_w: u32, out_h: u32) -> Vec<u8> {
    assert!(out_w > 0 && out_h > 0 && crop.w > 0 && crop.h > 0);
    assert!((crop.y + crop.h - 1) as usize * stride + (crop.x + crop.w) as usize * 4 <= src.len());
    let mut out = Vec::with_capacity(out_w as usize * out_h as usize * 4);
    for oy in 0..out_h {
        let y0 = crop.y + oy * crop.h / out_h;
        let y1 = (crop.y + (oy + 1) * crop.h / out_h).max(y0 + 1);
        for ox in 0..out_w {
            let x0 = crop.x + ox * crop.w / out_w;
            let x1 = (crop.x + (ox + 1) * crop.w / out_w).max(x0 + 1);
            let mut sum = [0u32; 4];
            for y in y0..y1 {
                let row = y as usize * stride;
                for x in x0..x1 {
                    let p = row + x as usize * 4;
                    for (s, v) in sum.iter_mut().zip(&src[p..p + 4]) {
                        *s += u32::from(*v);
                    }
                }
            }
            let n = (y1 - y0) * (x1 - x0);
            let [b, g, r, a] = sum.map(|s| (s / n) as u8);
            out.extend_from_slice(&[r, g, b, a]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halves_a_retina_buffer_and_swaps_channels() {
        // 4x2 BGRA, stride 20 (4 bytes of padding per row).
        let mut src = vec![0u8; 40];
        for y in 0..2 {
            for x in 0..4 {
                let p = y * 20 + x * 4;
                src[p..p + 4].copy_from_slice(&[10 * x as u8, 0, 200, 255]);
            }
        }
        let out = crop_scale_bgra(&src, 20, PixelRect { x: 0, y: 0, w: 4, h: 2 }, 2, 1);
        assert_eq!(out, vec![200, 0, 5, 255, 200, 0, 25, 255]);
    }

    #[test]
    fn crops_before_scaling() {
        let mut src = vec![0u8; 16];
        src[12..16].copy_from_slice(&[1, 2, 3, 4]);
        let out = crop_scale_bgra(&src, 8, PixelRect { x: 1, y: 1, w: 1, h: 1 }, 1, 1);
        assert_eq!(out, vec![3, 2, 1, 4]);
    }
}
