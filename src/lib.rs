pub mod bitboard;
pub mod board;
pub mod encode;
pub mod game;
pub(crate) mod limits;
pub mod r#move;
pub mod outcome;
pub mod player;
pub mod position;

#[allow(unused_macros)]
#[macro_use]
mod dispatch;

pub mod gtp;

#[cfg(feature = "python")]
extern crate pyo3;

#[cfg(feature = "python")]
use pyo3::prelude::*;

#[cfg(feature = "python")]
mod python;

#[cfg(feature = "python")]
#[pymodule(gil_used = false)]
#[hotpath::measure]
fn spooky_go(m: &Bound<'_, PyModule>) -> PyResult<()> {
    use player::Player;
    use python::*;
    m.add_class::<PyBoard>()?;
    m.add_class::<PyGame>()?;
    m.add_class::<PyMove>()?;
    m.add_class::<PyGameOutcome>()?;
    m.add_class::<PyGtpEngine>()?;
    m.add("BLACK", Player::Black as i8)?;
    m.add("WHITE", Player::White as i8)?;
    m.add("TOTAL_INPUT_PLANES", encode::TOTAL_INPUT_PLANES)?;
    Ok(())
}
