use pyo3::prelude::*;

use super::dispatch::*;
use super::py_board::PyBoard;
use super::py_game_outcome::PyGameOutcome;
use super::py_move::PyMove;
use super::validate_dimensions;
use crate::encode;
use crate::player::Player;
use crate::position::Position;

#[pyclass(name = "Game")]
pub struct PyGame {
    inner: GameInner,
}

#[hotpath::measure_all]
#[pymethods]
impl PyGame {
    #[new]
    pub fn new(width: usize, height: usize) -> PyResult<Self> {
        let (width, height) = validate_dimensions(width, height)?;
        Ok(PyGame {
            inner: make_game_inner(width, height),
        })
    }

    #[staticmethod]
    #[pyo3(signature = (width, height, komi, min_moves_before_pass_possible, max_moves, superko))]
    pub fn with_options(
        width: usize,
        height: usize,
        komi: f32,
        min_moves_before_pass_possible: usize,
        max_moves: usize,
        superko: bool,
    ) -> PyResult<Self> {
        let (width, height) = validate_dimensions(width, height)?;
        Ok(PyGame {
            inner: make_game_inner_with_options(
                width,
                height,
                komi,
                min_moves_before_pass_possible as u16,
                max_moves as u16,
                superko,
            ),
        })
    }

    #[staticmethod]
    pub fn standard() -> Self {
        PyGame {
            inner: make_game_inner(19, 19),
        }
    }

    pub fn komi(&self) -> f32 {
        dispatch_game!(&self.inner, g => g.komi())
    }

    pub fn min_moves_before_pass_possible(&self) -> usize {
        dispatch_game!(&self.inner, g => g.min_moves_before_pass_possible() as usize)
    }

    pub fn max_moves(&self) -> usize {
        dispatch_game!(&self.inner, g => g.max_moves() as usize)
    }

    pub fn move_count(&self) -> usize {
        dispatch_game!(&self.inner, g => g.move_count())
    }

    pub fn score(&self) -> (f32, f32) {
        dispatch_game!(&self.inner, g => g.score())
    }

    pub fn width(&self) -> usize {
        dispatch_game!(&self.inner, g => g.width() as usize)
    }

    pub fn height(&self) -> usize {
        dispatch_game!(&self.inner, g => g.height() as usize)
    }

    pub fn get_piece(&self, col: usize, row: usize) -> Option<i8> {
        let pos = Position::new(col as u8, row as u8);
        dispatch_game!(&self.inner, g => g.get_piece(&pos).map(|p| p as i8))
    }

    pub fn set_piece(&mut self, col: usize, row: usize, piece: Option<i8>) {
        let pos = Position::new(col as u8, row as u8);
        let player = piece.map(|p| Player::from_int(p).expect("Invalid player value"));
        dispatch_game_mut!(&mut self.inner, g => g.set_piece(&pos, player))
    }

    pub fn turn(&self) -> i8 {
        dispatch_game!(&self.inner, g => g.turn() as i8)
    }

    pub fn is_over(&self) -> bool {
        dispatch_game!(&self.inner, g => g.is_over())
    }

    // ---------------------------------------------------------------------
    // Unified Game Protocol Methods
    // ---------------------------------------------------------------------

    pub fn legal_action_indices(&self) -> Vec<usize> {
        dispatch_game!(&self.inner, g => {
            let w = g.width();
            let h = g.height();
            g.legal_moves()
                .into_iter()
                .map(|m| encode::encode_move(&m, w, h))
                .collect()
        })
    }

    pub fn apply_action(&mut self, action: usize) -> bool {
        dispatch_game_mut!(&mut self.inner, g => {
            let w = g.width();
            let h = g.height();
            if let Some(move_) = encode::decode_move(action, w, h) {
                g.make_move(&move_)
            } else {
                false
            }
        })
    }

    // ---------------------------------------------------------------------
    // Encoding/decoding
    // ---------------------------------------------------------------------

    pub fn encode_game_planes(&mut self) -> (Vec<f32>, usize, usize, usize) {
        dispatch_game_mut!(&mut self.inner, g => encode::encode_game_planes(g))
    }

    pub fn decode_action(&self, action: usize) -> Option<PyMove> {
        dispatch_game!(&self.inner, g => {
            let w = g.width();
            let h = g.height();
            encode::decode_move(action, w, h).map(|move_| PyMove::from_move(move_))
        })
    }

    pub fn total_actions(&self) -> usize {
        dispatch_game!(&self.inner, g => encode::total_actions(g.width(), g.height()))
    }

    pub fn board_shape(&self) -> (usize, usize) {
        dispatch_game!(&self.inner, g => (g.height() as usize, g.width() as usize))
    }

    pub fn input_plane_count(&self) -> usize {
        encode::TOTAL_INPUT_PLANES
    }

    pub fn reward_absolute(&self) -> f32 {
        dispatch_game!(&self.inner, g => {
            g.outcome()
                .map(|o| o.encode_winner_absolute())
                .unwrap_or(0.0)
        })
    }

    pub fn reward_from_perspective(&self, perspective: i8) -> f32 {
        dispatch_game!(&self.inner, g => {
            g.outcome()
                .map(|o| {
                    o.encode_winner_from_perspective(
                        Player::from_int(perspective).expect("Invalid perspective"),
                    )
                })
                .unwrap_or(0.0)
        })
    }

    pub fn outcome(&self) -> Option<PyGameOutcome> {
        dispatch_game!(&self.inner, g => g.outcome().map(|o| PyGameOutcome::from_outcome(o)))
    }

    pub fn legal_moves(&self) -> Vec<PyMove> {
        dispatch_game!(&self.inner, g => {
            g.legal_moves()
                .into_iter()
                .map(|m| PyMove::from_move(m))
                .collect()
        })
    }

    pub fn is_legal_move(&self, move_: &PyMove) -> bool {
        dispatch_game!(&self.inner, g => g.is_legal_move(move_.as_inner()))
    }

    pub fn make_move(&mut self, move_: &PyMove) -> bool {
        dispatch_game_mut!(&mut self.inner, g => g.make_move(move_.as_inner()))
    }

    pub fn unmake_move(&mut self) -> bool {
        dispatch_game_mut!(&mut self.inner, g => g.unmake_move())
    }

    pub fn board(&self) -> PyBoard {
        PyBoard::from_inner(game_to_board_inner!(&self.inner))
    }

    pub fn superko(&self) -> bool {
        dispatch_game!(&self.inner, g => g.superko())
    }

    pub fn ko_point(&self) -> Option<(usize, usize)> {
        dispatch_game!(&self.inner, g => {
            g.ko_point().map(|p| (p.col as usize, p.row as usize))
        })
    }

    pub fn clone(&self) -> PyGame {
        PyGame {
            inner: self.inner.clone(),
        }
    }

    pub fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        dispatch_game!(&self.inner, g => {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            g.board().hash(&mut hasher);
            (g.turn() as i8).hash(&mut hasher);
            g.ko_point().hash(&mut hasher);
            hasher.finish()
        })
    }

    pub fn __str__(&self) -> String {
        dispatch_game!(&self.inner, g => g.to_string())
    }

    pub fn __repr__(&self) -> String {
        dispatch_game!(&self.inner, g => {
            format!(
                "Game(width={}, height={}, turn={:?}, over={}, superko={})",
                g.width(),
                g.height(),
                g.turn(),
                g.is_over(),
                g.superko()
            )
        })
    }
}
