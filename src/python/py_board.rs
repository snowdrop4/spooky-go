use pyo3::prelude::*;

use super::dispatch::*;
use super::validate_dimensions;
use crate::player::Player;
use crate::position::Position;

#[pyclass(name = "Board")]
#[derive(Clone)]
pub struct PyBoard {
    pub(super) inner: BoardInner,
}

#[hotpath::measure_all]
impl PyBoard {
    pub(super) fn from_inner(inner: BoardInner) -> Self {
        PyBoard { inner }
    }
}

#[hotpath::measure_all]
#[pymethods]
impl PyBoard {
    #[new]
    pub fn new(width: usize, height: usize) -> PyResult<Self> {
        let (width, height) = validate_dimensions(width, height)?;
        Ok(PyBoard {
            inner: make_board_inner(width, height),
        })
    }

    #[staticmethod]
    pub fn standard() -> Self {
        PyBoard {
            inner: make_board_inner(19, 19),
        }
    }

    pub fn width(&self) -> usize {
        dispatch_board!(&self.inner, b => b.width() as usize)
    }

    pub fn height(&self) -> usize {
        dispatch_board!(&self.inner, b => b.height() as usize)
    }

    pub fn get_piece(&self, col: usize, row: usize) -> Option<i8> {
        let pos = Position::new(col as u8, row as u8);
        dispatch_board!(&self.inner, b => b.get_piece(&pos).map(|p| p as i8))
    }

    pub fn set_piece(&mut self, col: usize, row: usize, piece: Option<i8>) {
        let pos = Position::new(col as u8, row as u8);
        let player = piece.map(|p| Player::from_int(p).expect("Invalid player value"));
        dispatch_board_mut!(&mut self.inner, b => b.set_piece(&pos, player))
    }

    pub fn clear(&mut self) {
        dispatch_board_mut!(&mut self.inner, b => b.clear())
    }

    pub fn __str__(&self) -> String {
        dispatch_board!(&self.inner, b => b.to_string())
    }

    pub fn __repr__(&self) -> String {
        let w = self.width();
        let h = self.height();
        format!("Board(width={}, height={})", w, h)
    }
}
