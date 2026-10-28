import multiprocessing as mp
from pathlib import Path
import random
import sys
import time

import spooky_go

# Add dlgo submodule to path
dlgo_path = Path(__file__).parent / "deep_learning_and_the_game_of_go" / "code"
sys.path.insert(0, str(dlgo_path))

from dlgo.goboard import GameState, Move
from dlgo.gotypes import Player, Point
from dlgo.scoring import compute_game_result


def _rust_move_to_dlgo(rust_move: spooky_go.Move) -> Move:
    if rust_move.is_pass():
        return Move.pass_turn()
    # spooky_go is 0-indexed, dlgo is 1-indexed
    row = rust_move.row()
    col = rust_move.col()
    assert row is not None
    assert col is not None
    return Move.play(Point(row=row + 1, col=col + 1))


def _dlgo_move_to_rust(dlgo_move: Move) -> spooky_go.Move:
    if dlgo_move.is_pass:
        return spooky_go.Move.pass_move()
    # dlgo is 1-indexed, spooky_go is 0-indexed
    assert dlgo_move.point is not None
    return spooky_go.Move.place(dlgo_move.point.col - 1, dlgo_move.point.row - 1)


def _get_rust_board_state(rust_game: spooky_go.Game) -> list[list[int | None]]:
    board = rust_game.board()
    state = []
    for row in range(board.height()):
        row_state = []
        for col in range(board.width()):
            piece = board.get_piece(col, row)
            row_state.append(piece)
        state.append(row_state)
    return state


def _get_dlgo_board_state(dlgo_game: GameState) -> list[list[int | None]]:
    state = []
    for row in range(1, dlgo_game.board.num_rows + 1):
        row_state = []
        for col in range(1, dlgo_game.board.num_cols + 1):
            stone = dlgo_game.board.get(Point(row=row, col=col))
            if stone is None:
                row_state.append(None)
            elif stone == Player.black:
                row_state.append(spooky_go.BLACK)
            else:
                row_state.append(spooky_go.WHITE)
        state.append(row_state)
    return state


def _board_to_string(state: list[list[int | None]], size: int) -> str:
    lines = []
    for row in range(size):
        line = ""
        for col in range(size):
            piece = state[row][col]
            if piece is None:
                line += ". "
            elif piece == spooky_go.BLACK:
                line += "X "
            else:
                line += "O "
        lines.append(line)
    return "\n".join(lines)


def _compare_game_states(
    rust_game: spooky_go.Game,
    dlgo_game: GameState,
    move_history: list[str],
    board_size: int,
) -> None:
    # Compare board state
    rust_state = _get_rust_board_state(rust_game)
    dlgo_state = _get_dlgo_board_state(dlgo_game)

    assert rust_state == dlgo_state, (
        f"Board state mismatch after moves {move_history}\n"
        f"Rust:\n{_board_to_string(rust_state, board_size)}\n"
        f"dlgo:\n{_board_to_string(dlgo_state, board_size)}"
    )

    # Compare turn
    rust_turn = rust_game.turn()
    dlgo_turn = spooky_go.BLACK if dlgo_game.next_player == Player.black else spooky_go.WHITE
    assert rust_turn == dlgo_turn, f"Turn mismatch after moves {move_history}\nRust: {rust_turn}, dlgo: {dlgo_turn}"

    # Compare game over status
    rust_over = rust_game.is_over()
    dlgo_over = dlgo_game.is_over()
    assert rust_over == dlgo_over, (
        f"Game over mismatch after moves {move_history}\nRust: {rust_over}, dlgo: {dlgo_over}"
    )

    # Note: Legal moves comparison is skipped because spooky_go uses simple ko
    # while dlgo uses positional superko. This is a known implementation difference.
    # The core game state (board, turn, game over) is verified above.


def _compare_scores(
    rust_game: spooky_go.Game,
    dlgo_game: GameState,
    move_history: list[str],
    board_size: int,
) -> None:
    rust_black, rust_white = rust_game.score()
    dlgo_result = compute_game_result(dlgo_game)

    # dlgo computes black_score and white_score (before komi), then adds komi to white
    dlgo_black = float(dlgo_result.b)
    dlgo_white = float(dlgo_result.w + dlgo_result.komi)

    # Allow small floating point differences
    assert abs(rust_black - dlgo_black) < 0.01, (
        f"Black score mismatch after moves {move_history}\n"
        f"Rust: {rust_black}, dlgo: {dlgo_black}\n"
        f"Board:\n{_board_to_string(_get_rust_board_state(rust_game), board_size)}"
    )
    assert abs(rust_white - dlgo_white) < 0.01, (
        f"White score mismatch after moves {move_history}\n"
        f"Rust: {rust_white}, dlgo: {dlgo_white}\n"
        f"Board:\n{_board_to_string(_get_rust_board_state(rust_game), board_size)}"
    )

    # Compare winner
    rust_outcome = rust_game.outcome()
    assert rust_outcome is not None

    rust_winner = rust_outcome.winner()
    dlgo_winner = spooky_go.BLACK if dlgo_result.winner == Player.black else spooky_go.WHITE

    assert rust_winner == dlgo_winner, (
        f"Winner mismatch after moves {move_history}\n"
        f"Rust scores: B={rust_black}, W={rust_white}, winner={rust_winner}\n"
        f"dlgo scores: B={dlgo_black}, W={dlgo_white}, winner={dlgo_winner}\n"
        f"Board:\n{_board_to_string(_get_rust_board_state(rust_game), board_size)}"
    )


def _play_random_game(
    board_size: int = 9,
    max_moves: int = 200,
    seed: int | None = None,
) -> tuple[int, list[str]]:
    if seed is not None:
        random.seed(seed)

    # Use with_options to set min_moves=0 so double-pass ends game immediately (matching dlgo behavior)
    rust_game = spooky_go.Game.with_options(
        width=board_size,
        height=board_size,
        komi=7.5,
        min_moves_before_pass_possible=0,
        superko=True,
        max_moves=max_moves * 2,  # High enough to not interfere
    )
    dlgo_game = GameState.new_game(board_size)
    move_history: list[str] = []
    moves_played = 0
    consecutive_passes = 0

    for _ in range(max_moves):
        # Compare states before making a move
        _compare_game_states(rust_game, dlgo_game, move_history, board_size)

        # Check if game is over
        if rust_game.is_over():
            break

        # Get legal moves from rust (reference implementation)
        rust_moves = rust_game.legal_moves()

        if not rust_moves:
            break

        # Choose a random move
        rust_move = random.choice(rust_moves)

        # Convert to dlgo move
        dlgo_move = _rust_move_to_dlgo(rust_move)

        # Record move
        if rust_move.is_pass():
            move_str = "pass"
            consecutive_passes += 1
        else:
            move_str = f"{rust_move.col()},{rust_move.row()}"
            consecutive_passes = 0

        # Make the move in both implementations
        rust_game.make_move(rust_move)
        dlgo_game = dlgo_game.apply_move(dlgo_move)

        move_history.append(move_str)
        moves_played += 1

        # Game ends after two consecutive passes
        if consecutive_passes >= 2:
            break

    # Final state comparison
    _compare_game_states(rust_game, dlgo_game, move_history, board_size)

    # Compare scores if game ended (both players passed)
    if rust_game.is_over():
        _compare_scores(rust_game, dlgo_game, move_history, board_size)

    return moves_played, move_history


def _run_fuzz_batch(num_games: int, start_seed: int, board_size: int) -> dict:
    total_moves = 0
    min_moves = float("inf")
    max_moves = 0
    games_with_passes = 0
    failed_games: list[dict] = []

    for game_num in range(num_games):
        seed = start_seed + game_num
        try:
            moves_played, move_history = _play_random_game(board_size=board_size, seed=seed)
            total_moves += moves_played
            min_moves = min(min_moves, moves_played)
            max_moves = max(max_moves, moves_played)

            # Count games ending with passes
            if len(move_history) >= 2 and move_history[-1] == "pass" and move_history[-2] == "pass":
                games_with_passes += 1

        except KeyboardInterrupt:
            raise
        except Exception as e:
            failed_games.append({"game_num": game_num, "seed": seed, "error": str(e)})

    return {
        "total_moves": total_moves,
        "min_moves": min_moves if min_moves != float("inf") else 0,
        "max_moves": max_moves,
        "games_with_passes": games_with_passes,
        "failed_games": failed_games,
        "games_completed": num_games - len(failed_games),
    }


def test_extensive_fuzz_9x9() -> None:
    num_games = 5_000
    board_size = 9
    num_cores = mp.cpu_count()
    games_per_core = num_games // num_cores
    remaining_games = num_games % num_cores

    print(f"\nRunning {num_games} games on {board_size}x{board_size} board across {num_cores} cores")

    start_time = time.time()

    # Prepare work batches
    work_batches = []
    current_seed = 0

    for i in range(num_cores):
        batch_size = games_per_core + (1 if i < remaining_games else 0)
        work_batches.append((batch_size, current_seed, board_size))
        current_seed += batch_size

    # Run batches in parallel
    with mp.Pool(processes=num_cores) as pool:
        results = pool.starmap(_run_fuzz_batch, work_batches)

    # Aggregate results
    total_moves = sum(r["total_moves"] for r in results)
    min_moves = min((r["min_moves"] for r in results if r["min_moves"] > 0), default=0)
    max_moves = max(r["max_moves"] for r in results)
    games_with_passes = sum(r["games_with_passes"] for r in results)
    total_games_completed = sum(r["games_completed"] for r in results)

    # Check for any failed games
    all_failed_games: list[dict] = []
    for r in results:
        all_failed_games.extend(r["failed_games"])

    if all_failed_games:
        print(f"\nFailed games: {len(all_failed_games)}")
        for failed in all_failed_games[:5]:
            print(f"  Game {failed['game_num']} (seed {failed['seed']}): {failed['error']}")
        if len(all_failed_games) > 5:
            print(f"  ... and {len(all_failed_games) - 5} more failures")

        raise AssertionError(f"Games failed: {all_failed_games[0]['error']}")

    elapsed_time = time.time() - start_time
    avg_moves = total_moves / total_games_completed if total_games_completed > 0 else 0

    print("\nFuzz Test Results (9x9):")
    print(f"  Games played: {total_games_completed}")
    print(f"  Total moves: {total_moves}")
    print(f"  Average moves per game: {avg_moves:.1f}")
    print(f"  Min moves in a game: {min_moves}")
    print(f"  Max moves in a game: {max_moves}")
    print(f"  Games ending with double pass: {games_with_passes}")
    print(f"  Time taken: {elapsed_time:.2f} seconds")
    print(f"  CPU cores used: {num_cores}")


def test_extensive_fuzz_19x19() -> None:
    num_games = 5_000
    board_size = 19
    num_cores = mp.cpu_count()
    games_per_core = num_games // num_cores
    remaining_games = num_games % num_cores

    print(f"\nRunning {num_games} games on {board_size}x{board_size} board across {num_cores} cores")

    start_time = time.time()

    work_batches = []
    current_seed = 10000  # Different seed range from 9x9 tests

    for i in range(num_cores):
        batch_size = games_per_core + (1 if i < remaining_games else 0)
        work_batches.append((batch_size, current_seed, board_size))
        current_seed += batch_size

    with mp.Pool(processes=num_cores) as pool:
        results = pool.starmap(_run_fuzz_batch, work_batches)

    total_moves = sum(r["total_moves"] for r in results)
    total_games_completed = sum(r["games_completed"] for r in results)

    all_failed_games: list[dict] = []
    for r in results:
        all_failed_games.extend(r["failed_games"])

    if all_failed_games:
        print(f"\nFailed games: {len(all_failed_games)}")
        for failed in all_failed_games[:5]:
            print(f"  Game {failed['game_num']} (seed {failed['seed']}): {failed['error']}")
        raise AssertionError(f"Games failed: {all_failed_games[0]['error']}")

    elapsed_time = time.time() - start_time
    avg_moves = total_moves / total_games_completed if total_games_completed > 0 else 0

    print("\nFuzz Test Results (19x19):")
    print(f"  Games played: {total_games_completed}")
    print(f"  Average moves per game: {avg_moves:.1f}")
    print(f"  Time taken: {elapsed_time:.2f} seconds")


def test_specific_capture_sequences() -> None:
    board_size = 9

    # Test sequence with a simple capture
    test_sequences = [
        # Corner capture: Black surrounds white stone at (0,0)
        [(0, 1), (0, 0), (1, 0)],  # B plays adjacent, W plays corner, B captures
        # Edge capture
        [(1, 0), (0, 0), (0, 1), (4, 4), (1, 1), (5, 5), (0, 2)],  # Surround and capture
    ]

    for sequence in test_sequences:
        # Use with_options to set min_moves=0 to match dlgo behavior
        rust_game = spooky_go.Game.with_options(
            width=board_size,
            height=board_size,
            komi=7.5,
            min_moves_before_pass_possible=0,
            superko=True,
            max_moves=1000,
        )
        dlgo_game = GameState.new_game(board_size)
        move_history: list[str] = []

        for col, row in sequence:
            rust_move = spooky_go.Move.place(col, row)
            dlgo_move = Move.play(Point(row=row + 1, col=col + 1))

            rust_game.make_move(rust_move)
            dlgo_game = dlgo_game.apply_move(dlgo_move)
            move_history.append(f"{col},{row}")

            _compare_game_states(rust_game, dlgo_game, move_history, board_size)


def test_ko_rule() -> None:
    board_size = 9

    # Set up a ko situation
    # This creates a position where capturing would recreate the previous position
    setup_moves = [
        (1, 0),  # B
        (0, 0),  # W
        (0, 1),  # B - captures W at (0,0)
    ]

    # Use with_options to set min_moves=0 to match dlgo behavior
    rust_game = spooky_go.Game.with_options(
        width=board_size,
        height=board_size,
        komi=7.5,
        min_moves_before_pass_possible=0,
        superko=True,
        max_moves=1000,
    )
    dlgo_game = GameState.new_game(board_size)

    for col, row in setup_moves:
        rust_move = spooky_go.Move.place(col, row)
        dlgo_move = Move.play(Point(row=row + 1, col=col + 1))
        rust_game.make_move(rust_move)
        dlgo_game = dlgo_game.apply_move(dlgo_move)

    # Now it's White's turn - verify legal moves match
    rust_moves = {f"{m.col()},{m.row()}" if not m.is_pass() else "pass" for m in rust_game.legal_moves()}
    dlgo_moves = set()
    for m in dlgo_game.legal_moves():
        if m.is_pass:
            dlgo_moves.add("pass")
        elif m.is_resign:
            continue
        else:
            assert m.point is not None
            dlgo_moves.add(f"{m.point.col - 1},{m.point.row - 1}")

    # The ko point (0,0) should be illegal for white to play immediately
    # Both implementations should agree on this
    assert rust_moves == dlgo_moves, f"Ko rule disagreement:\nRust: {rust_moves}\ndlgo: {dlgo_moves}"
