// Focused 5x7 bitmap atlas pattern selectively ported from PhysBox 4 revision
// 4295957e15480ef701e6006c7cafb5f0728ef101,
// crates/physbox-engine/src/options/font.rs. PhysBox 5 stores one R8 coverage
// byte per texel instead of the source RGBA atlas.

pub(crate) const GLYPH_WIDTH: u32 = 5;
pub(crate) const GLYPH_HEIGHT: u32 = 7;
pub(crate) const GLYPH_CELL_WIDTH: u32 = 6;
pub(crate) const GLYPH_CELL_HEIGHT: u32 = 8;
const GLYPH_ATLAS_FIRST: u8 = b' ';
const GLYPH_ATLAS_LAST: u8 = b']';
const GLYPH_ATLAS_COLUMNS: u32 = 16;

pub(crate) const fn glyph_atlas_width() -> u32 {
    GLYPH_ATLAS_COLUMNS * GLYPH_CELL_WIDTH
}

pub(crate) const fn glyph_atlas_height() -> u32 {
    let glyph_count = (GLYPH_ATLAS_LAST - GLYPH_ATLAS_FIRST + 1) as u32;
    glyph_count.div_ceil(GLYPH_ATLAS_COLUMNS) * GLYPH_CELL_HEIGHT
}

pub(crate) const fn glyph_atlas_bytes() -> u64 {
    glyph_atlas_width() as u64 * glyph_atlas_height() as u64
}

pub(crate) fn glyph_uv(byte: u8) -> [f32; 4] {
    let code = byte
        .to_ascii_uppercase()
        .clamp(GLYPH_ATLAS_FIRST, GLYPH_ATLAS_LAST);
    let index = u32::from(code - GLYPH_ATLAS_FIRST);
    let column = index % GLYPH_ATLAS_COLUMNS;
    let row = index / GLYPH_ATLAS_COLUMNS;
    let width = glyph_atlas_width() as f32;
    let height = glyph_atlas_height() as f32;
    let x = (column * GLYPH_CELL_WIDTH) as f32;
    let y = (row * GLYPH_CELL_HEIGHT) as f32;
    [
        x / width,
        y / height,
        (x + GLYPH_WIDTH as f32) / width,
        (y + GLYPH_HEIGHT as f32) / height,
    ]
}

pub(crate) fn glyph_atlas_r8() -> Vec<u8> {
    let width = glyph_atlas_width();
    let height = glyph_atlas_height();
    let mut coverage = vec![0; (width * height) as usize];
    for code in GLYPH_ATLAS_FIRST..=GLYPH_ATLAS_LAST {
        let index = u32::from(code - GLYPH_ATLAS_FIRST);
        let cell_x = (index % GLYPH_ATLAS_COLUMNS) * GLYPH_CELL_WIDTH;
        let cell_y = (index / GLYPH_ATLAS_COLUMNS) * GLYPH_CELL_HEIGHT;
        for (row, bits) in glyph_rows(code).iter().copied().enumerate() {
            for column in 0..GLYPH_WIDTH {
                if bits & (1 << (GLYPH_WIDTH - 1 - column)) != 0 {
                    coverage[((cell_y + row as u32) * width + cell_x + column) as usize] = 255;
                }
            }
        }
    }
    coverage
}

fn glyph_rows(code: u8) -> [u8; 7] {
    match code.to_ascii_uppercase() {
        b'#' => [10, 31, 10, 10, 31, 10, 0],
        b'%' => [17, 2, 4, 8, 16, 8, 17],
        b'-' => [0, 0, 0, 14, 0, 0, 0],
        b'.' => [0, 0, 0, 0, 0, 0, 4],
        b'/' => [1, 2, 4, 8, 16, 0, 0],
        b'0' => [14, 17, 19, 21, 25, 17, 14],
        b'1' => [4, 12, 4, 4, 4, 4, 14],
        b'2' => [14, 17, 1, 2, 4, 8, 31],
        b'3' => [30, 1, 1, 14, 1, 1, 30],
        b'4' => [2, 6, 10, 18, 31, 2, 2],
        b'5' => [31, 16, 16, 30, 1, 1, 30],
        b'6' => [14, 16, 16, 30, 17, 17, 14],
        b'7' => [31, 1, 2, 4, 8, 8, 8],
        b'8' => [14, 17, 17, 14, 17, 17, 14],
        b'9' => [14, 17, 17, 15, 1, 1, 14],
        b':' => [0, 4, 4, 0, 4, 4, 0],
        b'A' => [14, 17, 17, 31, 17, 17, 17],
        b'B' => [30, 17, 17, 30, 17, 17, 30],
        b'C' => [14, 17, 16, 16, 16, 17, 14],
        b'D' => [30, 17, 17, 17, 17, 17, 30],
        b'E' => [31, 16, 16, 30, 16, 16, 31],
        b'F' => [31, 16, 16, 30, 16, 16, 16],
        b'G' => [14, 17, 16, 23, 17, 17, 14],
        b'H' => [17, 17, 17, 31, 17, 17, 17],
        b'I' => [31, 4, 4, 4, 4, 4, 31],
        b'J' => [7, 2, 2, 2, 18, 18, 12],
        b'K' => [17, 18, 20, 24, 20, 18, 17],
        b'L' => [16, 16, 16, 16, 16, 16, 31],
        b'M' => [17, 27, 21, 21, 17, 17, 17],
        b'N' => [17, 25, 21, 19, 17, 17, 17],
        b'O' => [14, 17, 17, 17, 17, 17, 14],
        b'P' => [30, 17, 17, 30, 16, 16, 16],
        b'Q' => [14, 17, 17, 17, 21, 18, 13],
        b'R' => [30, 17, 17, 30, 20, 18, 17],
        b'S' => [15, 16, 16, 14, 1, 1, 30],
        b'T' => [31, 4, 4, 4, 4, 4, 4],
        b'U' => [17, 17, 17, 17, 17, 17, 14],
        b'V' => [17, 17, 17, 17, 17, 10, 4],
        b'W' => [17, 17, 17, 21, 21, 21, 10],
        b'X' => [17, 10, 4, 4, 4, 10, 17],
        b'Y' => [17, 10, 4, 4, 4, 4, 4],
        b'Z' => [31, 1, 2, 4, 8, 16, 31],
        b'[' => [14, 8, 8, 8, 8, 8, 14],
        b']' => [14, 2, 2, 2, 2, 2, 14],
        _ => [0; 7],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_r8_atlas_has_exact_declared_size_and_visible_glyphs() {
        let atlas = glyph_atlas_r8();
        assert_eq!(atlas.len() as u64, glyph_atlas_bytes());
        assert!(atlas.contains(&255));
        assert_eq!(glyph_atlas_bytes(), 3_072);
    }
}
