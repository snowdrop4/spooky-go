use crate::player::Player;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GameOutcome {
    BlackWin,
    WhiteWin,
    Draw,
}

#[hotpath::measure_all]
impl GameOutcome {
    pub fn winner(&self) -> Option<Player> {
        match self {
            GameOutcome::BlackWin => Some(Player::Black),
            GameOutcome::WhiteWin => Some(Player::White),
            GameOutcome::Draw => None,
        }
    }

    pub fn encode_winner_absolute(&self) -> f32 {
        match self {
            GameOutcome::BlackWin => 1.0,
            GameOutcome::WhiteWin => -1.0,
            GameOutcome::Draw => 0.0,
        }
    }

    pub fn encode_winner_from_perspective(&self, perspective: Player) -> f32 {
        match perspective {
            Player::Black => match self {
                GameOutcome::BlackWin => 1.0,
                GameOutcome::WhiteWin => -1.0,
                GameOutcome::Draw => 0.0,
            },
            Player::White => match self {
                GameOutcome::BlackWin => -1.0,
                GameOutcome::WhiteWin => 1.0,
                GameOutcome::Draw => 0.0,
            },
        }
    }

    pub fn is_draw(&self) -> bool {
        matches!(self, GameOutcome::Draw)
    }
}

#[hotpath::measure_all]
impl std::fmt::Display for GameOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameOutcome::BlackWin => write!(f, "Black wins"),
            GameOutcome::WhiteWin => write!(f, "White wins"),
            GameOutcome::Draw => write!(f, "Draw"),
        }
    }
}
