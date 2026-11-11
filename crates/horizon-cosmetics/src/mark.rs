//! Oteria's mark as the game draws it before an Oteria player's name (and the launcher before
//! yours): its logo's wave in white lines, curling over a fainter foam line, 8×8.
//!
//! ```text
//! ..####..
//! .#....#.
//! #..##...
//! #.#..#..
//! #.#.....
//! .#.###..
//! ..#####+
//! +++++...    (+: the fainter white)
//! ```

const WHITE: u32 = 0xFFFF_FFFF;
const FOAM: u32 = 0x66FF_FFFF;

/// The mark's rectangles in its 8×8 square: `(x, y, width, height, ARGB)`.
pub const MARK: [(u8, u8, u8, u8, u32); 12] = [
    (2, 0, 4, 1, WHITE),
    (1, 1, 1, 1, WHITE),
    (6, 1, 1, 1, WHITE),
    (0, 2, 1, 3, WHITE),
    (3, 2, 2, 1, WHITE),
    (2, 3, 1, 2, WHITE),
    (5, 3, 1, 1, WHITE),
    (1, 5, 1, 1, WHITE),
    (3, 5, 3, 1, WHITE),
    (2, 6, 5, 1, WHITE),
    (7, 6, 1, 1, FOAM),
    (0, 7, 5, 1, FOAM),
];

/// The mark as 8×8 RGBA texels, rows from the top.
pub fn texels() -> [[u8; 4]; 64] {
    let mut out = [[0; 4]; 64];
    for (x, y, w, h, argb) in MARK {
        let [a, r, g, b] = argb.to_be_bytes();
        for yy in y..y + h {
            for xx in x..x + w {
                out[usize::from(yy) * 8 + usize::from(xx)] = [r, g, b, a];
            }
        }
    }
    out
}
