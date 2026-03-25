import subprocess

import pytest

from spooky_go import BLACK, WHITE, GtpEngine, Move


def gnugo_available() -> bool:
    try:
        subprocess.run(
            ["gnugo", "--version"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        return True
    except FileNotFoundError:
        return False


pytestmark = pytest.mark.skipif(not gnugo_available(), reason="gnugo not found")


def make_engine(size: int = 9, komi: float = 7.5) -> GtpEngine:
    return GtpEngine("gnugo", ["--mode", "gtp"], size=size, komi=komi)


class TestGtpEngineCreation:
    def test_invalid_size_too_small(self) -> None:
        with pytest.raises(RuntimeError):
            make_engine(size=3)

    def test_invalid_size_too_large(self) -> None:
        with pytest.raises(RuntimeError):
            make_engine(size=26)

    def test_invalid_program(self) -> None:
        with pytest.raises(RuntimeError):
            GtpEngine("nonexistent_program_xyz")


class TestGtpEngineTurn:
    def test_initial_turn_is_black(self) -> None:
        engine = make_engine()
        assert engine.turn() == BLACK
        engine.quit()

    def test_turn_alternates_after_play(self) -> None:
        engine = make_engine()
        assert engine.turn() == BLACK
        engine.play(Move.place(2, 2))
        assert engine.turn() == WHITE
        engine.play(Move.place(4, 4))
        assert engine.turn() == BLACK
        engine.quit()

    def test_turn_alternates_after_pass(self) -> None:
        engine = make_engine()
        engine.play(Move.pass_move())
        assert engine.turn() == WHITE
        engine.play(Move.pass_move())
        assert engine.turn() == BLACK
        engine.quit()


class TestGtpEnginePlay:
    def test_play_multiple_moves(self) -> None:
        engine = make_engine()
        moves = [Move.place(2, 2), Move.place(6, 6), Move.place(2, 6), Move.place(6, 2)]
        for m in moves:
            engine.play(m)
        assert engine.turn() == BLACK
        engine.quit()

    def test_play_on_occupied_raises(self) -> None:
        engine = make_engine()
        engine.play(Move.place(3, 3))
        engine.play(Move.place(5, 5))
        with pytest.raises(RuntimeError):
            engine.play(Move.place(3, 3))
        engine.quit()


class TestGtpEnginePlayAs:
    def test_play_as_black(self) -> None:
        engine = make_engine()
        engine.play_as(BLACK, Move.place(4, 4))
        assert engine.turn() == WHITE
        engine.quit()

    def test_play_as_white(self) -> None:
        engine = make_engine()
        # Play black first, then white
        engine.play_as(BLACK, Move.place(4, 4))
        engine.play_as(WHITE, Move.place(3, 3))
        assert engine.turn() == BLACK
        engine.quit()

    def test_play_as_invalid_player(self) -> None:
        engine = make_engine()
        with pytest.raises(RuntimeError):
            engine.play_as(0, Move.place(4, 4))
        engine.quit()


class TestGtpEngineGenmove:
    def test_genmove_returns_move(self) -> None:
        engine = make_engine()
        result = engine.genmove()
        assert result is not None  # gnugo won't resign on first move
        assert isinstance(result, Move)
        engine.quit()

    def test_genmove_advances_turn(self) -> None:
        engine = make_engine()
        assert engine.turn() == BLACK
        engine.genmove()
        assert engine.turn() == WHITE
        engine.quit()

    def test_genmove_after_play(self) -> None:
        engine = make_engine()
        engine.play(Move.place(4, 4))
        result = engine.genmove()
        assert result is not None
        assert engine.turn() == BLACK
        engine.quit()


class TestGtpEngineGenmoveAs:
    def test_genmove_as_black(self) -> None:
        engine = make_engine()
        result = engine.genmove_as(BLACK)
        assert result is not None
        engine.quit()

    def test_genmove_as_white(self) -> None:
        engine = make_engine()
        engine.play(Move.place(4, 4))
        result = engine.genmove_as(WHITE)
        assert result is not None
        engine.quit()

    def test_genmove_as_invalid_player(self) -> None:
        engine = make_engine()
        with pytest.raises(RuntimeError):
            engine.genmove_as(0)
        engine.quit()


class TestGtpEngineUndo:
    def test_undo_reverts_turn(self) -> None:
        engine = make_engine()
        engine.play(Move.place(2, 2))
        assert engine.turn() == WHITE
        engine.undo()
        assert engine.turn() == BLACK
        engine.quit()

    def test_undo_after_multiple_moves(self) -> None:
        engine = make_engine()
        engine.play(Move.place(2, 2))
        engine.play(Move.place(6, 6))
        assert engine.turn() == BLACK
        engine.undo()
        assert engine.turn() == WHITE
        engine.undo()
        assert engine.turn() == BLACK
        engine.quit()


class TestGtpEngineClearBoard:
    def test_clear_board_resets_turn(self) -> None:
        engine = make_engine()
        engine.play(Move.place(2, 2))
        engine.clear_board()
        assert engine.turn() == BLACK
        engine.quit()

    def test_clear_board_allows_replay(self) -> None:
        engine = make_engine()
        engine.play(Move.place(4, 4))
        engine.clear_board()
        # Should be able to play on same spot after clear
        engine.play(Move.place(4, 4))
        assert engine.turn() == WHITE
        engine.quit()


class TestGtpEngineLegalMoves:
    def test_legal_moves_initial(self) -> None:
        engine = make_engine(size=9)
        moves = engine.legal_moves()
        # 9x9 = 81 intersections + pass = 82 legal moves
        assert len(moves) == 82
        engine.quit()

    def test_legal_moves_after_play(self) -> None:
        engine = make_engine(size=9)
        initial_count = len(engine.legal_moves())
        engine.play(Move.place(4, 4))
        after_count = len(engine.legal_moves())
        # At least one fewer legal move (the occupied spot)
        assert after_count < initial_count
        engine.quit()

    def test_legal_moves_contains_pass(self) -> None:
        engine = make_engine(size=9)
        moves = engine.legal_moves()
        pass_moves = [m for m in moves if m.is_pass()]
        assert len(pass_moves) == 1
        engine.quit()


class TestGtpEngineIsOver:
    def test_not_over_initially(self) -> None:
        engine = make_engine()
        assert not engine.is_over()
        engine.quit()

    def test_over_after_two_passes(self) -> None:
        engine = make_engine()
        engine.play(Move.pass_move())
        engine.play(Move.pass_move())
        assert engine.is_over()
        engine.quit()


class TestGtpEngineSendCommand:
    def test_send_invalid_command_raises(self) -> None:
        engine = make_engine()
        with pytest.raises(RuntimeError):
            engine.send_command("totally_invalid_command_xyz", [])
        engine.quit()


class TestGtpEngineQuit:
    def test_quit(self) -> None:
        engine = make_engine()
        engine.quit()
        # After quit, all methods should raise
        with pytest.raises(RuntimeError):
            engine.turn()

    def test_quit_idempotent(self) -> None:
        engine = make_engine()
        engine.quit()
        engine.quit()  # Should not raise

    def test_methods_raise_after_quit(self) -> None:
        engine = make_engine()
        engine.quit()

        with pytest.raises(RuntimeError):
            engine.play(Move.place(4, 4))
        with pytest.raises(RuntimeError):
            engine.genmove()
        with pytest.raises(RuntimeError):
            engine.is_over()
        with pytest.raises(RuntimeError):
            engine.legal_moves()
        with pytest.raises(RuntimeError):
            engine.score()
        with pytest.raises(RuntimeError):
            engine.size()
        with pytest.raises(RuntimeError):
            engine.undo()
        with pytest.raises(RuntimeError):
            engine.clear_board()
        with pytest.raises(RuntimeError):
            engine.set_komi(5.5)
        with pytest.raises(RuntimeError):
            engine.send_command("name", [])


class TestGtpEngineFullGame:
    def test_short_game_with_genmove(self) -> None:
        """Play a short game where the engine generates all moves."""
        engine = make_engine(size=9)
        move_count = 0
        max_moves = 20
        while not engine.is_over() and move_count < max_moves:
            result = engine.genmove()
            if result is None:
                break
            move_count += 1
        assert move_count > 0
        engine.quit()

    def test_mixed_play_and_genmove(self) -> None:
        """Human plays a move, then engine responds."""
        engine = make_engine(size=9)
        engine.play(Move.place(4, 4))
        response = engine.genmove()
        assert response is not None
        legal = engine.legal_moves()
        place_moves = [m for m in legal if not m.is_pass()]
        assert len(place_moves) > 0
        engine.play(place_moves[0])
        response = engine.genmove()
        assert response is not None
        assert engine.turn() == BLACK
        engine.quit()
