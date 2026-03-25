use std::fmt;
use std::hash::{Hash, Hasher};

use crate::bitboard::{nw_for_board, Bitboard};
use crate::limits::assert_supported_board_dimensions;
use crate::player::Player;
use crate::position::Position;

pub const STANDARD_COLS: u8 = 19;
pub const STANDARD_ROWS: u8 = 19;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Board<const NW: usize> {
    black: Bitboard<NW>,
    white: Bitboard<NW>,
    width: u8,
    height: u8,
}

#[hotpath::measure_all]
impl<const NW: usize> Hash for Board<NW> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.black.hash(state);
        self.white.hash(state);
        self.width.hash(state);
        self.height.hash(state);
    }
}

#[hotpath::measure_all]
impl<const NW: usize> Board<NW> {
    /// Create a board with dimensions in `4..=32`.
    /// `NW` must match `ceil(width * height / 64)`.
    pub fn new(width: u8, height: u8) -> Self {
        assert_supported_board_dimensions(width, height);
        let required_words = nw_for_board(width, height);
        assert!(
            NW == required_words,
            "NW={} does not match board {}x{} (need {})",
            NW,
            width,
            height,
            required_words
        );
        Board {
            black: Bitboard::empty(),
            white: Bitboard::empty(),
            width,
            height,
        }
    }

    pub fn width(&self) -> u8 {
        self.width
    }

    pub fn height(&self) -> u8 {
        self.height
    }

    pub fn get_piece(&self, pos: &Position) -> Option<Player> {
        if pos.is_valid(self.width, self.height) {
            let idx = pos.to_index(self.width);
            if self.black.get(idx) {
                Some(Player::Black)
            } else if self.white.get(idx) {
                Some(Player::White)
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn set_piece(&mut self, pos: &Position, player: Option<Player>) {
        if pos.is_valid(self.width, self.height) {
            let idx = pos.to_index(self.width);
            self.black.clear(idx);
            self.white.clear(idx);
            match player {
                Some(Player::Black) => self.black.set(idx),
                Some(Player::White) => self.white.set(idx),
                None => {}
            }
        }
    }

    pub fn clear(&mut self) {
        self.black = Bitboard::empty();
        self.white = Bitboard::empty();
    }

    #[inline]
    pub(crate) fn black_stones(&self) -> Bitboard<NW> {
        self.black
    }

    #[inline]
    pub(crate) fn white_stones(&self) -> Bitboard<NW> {
        self.white
    }

    #[inline]
    pub(crate) fn occupied(&self) -> Bitboard<NW> {
        self.black | self.white
    }

    #[inline]
    pub(crate) fn empty_squares(&self, board_mask: Bitboard<NW>) -> Bitboard<NW> {
        board_mask & !(self.black | self.white)
    }

    /// Remove all stones indicated by `bb` from the board.
    #[inline]
    pub(crate) fn remove_stones(&mut self, bb: Bitboard<NW>) {
        self.black &= !bb;
        self.white &= !bb;
    }

    /// Restore stones from a captured bitboard for the given player.
    #[inline]
    pub(crate) fn restore_stones(&mut self, bb: Bitboard<NW>, player: Player) {
        match player {
            Player::Black => self.black |= bb,
            Player::White => self.white |= bb,
        }
    }

    /// Get stones bitboard for a specific player.
    #[inline]
    pub(crate) fn stones_for(&self, player: Player) -> Bitboard<NW> {
        match player {
            Player::Black => self.black,
            Player::White => self.white,
        }
    }

    /// Set a single bit for a player (no clearing — caller must ensure position is empty).
    #[inline]
    pub(crate) fn set_bit(&mut self, idx: usize, player: Player) {
        match player {
            Player::Black => self.black.set(idx),
            Player::White => self.white.set(idx),
        }
    }

    /// Clear a single bit from both bitboards.
    #[inline]
    pub(crate) fn clear_bit(&mut self, idx: usize) {
        self.black.clear(idx);
        self.white.clear(idx);
    }
}

#[hotpath::measure_all]
impl Default for Board<{ nw_for_board(STANDARD_COLS, STANDARD_ROWS) }> {
    fn default() -> Self {
        Self::new(STANDARD_COLS, STANDARD_ROWS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_board_sizes() {
        let size_9x9 = std::mem::size_of::<Board<{ nw_for_board(9, 9) }>>();
        let size_19x19 = std::mem::size_of::<Board<{ nw_for_board(19, 19) }>>();
        let size_32x32 = std::mem::size_of::<Board<{ nw_for_board(32, 32) }>>();

        // 9x9 should be much smaller than 32x32
        assert!(size_9x9 < size_19x19);
        assert!(size_19x19 < size_32x32);

        // 9x9 (NW=2): ~40 bytes (2*16 + padding) vs old 258
        assert!(size_9x9 <= 40, "9x9 Board too large: {}", size_9x9);
        // 19x19 (NW=6): ~104 bytes vs old 258
        assert!(size_19x19 <= 104, "19x19 Board too large: {}", size_19x19);
    }
}

#[hotpath::measure_all]
impl<const NW: usize> fmt::Display for Board<NW> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for row in (0..self.height as usize).rev() {
            write!(f, "|")?;

            for col in 0..self.width as usize {
                let pos = Position::new(col as u8, row as u8);
                let c = if let Some(player) = self.get_piece(&pos) {
                    player.to_char()
                } else {
                    '.'
                };

                write!(f, "{}", c)?;
                write!(f, "|")?;
            }

            writeln!(f)?;
        }

        // Column numbers
        write!(f, " ")?;
        for col in 0..self.width as usize {
            write!(f, "{} ", col)?;
        }
        writeln!(f)?;

        Ok(())
    }
}
