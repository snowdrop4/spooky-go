use crate::game::Game;
use crate::player::Player;
use crate::r#move::Move;

/// Number of planes for piece positions (1 for WHITE + 1 for BLACK)
const PIECE_PLANES: usize = 1 + 1;

/// Number of positions in the game history to encode
pub const HISTORY_LENGTH: usize = 8;

/// Number of constant planes (1 for current player color)
const CONSTANT_PLANES: usize = 1;

/// Total number of input planes for the neural network
pub const TOTAL_INPUT_PLANES: usize = (HISTORY_LENGTH * PIECE_PLANES) + CONSTANT_PLANES;

/// Encode the full game state into a flat f32 array for efficient transfer to Python/numpy
/// Returns (flat_data, num_planes, height, width), where flat_data is in row-major order
#[hotpath::measure]
pub fn encode_game_planes<const NW: usize>(game: &mut Game<NW>) -> (Vec<f32>, usize, usize, usize) {
    let perspective = game.turn();
    let width = game.width() as usize;
    let height = game.height() as usize;
    let num_planes = TOTAL_INPUT_PLANES;
    let board_size = height * width;
    let total_size = num_planes * board_size;
    let mut data = vec![0.0f32; total_size];

    let history_len = game.move_count();
    let steps_back = (HISTORY_LENGTH - 1).min(history_len);

    // Save moves we'll need to replay
    let moves_to_replay: Vec<Move> = game.move_history()[(history_len - steps_back)..].to_vec();

    // T=0: current position
    fill_go_planes(&mut data, game, perspective, 0, board_size);

    // T=1..steps_back: walk backward through history
    for t in 1..=steps_back {
        game.unmake_move();
        fill_go_planes(&mut data, game, perspective, t, board_size);
    }

    // Replay saved moves to restore game state
    for mv in &moves_to_replay {
        game.make_move(mv);
    }

    // Color plane (last plane)
    let color_plane_offset = (HISTORY_LENGTH * PIECE_PLANES) * board_size;
    let color_value = if perspective == Player::Black {
        1.0
    } else {
        0.0
    };
    for i in 0..board_size {
        data[color_plane_offset + i] = color_value;
    }

    (data, num_planes, height, width)
}

#[hotpath::measure]
fn fill_go_planes<const NW: usize>(
    data: &mut [f32],
    game: &Game<NW>,
    perspective: Player,
    t: usize,
    board_size: usize,
) {
    let board = game.board();
    let (own_bb, opp_bb) = match perspective {
        Player::Black => (board.black_stones(), board.white_stones()),
        Player::White => (board.white_stones(), board.black_stones()),
    };

    let own_offset = t * PIECE_PLANES * board_size;
    for idx in own_bb.iter_ones() {
        data[own_offset + idx] = 1.0;
    }

    let opp_offset = (t * PIECE_PLANES + 1) * board_size;
    for idx in opp_bb.iter_ones() {
        data[opp_offset + idx] = 1.0;
    }
}

/// Encode a move as an action index for the policy head
#[hotpath::measure]
pub fn encode_move(move_: &Move, board_width: u8, board_height: u8) -> usize {
    match move_ {
        Move::Place { col, row } => *row as usize * board_width as usize + *col as usize,
        Move::Pass => board_width as usize * board_height as usize,
    }
}

/// Returns the column number and row where the piece would land
#[hotpath::measure]
pub fn decode_move(action: usize, board_width: u8, board_height: u8) -> Option<Move> {
    let w = board_width as usize;
    let board_size = w * board_height as usize;

    if action == board_size {
        return Some(Move::pass());
    }

    if action > board_size {
        return None;
    }

    let col = (action % w) as u8;
    let row = (action / w) as u8;

    Some(Move::place(col, row))
}

#[hotpath::measure]
pub fn total_actions(board_width: u8, board_height: u8) -> usize {
    board_width as usize * board_height as usize + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitboard::nw_for_board;

    fn get_plane_value(
        data: &[f32],
        plane: usize,
        row: usize,
        col: usize,
        height: usize,
        width: usize,
    ) -> f32 {
        data[plane * height * width + row * width + col]
    }

    #[test]
    fn test_encode_game_empty() {
        let mut game = Game::<{ nw_for_board(9, 9) }>::new(9, 9);
        let (data, num_planes, height, width) = encode_game_planes(&mut game);

        assert_eq!(num_planes, TOTAL_INPUT_PLANES);
        assert_eq!(height, 9);
        assert_eq!(width, 9);
        assert_eq!(data.len(), num_planes * height * width);

        // First two planes (current and opponent pieces) should be zeros for empty board
        for plane in 0..PIECE_PLANES {
            for row in 0..height {
                for col in 0..width {
                    assert_eq!(get_plane_value(&data, plane, row, col, height, width), 0.0);
                }
            }
        }
    }

    #[test]
    fn test_encode_decode_move() {
        let width: u8 = 9;
        let height: u8 = 9;

        for row in 0..height {
            for col in 0..width {
                let move_ = Move::place(col, row);
                let encoded = encode_move(&move_, width, height);
                let decoded = decode_move(encoded, width, height)
                    .expect("test_encode_decode_move: failed to decode placement move");

                assert_eq!(decoded, move_);
            }
        }

        let pass = Move::pass();
        let encoded_pass = encode_move(&pass, width, height);
        assert_eq!(encoded_pass, width as usize * height as usize);

        let decoded_pass = decode_move(encoded_pass, width, height)
            .expect("test_encode_decode_move: failed to decode pass move");
        assert_eq!(decoded_pass, pass);
    }

    #[test]
    fn test_total_actions() {
        assert_eq!(total_actions(9, 9), 82);
        assert_eq!(total_actions(19, 19), 362);
        assert_eq!(total_actions(5, 5), 26);
    }

    #[test]
    fn test_encode_game_with_pieces() {
        let mut game = Game::<{ nw_for_board(9, 9) }>::new(9, 9);

        let move1 = Move::place(0, 0);
        game.make_move(&move1);

        let move2 = Move::place(1, 0);
        game.make_move(&move2);

        // Now it's Black's turn again, so encode from Black's perspective
        let (data, _num_planes, height, width) = encode_game_planes(&mut game);

        // From Black's perspective: Black's piece at (0,0) should be in plane 0, White's at (1,0) in plane 1
        assert_eq!(get_plane_value(&data, 0, 0, 0, height, width), 1.0);
        assert_eq!(get_plane_value(&data, 0, 0, 1, height, width), 0.0);

        assert_eq!(get_plane_value(&data, 1, 0, 0, height, width), 0.0);
        assert_eq!(get_plane_value(&data, 1, 0, 1, height, width), 1.0);
    }

    #[test]
    fn test_fuzz_encoding_random_games() {
        use rand::prelude::IndexedRandom;
        use rand::SeedableRng;
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::sync::Arc;
        use std::thread;

        let num_games = 5_000;
        let num_threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        let games_per_thread = num_games / num_threads;

        let total_moves_played = Arc::new(AtomicU64::new(0));
        let total_moves_tested = Arc::new(AtomicU64::new(0));

        let mut handles = vec![];

        for thread_id in 0..num_threads {
            let moves_played = Arc::clone(&total_moves_played);
            let moves_tested = Arc::clone(&total_moves_tested);

            let handle = thread::spawn(move || {
                let mut rng = rand::rngs::SmallRng::seed_from_u64(thread_id as u64);
                let mut thread_moves_played = 0u64;
                let mut thread_moves_tested = 0u64;

                for _game_num in 0..games_per_thread {
                    let mut game = Game::<{ nw_for_board(9, 9) }>::new(9, 9);
                    let max_moves = 100;

                    for _move_num in 0..max_moves {
                        if game.is_over() {
                            break;
                        }

                        let legal_moves = game.legal_moves();
                        if legal_moves.is_empty() {
                            break;
                        }

                        let (data, num_planes, height, width) = encode_game_planes(&mut game);
                        assert_eq!(num_planes, TOTAL_INPUT_PLANES);
                        assert_eq!(height, game.height() as usize);
                        assert_eq!(width, game.width() as usize);
                        assert_eq!(data.len(), num_planes * height * width);

                        for move_ in &legal_moves {
                            let w = game.width();
                            let h = game.height();
                            let action = encode_move(move_, w, h);
                            assert!(
                                action <= w as usize * h as usize,
                                "Invalid action {} for move {:?}",
                                action,
                                move_
                            );

                            let decoded = decode_move(action, w, h);
                            assert!(decoded.is_some(), "Failed to decode action {}", action);

                            thread_moves_tested += 1;
                        }

                        let chosen_move = legal_moves.choose(&mut rng).expect(
                            "test_fuzz_encoding_random_games: legal moves must not be empty",
                        );
                        let success = game.make_move(chosen_move);
                        assert!(success, "Failed to make move {:?}", chosen_move);

                        thread_moves_played += 1;
                    }
                }

                moves_played.fetch_add(thread_moves_played, Ordering::Relaxed);
                moves_tested.fetch_add(thread_moves_tested, Ordering::Relaxed);
            });

            handles.push(handle);
        }

        for handle in handles {
            handle
                .join()
                .expect("test_fuzz_encoding_random_games: worker thread panicked");
        }

        let final_moves_played = total_moves_played.load(Ordering::Relaxed);
        let final_moves_tested = total_moves_tested.load(Ordering::Relaxed);

        println!(
            "\nGo Encoding Fuzz Test:\n  Games: {}\n  Threads: {}\n  Moves played: {}\n  Moves tested: {}",
            num_games, num_threads, final_moves_played, final_moves_tested
        );

        assert!(final_moves_played > 0, "No moves were played");
        assert!(final_moves_tested > 0, "No moves were tested");
    }

    #[test]
    fn test_encoding_consistency() {
        use rand::prelude::IndexedRandom;
        use rand::SeedableRng;

        let mut game = Game::<{ nw_for_board(9, 9) }>::new(9, 9);
        let mut rng = rand::rngs::SmallRng::seed_from_u64(123);

        for _ in 0..20 {
            if game.is_over() {
                break;
            }

            let legal_moves = game.legal_moves();
            if legal_moves.is_empty() {
                break;
            }

            let encoding1 = encode_game_planes(&mut game);
            let encoding2 = encode_game_planes(&mut game);
            assert_eq!(encoding1, encoding2, "Encoding should be deterministic");

            let chosen_move = legal_moves
                .choose(&mut rng)
                .expect("test_encoding_consistency: legal moves must not be empty");
            game.make_move(chosen_move);
        }
    }

    #[test]
    fn test_encoding_after_undo() {
        let mut game = Game::<{ nw_for_board(9, 9) }>::new(9, 9);

        let initial_encoding = encode_game_planes(&mut game);

        let move1 = Move::place(0, 0);
        game.make_move(&move1);

        let move2 = Move::place(1, 0);
        game.make_move(&move2);

        game.unmake_move();
        game.unmake_move();

        let final_encoding = encode_game_planes(&mut game);
        assert_eq!(
            initial_encoding, final_encoding,
            "Encoding after undo should match initial state"
        );
    }

    #[test]
    fn test_plane_sizes() {
        let mut game = Game::<{ nw_for_board(9, 9) }>::new(9, 9);
        let (data, num_planes, height, width) = encode_game_planes(&mut game);

        assert_eq!(num_planes, TOTAL_INPUT_PLANES);
        assert_eq!(height, game.height() as usize);
        assert_eq!(width, game.width() as usize);
        assert_eq!(data.len(), num_planes * height * width);
    }

    #[test]
    fn test_encoding_different_positions() {
        let mut game1 = Game::<{ nw_for_board(9, 9) }>::new(9, 9);
        let mut game2 = Game::<{ nw_for_board(9, 9) }>::new(9, 9);

        game1.make_move(&Move::place(0, 0));
        game2.make_move(&Move::place(1, 0));

        let encoding1 = encode_game_planes(&mut game1);
        let encoding2 = encode_game_planes(&mut game2);

        assert_ne!(
            encoding1, encoding2,
            "Different positions should have different encodings"
        );
    }

    #[test]
    fn test_invalid_action_decoding() {
        let width: u8 = 9;
        let height: u8 = 9;
        let board_size = width as usize * height as usize;

        assert!(decode_move(board_size + 1, width, height).is_none());
        assert!(decode_move(board_size + 10, width, height).is_none());
        assert!(decode_move(1000, width, height).is_none());
    }

    #[test]
    fn test_encode_arbitrary_board_size_19x19() {
        let mut game = Game::<{ nw_for_board(19, 19) }>::new(19, 19);

        assert_eq!(game.width(), 19u8);
        assert_eq!(game.height(), 19u8);

        let (data, num_planes, height, width) = encode_game_planes(&mut game);
        assert_eq!(num_planes, TOTAL_INPUT_PLANES);
        assert_eq!(height, 19);
        assert_eq!(width, 19);
        assert_eq!(data.len(), num_planes * height * width);

        for row in 0u8..19 {
            for col in 0u8..19 {
                let move_ = Move::place(col, row);
                let encoded = encode_move(&move_, 19, 19);
                let decoded = decode_move(encoded, 19, 19)
                    .expect("test_encode_arbitrary_board_size_19x19: failed to decode 19x19 move");
                assert_eq!(decoded, move_);
            }
        }

        assert!(decode_move(362, 19, 19).is_none());
    }

    #[test]
    fn test_encode_arbitrary_board_size_5x5() {
        let mut game = Game::<{ nw_for_board(5, 5) }>::new(5, 5);

        assert_eq!(game.width(), 5u8);
        assert_eq!(game.height(), 5u8);

        let (data, num_planes, height, width) = encode_game_planes(&mut game);
        assert_eq!(num_planes, TOTAL_INPUT_PLANES);
        assert_eq!(height, 5);
        assert_eq!(width, 5);
        assert_eq!(data.len(), num_planes * height * width);
    }

    #[test]
    fn test_encode_different_board_sizes_different_encodings() {
        let mut game1 = Game::<{ nw_for_board(9, 9) }>::new(9, 9);
        let mut game2 = Game::<{ nw_for_board(19, 19) }>::new(19, 19);

        let (data1, num_planes1, height1, width1) = encode_game_planes(&mut game1);
        let (data2, num_planes2, height2, width2) = encode_game_planes(&mut game2);

        assert_eq!(num_planes1, TOTAL_INPUT_PLANES);
        assert_eq!(num_planes2, TOTAL_INPUT_PLANES);

        assert_eq!(height1, 9);
        assert_eq!(width1, 9);
        assert_eq!(data1.len(), num_planes1 * 9 * 9);

        assert_eq!(height2, 19);
        assert_eq!(width2, 19);
        assert_eq!(data2.len(), num_planes2 * 19 * 19);
    }

    #[test]
    fn test_pass_move_encoding() {
        let pass = Move::pass();

        let encoded_9x9 = encode_move(&pass, 9, 9);
        assert_eq!(encoded_9x9, 81);

        let encoded_19x19 = encode_move(&pass, 19, 19);
        assert_eq!(encoded_19x19, 361);

        let decoded = decode_move(81, 9, 9)
            .expect("test_pass_move_encoding: failed to decode pass action 81 for 9x9");
        assert!(decoded.is_pass());
    }
}
