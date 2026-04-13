use super::error::GtpError;
use crate::gtp::{GenmoveResult, GtpEngine};
use crate::player::Player;
use crate::position::Position;
use crate::r#move::Move;

fn gnugo_available() -> bool {
    std::process::Command::new("gnugo")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
}

#[test]
fn test_engine_unsupported_board_size_too_small() {
    let result = GtpEngine::new("gnugo", &["--mode", "gtp"], 3, 7.5);
    assert!(result.is_err());
    match result {
        Err(GtpError::UnsupportedBoardSize(3)) => {}
        _ => panic!("expected UnsupportedBoardSize(3)"),
    }
}

#[test]
fn test_engine_unsupported_board_size_too_large() {
    let result = GtpEngine::new("gnugo", &["--mode", "gtp"], 26, 7.5);
    assert!(result.is_err());
    match result {
        Err(GtpError::UnsupportedBoardSize(26)) => {}
        _ => panic!("expected UnsupportedBoardSize(26)"),
    }
}

#[test]
fn test_engine_invalid_program() {
    let result = GtpEngine::new("nonexistent_gtp_program_xyz", &[], 9, 7.5);
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Integration tests (require gnugo)
// ---------------------------------------------------------------------------

#[test]
fn test_gtp_engine_basic() {
    if !gnugo_available() {
        eprintln!("gnugo not found, skipping");
        return;
    }

    let mut engine =
        GtpEngine::new("gnugo", &["--mode", "gtp"], 9, 7.5).expect("failed to start gnugo");

    assert_eq!(engine.turn(), Player::Black);
    assert!(!engine.is_over());

    // Play a move
    engine.play(Move::place(2, 2)).expect("play failed");
    assert_eq!(engine.turn(), Player::White);

    // Engine generates a move
    let result = engine.genmove().expect("genmove failed");
    assert!(matches!(result, GenmoveResult::Move(_)));
    assert_eq!(engine.turn(), Player::Black);

    // Undo
    engine.undo().expect("undo failed");
    assert_eq!(engine.turn(), Player::White);

    // Clear board
    engine.clear_board().expect("clear_board failed");
    assert_eq!(engine.turn(), Player::Black);
}

#[test]
fn test_gtp_engine_raw_commands() {
    if !gnugo_available() {
        eprintln!("gnugo not found, skipping");
        return;
    }

    let mut engine =
        GtpEngine::new("gnugo", &["--mode", "gtp"], 9, 7.5).expect("failed to start gnugo");

    let name = engine.client().name().expect("name failed");
    assert!(name.contains("GNU Go") || name.contains("gnugo") || !name.is_empty());

    let version = engine.client().version().expect("version failed");
    assert!(!version.is_empty());
}

#[test]
fn test_gtp_engine_multiple_moves() {
    if !gnugo_available() {
        eprintln!("gnugo not found, skipping");
        return;
    }

    let mut engine =
        GtpEngine::new("gnugo", &["--mode", "gtp"], 9, 7.5).expect("failed to start gnugo");

    // Play several moves
    let moves = vec![
        Move::place(2, 2),
        Move::place(6, 6),
        Move::place(2, 6),
        Move::place(6, 2),
    ];

    for (i, m) in moves.iter().enumerate() {
        engine
            .play(*m)
            .unwrap_or_else(|e| panic!("move {} failed: {}", i, e));
    }

    // Have the engine play
    let result = engine.genmove().expect("genmove failed");
    assert!(matches!(result, GenmoveResult::Move(_)));

    // Verify legal moves are non-empty
    let legal = engine.legal_moves();
    assert!(!legal.is_empty());
}

#[test]
fn test_gtp_engine_play_as() {
    if !gnugo_available() {
        eprintln!("gnugo not found, skipping");
        return;
    }

    let mut engine =
        GtpEngine::new("gnugo", &["--mode", "gtp"], 9, 7.5).expect("failed to start gnugo");

    engine
        .play_as(Player::Black, Move::place(4, 4))
        .expect("play_as black failed");
    assert_eq!(engine.turn(), Player::White);

    engine
        .play_as(Player::White, Move::place(3, 3))
        .expect("play_as white failed");
    assert_eq!(engine.turn(), Player::Black);
}

#[test]
fn test_gtp_engine_genmove_as() {
    if !gnugo_available() {
        eprintln!("gnugo not found, skipping");
        return;
    }

    let mut engine =
        GtpEngine::new("gnugo", &["--mode", "gtp"], 9, 7.5).expect("failed to start gnugo");

    let result = engine.genmove_as(Player::Black).expect("genmove_as failed");
    assert!(matches!(result, GenmoveResult::Move(_)));
    assert_eq!(engine.turn(), Player::White);

    let result = engine.genmove_as(Player::White).expect("genmove_as failed");
    assert!(matches!(result, GenmoveResult::Move(_)));
    assert_eq!(engine.turn(), Player::Black);
}

#[test]
fn test_gtp_engine_pass_moves() {
    if !gnugo_available() {
        eprintln!("gnugo not found, skipping");
        return;
    }

    let mut engine =
        GtpEngine::new("gnugo", &["--mode", "gtp"], 9, 7.5).expect("failed to start gnugo");

    engine.play(Move::pass()).expect("pass 1 failed");
    assert_eq!(engine.turn(), Player::White);

    engine.play(Move::pass()).expect("pass 2 failed");
    assert_eq!(engine.turn(), Player::Black);
    assert!(engine.is_over());
}

#[test]
fn test_gtp_engine_play_on_occupied_fails() {
    if !gnugo_available() {
        eprintln!("gnugo not found, skipping");
        return;
    }

    let mut engine =
        GtpEngine::new("gnugo", &["--mode", "gtp"], 9, 7.5).expect("failed to start gnugo");

    engine.play(Move::place(3, 3)).expect("first play ok");
    engine.play(Move::place(5, 5)).expect("second play ok");

    let result = engine.play(Move::place(3, 3));
    assert!(result.is_err());
}

#[test]
fn test_gtp_engine_komi() {
    if !gnugo_available() {
        eprintln!("gnugo not found, skipping");
        return;
    }

    let mut engine =
        GtpEngine::new("gnugo", &["--mode", "gtp"], 9, 7.5).expect("failed to start gnugo");
    assert_eq!(engine.komi(), 7.5);

    engine.set_komi(6.5).expect("set_komi failed");
    assert_eq!(engine.komi(), 6.5);

    engine.set_komi(0.0).expect("set_komi zero failed");
    assert_eq!(engine.komi(), 0.0);
}

#[test]
fn test_gtp_engine_set_setup_position() {
    if !gnugo_available() {
        eprintln!("gnugo not found, skipping");
        return;
    }

    let mut engine =
        GtpEngine::new("gnugo", &["--mode", "gtp"], 9, 7.5).expect("failed to start gnugo");

    engine
        .set_setup_position(
            &[Position::new(0, 0), Position::new(2, 2)],
            &[Position::new(1, 0)],
            Player::Black,
        )
        .expect("setup position should succeed");

    assert_eq!(engine.turn(), Player::Black);
    assert!(!engine.legal_moves().contains(&Move::place(0, 0)));
    assert!(!engine.legal_moves().contains(&Move::place(1, 0)));
    assert!(!engine.legal_moves().contains(&Move::place(2, 2)));
}

#[test]
fn test_gtp_engine_legal_moves_initial() {
    if !gnugo_available() {
        eprintln!("gnugo not found, skipping");
        return;
    }

    let engine =
        GtpEngine::new("gnugo", &["--mode", "gtp"], 9, 7.5).expect("failed to start gnugo");
    let moves = engine.legal_moves();
    // 9*9 = 81 intersections + pass = 82
    assert_eq!(moves.len(), 82);
}

#[test]
fn test_gtp_engine_legal_moves_decrease_after_play() {
    if !gnugo_available() {
        eprintln!("gnugo not found, skipping");
        return;
    }

    let mut engine =
        GtpEngine::new("gnugo", &["--mode", "gtp"], 9, 7.5).expect("failed to start gnugo");
    let initial = engine.legal_moves().len();
    engine.play(Move::place(4, 4)).expect("play failed");
    let after = engine.legal_moves().len();
    assert!(after < initial);
}

#[test]
fn test_gtp_engine_clear_board_allows_replay() {
    if !gnugo_available() {
        eprintln!("gnugo not found, skipping");
        return;
    }

    let mut engine =
        GtpEngine::new("gnugo", &["--mode", "gtp"], 9, 7.5).expect("failed to start gnugo");

    engine.play(Move::place(4, 4)).expect("play failed");
    engine.clear_board().expect("clear failed");
    assert_eq!(engine.turn(), Player::Black);
    // Should be able to play same spot again
    engine.play(Move::place(4, 4)).expect("replay failed");
    assert_eq!(engine.turn(), Player::White);
}

#[test]
fn test_gtp_client_protocol_version() {
    if !gnugo_available() {
        eprintln!("gnugo not found, skipping");
        return;
    }

    let mut engine =
        GtpEngine::new("gnugo", &["--mode", "gtp"], 9, 7.5).expect("failed to start gnugo");
    let proto = engine
        .client()
        .protocol_version()
        .expect("protocol_version failed");
    assert_eq!(proto.trim(), "2");
}

#[test]
fn test_gtp_client_known_command() {
    if !gnugo_available() {
        eprintln!("gnugo not found, skipping");
        return;
    }

    let mut engine =
        GtpEngine::new("gnugo", &["--mode", "gtp"], 9, 7.5).expect("failed to start gnugo");
    assert!(engine
        .client()
        .known_command("play")
        .expect("known_command failed"));
    assert!(!engine
        .client()
        .known_command("nonexistent_xyz")
        .expect("known_command failed"));
}

#[test]
fn test_gtp_client_list_commands() {
    if !gnugo_available() {
        eprintln!("gnugo not found, skipping");
        return;
    }

    let mut engine =
        GtpEngine::new("gnugo", &["--mode", "gtp"], 9, 7.5).expect("failed to start gnugo");
    let commands = engine
        .client()
        .list_commands()
        .expect("list_commands failed");
    assert!(commands.contains(&"play".to_string()));
    assert!(commands.contains(&"genmove".to_string()));
    assert!(commands.contains(&"boardsize".to_string()));
}

#[test]
fn test_gtp_client_showboard() {
    if !gnugo_available() {
        eprintln!("gnugo not found, skipping");
        return;
    }

    let mut engine =
        GtpEngine::new("gnugo", &["--mode", "gtp"], 9, 7.5).expect("failed to start gnugo");
    let board = engine.client().showboard().expect("showboard failed");
    assert!(!board.is_empty());
}

#[test]
fn test_gtp_client_final_score() {
    if !gnugo_available() {
        eprintln!("gnugo not found, skipping");
        return;
    }

    let mut engine =
        GtpEngine::new("gnugo", &["--mode", "gtp"], 9, 7.5).expect("failed to start gnugo");

    // Play two passes to end the game
    engine.play(Move::pass()).expect("pass 1");
    engine.play(Move::pass()).expect("pass 2");

    let score = engine.client().final_score().expect("final_score failed");
    // gnugo returns something like "W+7.5" or "B+3.5"
    assert!(score.contains('+') || score.contains('0'));
}

#[test]
fn test_gtp_engine_undo_multiple() {
    if !gnugo_available() {
        eprintln!("gnugo not found, skipping");
        return;
    }

    let mut engine =
        GtpEngine::new("gnugo", &["--mode", "gtp"], 9, 7.5).expect("failed to start gnugo");

    engine.play(Move::place(2, 2)).expect("move 1");
    engine.play(Move::place(6, 6)).expect("move 2");
    engine.play(Move::place(2, 6)).expect("move 3");

    assert_eq!(engine.turn(), Player::White);

    engine.undo().expect("undo 1");
    assert_eq!(engine.turn(), Player::Black);

    engine.undo().expect("undo 2");
    assert_eq!(engine.turn(), Player::White);

    engine.undo().expect("undo 3");
    assert_eq!(engine.turn(), Player::Black);
}

#[test]
fn test_gtp_engine_mixed_play_and_genmove() {
    if !gnugo_available() {
        eprintln!("gnugo not found, skipping");
        return;
    }

    let mut engine =
        GtpEngine::new("gnugo", &["--mode", "gtp"], 9, 7.5).expect("failed to start gnugo");

    // Human plays, then engine responds
    engine.play(Move::place(4, 4)).expect("human move");
    let result = engine.genmove().expect("engine response");
    assert!(matches!(result, GenmoveResult::Move(_)));

    engine.play(Move::place(0, 0)).expect("human move 2");
    let result = engine.genmove().expect("engine response 2");
    assert!(matches!(result, GenmoveResult::Move(_)));

    assert_eq!(engine.turn(), Player::Black);
}
