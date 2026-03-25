use std::collections::HashSet;
use std::hash::{Hash, Hasher};

use crate::bitboard::{nw_for_board, Bitboard, BoardGeometry};
use crate::board::{Board, STANDARD_COLS, STANDARD_ROWS};
use crate::limits::assert_supported_board_dimensions;
use crate::outcome::GameOutcome;
use crate::player::Player;
use crate::position::Position;
use crate::r#move::Move;

#[hotpath::measure]
fn compute_position_hash<const NW: usize>(board: &Board<NW>, player: Player) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    board.hash(&mut hasher);
    (player as i8).hash(&mut hasher);
    hasher.finish()
}

#[derive(Clone, Debug)]
struct MoveHistoryEntry<const NW: usize> {
    move_: Move,
    captured_stones: Bitboard<NW>,
    previous_ko_point: Option<Position>,
}

pub const DEFAULT_KOMI: f32 = 7.5;

#[derive(Clone, Debug)]
pub struct Game<const NW: usize> {
    board: Board<NW>,
    geo: BoardGeometry<NW>,
    current_player: Player,
    move_history: Vec<MoveHistoryEntry<NW>>,
    is_over: bool,
    outcome: Option<GameOutcome>,
    consecutive_passes: u8,
    ko_point: Option<Position>,
    komi: f32,
    min_moves_before_pass_possible: u16,
    max_moves: u16,
    superko: bool,
    position_hashes: Option<HashSet<u64>>,
}

struct EmptyRegionIter<'a, const NW: usize> {
    remaining: Bitboard<NW>,
    empty_mask: Bitboard<NW>,
    black: Bitboard<NW>,
    white: Bitboard<NW>,
    geo: &'a BoardGeometry<NW>,
}

impl<const NW: usize> Iterator for EmptyRegionIter<'_, NW> {
    type Item = (Bitboard<NW>, Option<Player>);

    fn next(&mut self) -> Option<Self::Item> {
        let idx = self.remaining.lowest_bit_index()?;
        let seed = Bitboard::single(idx);
        let region = self.geo.flood_fill(seed, self.empty_mask);
        self.remaining = self.remaining.andnot(region);

        let region_neighbors = self.geo.neighbors(&region);
        let black_adjacent = (region_neighbors & self.black).is_nonzero();
        let white_adjacent = (region_neighbors & self.white).is_nonzero();

        let owner = match (black_adjacent, white_adjacent) {
            (true, false) => Some(Player::Black),
            (false, true) => Some(Player::White),
            _ => None,
        };

        Some((region, owner))
    }
}

#[hotpath::measure_all]
impl<const NW: usize> Game<NW> {
    /// Create a game with dimensions in `4..=32`.
    /// `NW` must match `ceil(width * height / 64)`.
    pub fn new(width: u8, height: u8) -> Self {
        assert_supported_board_dimensions(width, height);
        let board_size = width as u16 * height as u16;
        let min_moves_before_pass_possible = board_size / 2;
        let max_moves = board_size * 3;
        Self::with_options(
            width,
            height,
            DEFAULT_KOMI,
            min_moves_before_pass_possible,
            max_moves,
            true,
        )
    }

    pub fn with_options(
        width: u8,
        height: u8,
        komi: f32,
        min_moves_before_pass_possible: u16,
        max_moves: u16,
        superko: bool,
    ) -> Self {
        assert_supported_board_dimensions(width, height);
        let board = Board::new(width, height);
        let position_hashes = if superko {
            let mut hashes = HashSet::new();
            hashes.insert(compute_position_hash(&board, Player::Black));
            hashes
        } else {
            HashSet::new()
        };
        Game {
            board,
            geo: BoardGeometry::new(width, height),
            current_player: Player::Black,
            move_history: Vec::new(),
            is_over: false,
            outcome: None,
            consecutive_passes: 0,
            ko_point: None,
            komi,
            min_moves_before_pass_possible,
            max_moves,
            superko,
            position_hashes: if superko { Some(position_hashes) } else { None },
        }
    }

    pub fn komi(&self) -> f32 {
        self.komi
    }

    pub fn set_komi(&mut self, komi: f32) {
        self.komi = komi;
    }

    pub fn min_moves_before_pass_possible(&self) -> u16 {
        self.min_moves_before_pass_possible
    }

    pub fn max_moves(&self) -> u16 {
        self.max_moves
    }

    pub fn move_count(&self) -> usize {
        self.move_history.len()
    }

    pub fn width(&self) -> u8 {
        self.board.width()
    }

    pub fn height(&self) -> u8 {
        self.board.height()
    }

    pub fn get_piece(&self, pos: &Position) -> Option<i8> {
        self.board.get_piece(pos).map(|p| p as i8)
    }

    pub fn set_piece(&mut self, pos: &Position, player: Option<Player>) {
        self.board.set_piece(pos, player)
    }

    pub fn board(&self) -> &Board<NW> {
        &self.board
    }

    pub fn turn(&self) -> Player {
        self.current_player
    }

    pub fn is_over(&self) -> bool {
        self.is_over
    }

    pub fn outcome(&self) -> Option<GameOutcome> {
        self.outcome
    }

    pub fn move_history(&self) -> Vec<Move> {
        self.move_history.iter().map(|e| e.move_).collect()
    }

    pub fn ko_point(&self) -> Option<Position> {
        self.ko_point
    }

    pub fn superko(&self) -> bool {
        self.superko
    }

    /// Simulate placing a stone and performing captures, returning the resulting board.
    fn simulate_placement(&self, idx: usize, player: Player) -> Board<NW> {
        let mut board = self.board;
        board.set_bit(idx, player);

        let opponent = player.opposite();
        let bit = Bitboard::single(idx);
        let adj_opp = self.geo.neighbors(&bit) & board.stones_for(opponent);

        let mut remaining = adj_opp;
        while let Some(opp_idx) = remaining.lowest_bit_index() {
            let opp_seed = Bitboard::single(opp_idx);
            let opp_group = self.geo.flood_fill(opp_seed, board.stones_for(opponent));
            remaining &= !opp_group;

            let opp_neighbors = self.geo.neighbors(&opp_group);
            if (opp_neighbors & board.empty_squares(self.geo.board_mask)).is_empty() {
                board.remove_stones(opp_group);
            }
        }
        board
    }

    fn is_illegal_placement(&self, idx: usize, player: Player) -> bool {
        let bit = Bitboard::single(idx);
        let own = self.board.stones_for(player) | bit;
        let opponent = player.opposite();
        let opp = self.board.stones_for(opponent);
        let occupied = own | opp;
        let empty = self.geo.board_mask.andnot(occupied);
        let bit_neighbors = self.geo.neighbors(&bit);

        // Fast path: placed stone has an empty neighbor -> not suicide
        if (bit_neighbors & empty).is_nonzero() {
            // Check superko only if captures occur
            if self.superko {
                let adj_opp = bit_neighbors & opp;
                if adj_opp.is_nonzero() && self.adj_opp_has_captures(adj_opp, opp, empty) {
                    return self.check_superko(idx, player);
                }
            }
            return false;
        }

        // No immediate liberties. Flood-fill own group.
        let group = self.geo.flood_fill(bit, own);
        let group_neighbors = self.geo.neighbors(&group);

        // Group has liberties through connected friendly stones -> not suicide
        if (group_neighbors & empty).is_nonzero() {
            if self.superko {
                let adj_opp = bit_neighbors & opp;
                if adj_opp.is_nonzero() && self.adj_opp_has_captures(adj_opp, opp, empty) {
                    return self.check_superko(idx, player);
                }
            }
            return false;
        }

        // No liberties for our group. Check if we capture any opponent groups.
        let adj_opp = group_neighbors & opp;
        if adj_opp.is_empty() {
            return true; // Suicide — no opponent neighbors to capture
        }

        let mut remaining = adj_opp;
        let mut any_captures = false;
        while let Some(opp_idx) = remaining.lowest_bit_index() {
            let opp_seed = Bitboard::single(opp_idx);
            let opp_group = self.geo.flood_fill(opp_seed, opp);
            remaining = remaining.andnot(opp_group);
            let opp_nbrs = self.geo.neighbors(&opp_group);
            if (opp_nbrs & empty).is_empty() {
                any_captures = true;
                break;
            }
        }

        if !any_captures {
            return true; // Suicide
        }

        // Not suicide (captures save us). Check superko only when captures occur.
        if self.superko {
            return self.check_superko(idx, player);
        }

        false
    }

    /// Check if any adjacent opponent group has zero liberties (would be captured).
    fn adj_opp_has_captures(
        &self,
        adj_opp: Bitboard<NW>,
        opp: Bitboard<NW>,
        empty: Bitboard<NW>,
    ) -> bool {
        let mut remaining = adj_opp;
        while let Some(opp_idx) = remaining.lowest_bit_index() {
            let opp_seed = Bitboard::single(opp_idx);
            let opp_group = self.geo.flood_fill(opp_seed, opp);
            remaining = remaining.andnot(opp_group);
            let opp_nbrs = self.geo.neighbors(&opp_group);
            if (opp_nbrs & empty).is_empty() {
                return true;
            }
        }
        false
    }

    fn check_superko(&self, idx: usize, player: Player) -> bool {
        if let Some(ref hashes) = self.position_hashes {
            let result_board = self.simulate_placement(idx, player);
            let hash = compute_position_hash(&result_board, player.opposite());
            hashes.contains(&hash)
        } else {
            false
        }
    }

    fn empty_regions(&self) -> EmptyRegionIter<'_, NW> {
        let occupied = self.board.occupied();
        EmptyRegionIter {
            remaining: self.board.empty_squares(self.geo.board_mask),
            empty_mask: self.geo.board_mask.andnot(occupied),
            black: self.board.black_stones(),
            white: self.board.white_stones(),
            geo: &self.geo,
        }
    }

    pub fn score(&self) -> (f32, f32) {
        let mut black_score = self.board.black_stones().count() as f32;
        let mut white_score = self.board.white_stones().count() as f32 + self.komi;

        for (region, owner) in self.empty_regions() {
            let territory = region.count() as f32;
            match owner {
                Some(Player::Black) => black_score += territory,
                Some(Player::White) => white_score += territory,
                None => {}
            }
        }

        (black_score, white_score)
    }

    // Per-square ownership from black's (first player's) absolute perspective.
    // +1.0 = black owns, -1.0 = white owns, 0.0 = neutral/disputed.
    // Stones count as owned by their player; empty regions are assigned
    // based on which player's stones exclusively border them (area scoring).
    // Layout: row-major, index = row * width + col.
    pub fn ownership_map_absolute(&self) -> Vec<f32> {
        let w = self.board.width() as usize;
        let h = self.board.height() as usize;
        let mut ownership = vec![0.0f32; h * w];

        for idx in self.board.black_stones().iter_ones() {
            ownership[idx] = 1.0;
        }
        for idx in self.board.white_stones().iter_ones() {
            ownership[idx] = -1.0;
        }

        for (region, owner) in self.empty_regions() {
            let value = match owner {
                Some(Player::Black) => 1.0,
                Some(Player::White) => -1.0,
                None => 0.0,
            };
            for region_idx in region.iter_ones() {
                ownership[region_idx] = value;
            }
        }

        ownership
    }

    pub fn ownership_map_from_perspective(&self, perspective: Player) -> Vec<f32> {
        let mut ownership = self.ownership_map_absolute();
        if perspective == Player::White {
            for v in &mut ownership {
                *v = -*v;
            }
        }
        ownership
    }

    // Score margin from black's absolute perspective (includes komi).
    // Positive means black is ahead.
    pub fn score_margin_absolute(&self) -> f32 {
        let (black_score, white_score) = self.score();
        black_score - white_score
    }

    pub fn score_margin_from_perspective(&self, perspective: Player) -> f32 {
        let margin = self.score_margin_absolute();
        match perspective {
            Player::Black => margin,
            Player::White => -margin,
        }
    }

    fn determine_outcome(&self) -> GameOutcome {
        let (black_score, white_score) = self.score();
        if black_score > white_score {
            GameOutcome::BlackWin
        } else if white_score > black_score {
            GameOutcome::WhiteWin
        } else {
            GameOutcome::Draw
        }
    }

    pub fn legal_moves(&self) -> Vec<Move> {
        if self.is_over {
            return Vec::new();
        }

        let mut moves = Vec::new();
        let empty = self.board.empty_squares(self.geo.board_mask);
        let w = self.geo.width;
        let ko_idx = self.ko_point.map(|p| p.to_index(w));

        for idx in empty.iter_ones() {
            if let Some(ki) = ko_idx {
                if ki == idx {
                    continue;
                }
            }

            if self.is_illegal_placement(idx, self.current_player) {
                continue;
            }

            let pos = Position::from_index(idx, w);
            moves.push(Move::place(pos.col, pos.row));
        }

        if moves.is_empty()
            || self.move_history.len() >= self.min_moves_before_pass_possible as usize
        {
            moves.push(Move::pass());
        }

        moves
    }

    fn has_legal_board_moves(&self) -> bool {
        let empty = self.board.empty_squares(self.geo.board_mask);
        let w = self.geo.width;
        let ko_idx = self.ko_point.map(|p| p.to_index(w));

        for idx in empty.iter_ones() {
            if let Some(ki) = ko_idx {
                if ki == idx {
                    continue;
                }
            }

            if self.is_illegal_placement(idx, self.current_player) {
                continue;
            }

            return true;
        }

        false
    }

    pub fn is_legal_move(&self, move_: &Move) -> bool {
        if self.is_over {
            return false;
        }

        match move_ {
            Move::Pass => {
                self.move_history.len() >= self.min_moves_before_pass_possible as usize
                    || !self.has_legal_board_moves()
            }
            Move::Place { col, row } => {
                let pos = Position::new(*col, *row);

                if !pos.is_valid(self.board.width(), self.board.height()) {
                    return false;
                }

                let idx = pos.to_index(self.board.width());

                if self.board.occupied().get(idx) {
                    return false;
                }

                if let Some(ko) = self.ko_point {
                    if ko == pos {
                        return false;
                    }
                }

                !self.is_illegal_placement(idx, self.current_player)
            }
        }
    }

    pub fn make_move(&mut self, move_: &Move) -> bool {
        if !self.is_legal_move(move_) {
            return false;
        }

        let previous_ko_point = self.ko_point;
        let mut captured_stones = Bitboard::empty();
        self.ko_point = None;

        match move_ {
            Move::Pass => {
                self.consecutive_passes += 1;

                if self.consecutive_passes >= 2 {
                    self.is_over = true;
                    self.outcome = Some(self.determine_outcome());
                }
            }
            Move::Place { col, row } => {
                self.consecutive_passes = 0;

                let pos = Position::new(*col, *row);
                let idx = pos.to_index(self.board.width());
                self.board.set_bit(idx, self.current_player);

                let opponent = self.current_player.opposite();
                let bit = Bitboard::single(idx);
                let bit_neighbors = self.geo.neighbors(&bit);
                let adjacent_opponent = bit_neighbors & self.board.stones_for(opponent);

                let mut total_captured: u32 = 0;
                let mut single_capture_idx: Option<usize> = None;

                let mut remaining = adjacent_opponent;
                while let Some(opp_idx) = remaining.lowest_bit_index() {
                    let opp_seed = Bitboard::single(opp_idx);
                    let opp_group = self
                        .geo
                        .flood_fill(opp_seed, self.board.stones_for(opponent));

                    remaining &= !opp_group;

                    let opp_neighbors = self.geo.neighbors(&opp_group);
                    let opp_empty = self.board.empty_squares(self.geo.board_mask);
                    if (opp_neighbors & opp_empty).is_empty() {
                        let group_size = opp_group.count();
                        if group_size == 1 && total_captured == 0 {
                            single_capture_idx = Some(opp_idx);
                        } else {
                            single_capture_idx = None;
                        }
                        total_captured += group_size;
                        captured_stones |= opp_group;
                        self.board.remove_stones(opp_group);
                    }
                }

                // Ko detection
                if total_captured == 1 {
                    if let Some(cap_idx) = single_capture_idx {
                        let placed_group = self
                            .geo
                            .flood_fill(bit, self.board.stones_for(self.current_player));
                        if placed_group.count() == 1 {
                            let placed_neighbors = self.geo.neighbors(&placed_group);
                            let placed_liberties =
                                placed_neighbors & self.board.empty_squares(self.geo.board_mask);
                            if placed_liberties.count() == 1 {
                                self.ko_point =
                                    Some(Position::from_index(cap_idx, self.board.width()));
                            }
                        }
                    }
                }
            }
        }

        self.move_history.push(MoveHistoryEntry {
            move_: *move_,
            captured_stones,
            previous_ko_point,
        });

        self.current_player = self.current_player.opposite();

        if let Some(ref mut hashes) = self.position_hashes {
            hashes.insert(compute_position_hash(&self.board, self.current_player));
        }

        // Check max moves limit
        if !self.is_over && self.move_history.len() >= self.max_moves as usize {
            self.is_over = true;
            self.outcome = Some(self.determine_outcome());
        }

        true
    }

    pub fn unmake_move(&mut self) -> bool {
        if let Some(entry) = self.move_history.pop() {
            if let Some(ref mut hashes) = self.position_hashes {
                let hash = compute_position_hash(&self.board, self.current_player);
                hashes.remove(&hash);
            }

            self.current_player = self.current_player.opposite();
            self.ko_point = entry.previous_ko_point;

            match entry.move_ {
                Move::Pass => {
                    self.consecutive_passes = self.consecutive_passes.saturating_sub(1);
                    self.is_over = false;
                    self.outcome = None;
                }
                Move::Place { col, row } => {
                    let pos = Position::new(col, row);
                    let idx = pos.to_index(self.board.width());
                    self.board.clear_bit(idx);

                    let opponent = self.current_player.opposite();
                    self.board.restore_stones(entry.captured_stones, opponent);

                    self.is_over = false;
                    self.outcome = None;
                }
            }

            true
        } else {
            false
        }
    }
}

#[hotpath::measure_all]
impl Game<{ nw_for_board(STANDARD_COLS, STANDARD_ROWS) }> {
    pub fn standard() -> Self {
        Self::new(STANDARD_COLS, STANDARD_ROWS)
    }
}

#[hotpath::measure_all]
impl Default for Game<{ nw_for_board(STANDARD_COLS, STANDARD_ROWS) }> {
    fn default() -> Self {
        Self::standard()
    }
}

#[hotpath::measure_all]
impl<const NW: usize> std::fmt::Display for Game<NW> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Game(turn: {}, is_over: {}, outcome: {:?})\n{}",
            self.current_player, self.is_over, self.outcome, self.board
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_game() {
        let game = Game::<{ nw_for_board(19, 19) }>::standard();
        assert_eq!(game.turn(), Player::Black);
        assert!(!game.is_over());
        assert!(game.outcome().is_none());
    }

    #[test]
    fn test_legal_moves_initial() {
        let game = Game::<{ nw_for_board(9, 9) }>::new(9, 9);
        let moves = game.legal_moves();
        // Pass is not legal initially because min_moves_before_pass_possible > 0
        assert_eq!(moves.len(), 9 * 9);
    }

    #[test]
    fn test_legal_moves_initial_with_pass() {
        let game = Game::<{ nw_for_board(9, 9) }>::with_options(9, 9, DEFAULT_KOMI, 0, 1000, false);
        let moves = game.legal_moves();
        assert_eq!(moves.len(), 9 * 9 + 1);
    }

    #[test]
    fn test_make_move() {
        let mut game = Game::<{ nw_for_board(9, 9) }>::new(9, 9);
        let move_ = Move::place(0, 0);

        assert!(game.is_legal_move(&move_));
        assert!(game.make_move(&move_));
        assert_eq!(game.turn(), Player::White);
    }

    #[test]
    fn test_make_invalid_move() {
        let mut game = Game::<{ nw_for_board(9, 9) }>::new(9, 9);
        let move_ = Move::place(10, 0);

        assert!(!game.is_legal_move(&move_));
        assert!(!game.make_move(&move_));
    }

    #[test]
    fn test_occupied_position() {
        let mut game = Game::<{ nw_for_board(9, 9) }>::new(9, 9);
        let move_ = Move::place(0, 0);

        game.make_move(&move_);

        let same_pos = Move::place(0, 0);
        assert!(!game.is_legal_move(&same_pos));
    }

    #[test]
    fn test_unmake_move() {
        let mut game = Game::<{ nw_for_board(9, 9) }>::new(9, 9);
        let move_ = Move::place(0, 0);

        game.make_move(&move_);
        assert_eq!(game.turn(), Player::White);

        assert!(game.unmake_move());
        assert_eq!(game.turn(), Player::Black);
        assert_eq!(game.move_history().len(), 0);
        assert!(game.board().get_piece(&Position::new(0, 0)).is_none());
    }

    #[test]
    fn test_pass_move() {
        let mut game =
            Game::<{ nw_for_board(9, 9) }>::with_options(9, 9, DEFAULT_KOMI, 0, 1000, false);

        assert!(game.make_move(&Move::pass()));
        assert_eq!(game.turn(), Player::White);
        assert!(!game.is_over());

        assert!(game.make_move(&Move::pass()));
        assert!(game.is_over());
        assert_eq!(game.outcome(), Some(GameOutcome::WhiteWin));
    }

    #[test]
    fn test_pass_not_legal_before_min_moves() {
        let mut game = Game::<{ nw_for_board(9, 9) }>::new(9, 9);
        assert_eq!(game.min_moves_before_pass_possible(), 40u16);

        // Pass should not be legal before min_moves_before_pass_possible
        assert!(!game.is_legal_move(&Move::pass()));
        assert!(!game.make_move(&Move::pass()));
    }

    #[test]
    fn test_pass_ends_game_after_min_moves() {
        let mut game =
            Game::<{ nw_for_board(9, 9) }>::with_options(9, 9, DEFAULT_KOMI, 4, 1000, false);

        // Pass not legal before 4 moves
        assert!(!game.is_legal_move(&Move::pass()));
        game.make_move(&Move::place(0, 0));
        game.make_move(&Move::place(1, 0));
        game.make_move(&Move::place(2, 0));
        game.make_move(&Move::place(3, 0));
        // Now at 4 moves, pass is legal
        assert!(game.is_legal_move(&Move::pass()));
        game.make_move(&Move::pass());
        game.make_move(&Move::pass());
        assert!(game.is_over());
    }

    #[test]
    fn test_max_moves_ends_game() {
        let mut game =
            Game::<{ nw_for_board(9, 9) }>::with_options(9, 9, DEFAULT_KOMI, 100, 5, false);

        game.make_move(&Move::place(0, 0));
        game.make_move(&Move::place(1, 0));
        game.make_move(&Move::place(2, 0));
        game.make_move(&Move::place(3, 0));
        assert!(!game.is_over());

        game.make_move(&Move::place(4, 0));
        assert!(game.is_over());
        assert!(game.outcome().is_some());
    }

    #[test]
    fn test_scoring_black_wins() {
        let mut game = Game::<{ nw_for_board(5, 5) }>::with_options(5, 5, 0.5, 0, 1000, false);

        game.make_move(&Move::place(0, 0));
        game.make_move(&Move::pass());
        game.make_move(&Move::place(1, 0));
        game.make_move(&Move::pass());
        game.make_move(&Move::place(0, 1));
        game.make_move(&Move::pass());
        game.make_move(&Move::place(1, 1));
        game.make_move(&Move::pass());
        game.make_move(&Move::pass());

        assert!(game.is_over());
        let (black_score, white_score) = game.score();
        assert!(black_score > white_score);
        assert_eq!(game.outcome(), Some(GameOutcome::BlackWin));
    }

    #[test]
    fn test_scoring_with_territory() {
        let mut game = Game::<{ nw_for_board(5, 5) }>::with_options(5, 5, 0.0, 0, 1000, false);

        game.make_move(&Move::place(0, 2));
        game.make_move(&Move::pass());
        game.make_move(&Move::place(0, 3));
        game.make_move(&Move::pass());
        game.make_move(&Move::place(1, 2));
        game.make_move(&Move::pass());
        game.make_move(&Move::pass());

        let (black_score, white_score) = game.score();
        assert!(black_score > white_score);
        assert_eq!(game.outcome(), Some(GameOutcome::BlackWin));
    }

    #[test]
    fn test_simple_capture() {
        let mut game = Game::<{ nw_for_board(5, 5) }>::new(5, 5);

        game.make_move(&Move::place(1, 0));
        game.make_move(&Move::place(0, 0));
        game.make_move(&Move::place(0, 1));

        assert!(game.board().get_piece(&Position::new(0, 0)).is_none());
    }

    #[test]
    fn test_capture_group() {
        let mut game =
            Game::<{ nw_for_board(5, 5) }>::with_options(5, 5, DEFAULT_KOMI, 0, 1000, true);

        game.make_move(&Move::place(0, 0));
        game.make_move(&Move::place(1, 0));

        game.make_move(&Move::place(0, 1));
        game.make_move(&Move::place(1, 1));

        game.make_move(&Move::pass());
        game.make_move(&Move::place(0, 2));

        game.make_move(&Move::pass());
        game.make_move(&Move::place(2, 0));

        game.make_move(&Move::pass());
        game.make_move(&Move::place(2, 1));

        assert!(game.board().get_piece(&Position::new(0, 0)).is_none());
        assert!(game.board().get_piece(&Position::new(0, 1)).is_none());
        assert!(game.board().get_piece(&Position::new(1, 0)).is_some());
        assert!(game.board().get_piece(&Position::new(1, 1)).is_some());
    }

    #[test]
    fn test_suicide_prevention() {
        let mut game =
            Game::<{ nw_for_board(5, 5) }>::with_options(5, 5, DEFAULT_KOMI, 0, 1000, true);

        game.make_move(&Move::place(1, 0));
        game.make_move(&Move::pass());
        game.make_move(&Move::place(0, 1));
        game.make_move(&Move::pass());

        let suicide_move = Move::place(0, 0);
        assert!(game.is_legal_move(&suicide_move));
        game.make_move(&suicide_move);
        assert!(game.board().get_piece(&Position::new(0, 0)).is_some());
    }

    #[test]
    fn test_actual_suicide_prevention() {
        let mut game =
            Game::<{ nw_for_board(5, 5) }>::with_options(5, 5, DEFAULT_KOMI, 0, 1000, false);

        game.make_move(&Move::place(1, 0));
        game.make_move(&Move::pass());
        game.make_move(&Move::place(0, 1));
        game.make_move(&Move::pass());
        game.make_move(&Move::pass());

        let suicide_move = Move::place(0, 0);
        assert!(!game.is_legal_move(&suicide_move));
    }

    #[test]
    fn test_ko_rule() {
        let mut game =
            Game::<{ nw_for_board(5, 5) }>::with_options(5, 5, DEFAULT_KOMI, 0, 1000, true);

        game.make_move(&Move::place(1, 0));
        game.make_move(&Move::place(2, 0));

        game.make_move(&Move::place(0, 1));
        game.make_move(&Move::place(1, 1));

        game.make_move(&Move::place(1, 2));
        game.make_move(&Move::place(2, 2));

        game.make_move(&Move::pass());
        game.make_move(&Move::place(3, 1));

        let ko_capture = Move::place(2, 1);
        assert!(game.is_legal_move(&ko_capture));
        game.make_move(&ko_capture);

        assert!(game.board().get_piece(&Position::new(1, 1)).is_none());
        assert_eq!(game.ko_point(), Some(Position::new(1, 1)));

        let immediate_recapture = Move::place(1, 1);
        assert!(!game.is_legal_move(&immediate_recapture));
    }

    #[test]
    fn test_unmake_restores_captures() {
        let mut game = Game::<{ nw_for_board(5, 5) }>::new(5, 5);

        game.make_move(&Move::place(1, 0));
        game.make_move(&Move::place(0, 0));
        game.make_move(&Move::place(0, 1));

        assert!(game.board().get_piece(&Position::new(0, 0)).is_none());

        game.unmake_move();

        assert_eq!(
            game.board().get_piece(&Position::new(0, 0)),
            Some(Player::White)
        );
    }

    #[test]
    fn test_move_history() {
        let mut game = Game::<{ nw_for_board(9, 9) }>::new(9, 9);

        assert_eq!(game.move_history().len(), 0);

        let move1 = Move::place(0, 0);
        game.make_move(&move1);
        assert_eq!(game.move_history().len(), 1);

        let move2 = Move::place(1, 0);
        game.make_move(&move2);
        assert_eq!(game.move_history().len(), 2);

        game.unmake_move();
        assert_eq!(game.move_history().len(), 1);
    }

    #[test]
    fn test_legal_moves_when_game_over() {
        let mut game =
            Game::<{ nw_for_board(9, 9) }>::with_options(9, 9, DEFAULT_KOMI, 0, 1000, false);

        game.make_move(&Move::pass());
        game.make_move(&Move::pass());

        assert!(game.is_over());
        assert_eq!(game.legal_moves().len(), 0);
    }

    #[test]
    fn test_superko_unmake_restores() {
        let mut game = Game::<{ nw_for_board(5, 5) }>::new(5, 5);

        game.make_move(&Move::place(1, 0));
        game.make_move(&Move::place(2, 0));

        game.make_move(&Move::place(0, 1));
        game.make_move(&Move::place(1, 1));

        game.make_move(&Move::place(1, 2));
        game.make_move(&Move::place(2, 2));

        game.make_move(&Move::pass());
        game.make_move(&Move::place(3, 1));

        game.make_move(&Move::place(2, 1));

        assert!(!game.is_legal_move(&Move::place(1, 1)));

        game.unmake_move();

        assert!(game.is_legal_move(&Move::place(2, 1)));
    }
}
