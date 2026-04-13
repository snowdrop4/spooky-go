use crate::dispatch::{make_game_inner_with_options, GameInner};
use crate::limits::{board_dimension_is_valid, MAX_GTP_BOARD_SIZE, MIN_GTP_BOARD_SIZE};
use crate::player::Player;
use crate::position::Position;
use crate::r#move::Move;

use super::client::GtpClient;
use super::error::{GenmoveResult, GtpError};

/// A synchronized GTP engine that pairs a `GtpClient` with a local `Game`.
pub struct GtpEngine {
    client: GtpClient,
    game: GameInner,
    size: u8,
}

impl GtpEngine {
    /// Create a new GTP engine connection. Sends `boardsize`, `clear_board`, and `komi`
    /// to initialize the engine. The board is square (size x size).
    pub fn new(program: &str, args: &[&str], size: u8, komi: f32) -> Result<Self, GtpError> {
        if !board_dimension_is_valid(size)
            || !(MIN_GTP_BOARD_SIZE..=MAX_GTP_BOARD_SIZE).contains(&size)
        {
            return Err(GtpError::UnsupportedBoardSize(size));
        }

        let mut client = GtpClient::new(program, args)?;
        client.boardsize(size)?;
        client.clear_board()?;
        client.komi(komi)?;

        let game = make_game_inner_with_options(
            size,
            size,
            komi,
            0,        // no min_moves restriction for GTP
            u16::MAX, // effectively unlimited
            true,     // superko on
        );

        Ok(GtpEngine { client, game, size })
    }

    /// Play a move for the current turn's player.
    pub fn play(&mut self, m: Move) -> Result<(), GtpError> {
        let player = self.turn();
        self.play_as(player, m)
    }

    /// Play a move as a specific player.
    pub fn play_as(&mut self, player: Player, m: Move) -> Result<(), GtpError> {
        self.client.play(player, &m, self.size)?;
        let success = dispatch_game_mut!(&mut self.game, g => g.make_move(&m));
        if !success {
            return Err(GtpError::InvalidMove(format!(
                "local game rejected move: {}",
                m
            )));
        }
        Ok(())
    }

    /// Ask the engine to generate a move for the current turn's player.
    pub fn genmove(&mut self) -> Result<GenmoveResult, GtpError> {
        let player = self.turn();
        self.genmove_as(player)
    }

    /// Ask the engine to generate a move as a specific player.
    pub fn genmove_as(&mut self, player: Player) -> Result<GenmoveResult, GtpError> {
        let result = self.client.genmove(player, self.size)?;
        match &result {
            GenmoveResult::Move(m) => {
                let success = dispatch_game_mut!(&mut self.game, g => g.make_move(m));
                if !success {
                    return Err(GtpError::InvalidMove(format!(
                        "local game rejected engine move: {}",
                        m
                    )));
                }
            }
            GenmoveResult::Resign => {}
        }
        Ok(result)
    }

    /// Undo the last move on both the engine and local game.
    pub fn undo(&mut self) -> Result<(), GtpError> {
        self.client.undo()?;
        dispatch_game_mut!(&mut self.game, g => g.unmake_move());
        Ok(())
    }

    /// Clear the board on both the engine and local game.
    pub fn clear_board(&mut self) -> Result<(), GtpError> {
        self.client.clear_board()?;
        let komi = self.komi();
        self.game = make_game_inner_with_options(self.size, self.size, komi, 0, u16::MAX, true);
        Ok(())
    }

    /// Update komi on both the engine and local game.
    pub fn set_komi(&mut self, komi: f32) -> Result<(), GtpError> {
        self.client.komi(komi)?;
        dispatch_game_mut!(&mut self.game, g => g.set_komi(komi));
        Ok(())
    }

    /// Set an arbitrary setup position and leave `current_player` to move.
    ///
    /// This uses raw `play` commands so it works with engines that accept
    /// explicit-color setup through standard GTP. The local mirror is then
    /// reset to the same board as a fresh position.
    pub fn set_setup_position(
        &mut self,
        black_positions: &[Position],
        white_positions: &[Position],
        current_player: Player,
    ) -> Result<(), GtpError> {
        self.clear_board()?;

        let mut engine_player_to_move = Player::Black;
        for pos in black_positions {
            self.client
                .play(Player::Black, &Move::place(pos.col, pos.row), self.size)?;
            engine_player_to_move = Player::White;
        }
        for pos in white_positions {
            self.client
                .play(Player::White, &Move::place(pos.col, pos.row), self.size)?;
            engine_player_to_move = Player::Black;
        }

        if engine_player_to_move != current_player {
            self.client
                .play(engine_player_to_move, &Move::pass(), self.size)?;
        }

        let setup_result = dispatch_game_mut!(&mut self.game, g => {
            g.set_setup_position(black_positions, white_positions, current_player)
        });
        setup_result.map_err(GtpError::InvalidSetup)?;

        Ok(())
    }

    /// Get the current turn player from the local game.
    pub fn turn(&self) -> Player {
        dispatch_game!(&self.game, g => g.turn())
    }

    /// Check if the local game is over.
    pub fn is_over(&self) -> bool {
        dispatch_game!(&self.game, g => g.is_over())
    }

    /// Get legal moves from the local game.
    pub fn legal_moves(&self) -> Vec<Move> {
        dispatch_game!(&self.game, g => g.legal_moves())
    }

    /// Get the score from the local game (black_score, white_score).
    pub fn score(&self) -> (f32, f32) {
        dispatch_game!(&self.game, g => g.score())
    }

    /// Get the komi from the local game.
    pub fn komi(&self) -> f32 {
        dispatch_game!(&self.game, g => g.komi())
    }

    /// Get the board size.
    pub fn size(&self) -> u8 {
        self.size
    }

    /// Get the engine's name via the GTP `name` command.
    pub fn engine_name(&mut self) -> Option<String> {
        self.client.name().ok()
    }

    /// Get the engine's version via the GTP `version` command.
    pub fn engine_version(&mut self) -> Option<String> {
        self.client.version().ok()
    }

    /// Access the underlying GTP client for raw commands.
    pub fn client(&mut self) -> &mut GtpClient {
        &mut self.client
    }

    /// Read access to the local game state.
    #[allow(dead_code)]
    pub(crate) fn game(&self) -> &GameInner {
        &self.game
    }
}
