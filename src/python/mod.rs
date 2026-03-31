use pyo3::prelude::*;

use crate::limits::{board_dimension_is_valid, MAX_BOARD_DIM, MIN_BOARD_DIM};

#[macro_use]
mod dispatch;
mod py_board;
mod py_game;
mod py_game_outcome;
mod py_gtp;
mod py_move;
mod symmetry;

pub use py_board::PyBoard;
pub use py_game::PyGame;
pub use py_game_outcome::PyGameOutcome;
pub use py_gtp::PyGtpEngine;
pub use py_move::PyMove;
pub use symmetry::augment_symmetries;

pub(crate) fn validate_dimensions(width: usize, height: usize) -> PyResult<(u8, u8)> {
    let width = u8::try_from(width).map_err(|_| {
        PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
            "Board width must be between {} and {}",
            MIN_BOARD_DIM, MAX_BOARD_DIM
        ))
    })?;
    if !board_dimension_is_valid(width) {
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
            "Board width must be between {} and {}",
            MIN_BOARD_DIM, MAX_BOARD_DIM
        )));
    }

    let height = u8::try_from(height).map_err(|_| {
        PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
            "Board height must be between {} and {}",
            MIN_BOARD_DIM, MAX_BOARD_DIM
        ))
    })?;
    if !board_dimension_is_valid(height) {
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
            "Board height must be between {} and {}",
            MIN_BOARD_DIM, MAX_BOARD_DIM
        )));
    }

    Ok((width, height))
}
