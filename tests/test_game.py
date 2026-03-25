from spooky_go import BLACK, WHITE, Game, Move


class TestGameCreation:
    def test_new_game(self) -> None:
        game = Game(9, 9)
        assert game.width() == 9
        assert game.height() == 9

    def test_standard_game(self) -> None:
        game = Game.standard()
        assert game.width() == 19
        assert game.height() == 19

    def test_custom_size_game(self) -> None:
        game = Game(5, 7)
        assert game.width() == 5
        assert game.height() == 7


class TestGameState:
    def test_initial_turn(self) -> None:
        game = Game(9, 9)
        assert game.turn() == BLACK

    def test_turn_alternates(self) -> None:
        game = Game(9, 9)
        assert game.turn() == BLACK

        game.make_move(Move.place(4, 4))
        assert game.turn() == WHITE

        game.make_move(Move.place(3, 3))
        assert game.turn() == BLACK

    def test_initial_not_over(self) -> None:
        game = Game(9, 9)
        assert not game.is_over()
        assert game.outcome() is None

    def test_board_access(self) -> None:
        game = Game(9, 9)
        board = game.board()
        assert board.width() == 9
        assert board.height() == 9


class TestGameMoves:
    def test_make_move(self) -> None:
        game = Game(9, 9)
        move = Move.place(4, 4)

        assert game.is_legal_move(move)
        result = game.make_move(move)
        assert result is True

    def test_make_illegal_move_occupied(self) -> None:
        game = Game(9, 9)
        move = Move.place(4, 4)
        game.make_move(move)

        same_move = Move.place(4, 4)
        assert not game.is_legal_move(same_move)
        result = game.make_move(same_move)
        assert result is False

    def test_legal_moves_initial(self) -> None:
        game = Game(9, 9)
        moves = game.legal_moves()
        # 81 board positions, pass not yet legal (min_moves_before_pass_possible)
        assert len(moves) == 81

    def test_legal_moves_after_move(self) -> None:
        game = Game(9, 9)
        game.make_move(Move.place(4, 4))
        moves = game.legal_moves()
        # 80 board positions, pass not yet legal
        assert len(moves) == 80

    def test_unmake_move(self) -> None:
        game = Game(9, 9)
        move = Move.place(4, 4)
        game.make_move(move)

        assert game.turn() == WHITE
        result = game.unmake_move()
        assert result is True
        assert game.turn() == BLACK

    def test_unmake_on_empty_history(self) -> None:
        game = Game(9, 9)
        result = game.unmake_move()
        assert result is False


class TestPassMove:
    def test_pass_is_legal(self) -> None:
        game = Game.with_options(9, 9, 7.5, 0, 1000, False)
        assert game.is_legal_move(Move.pass_move())

    def test_pass_not_legal_before_min_moves(self) -> None:
        game = Game(9, 9)
        assert not game.is_legal_move(Move.pass_move())

    def test_pass_changes_turn(self) -> None:
        game = Game.with_options(9, 9, 7.5, 0, 1000, False)
        assert game.turn() == BLACK

        game.make_move(Move.pass_move())
        assert game.turn() == WHITE

    def test_two_passes_ends_game(self) -> None:
        game = Game.with_options(9, 9, 7.5, 0, 1000, False)

        game.make_move(Move.pass_move())
        assert not game.is_over()

        game.make_move(Move.pass_move())
        assert game.is_over()
        assert game.outcome() is not None

    def test_two_passes_requires_min_moves(self) -> None:
        game = Game(9, 9)
        assert game.min_moves_before_pass_possible() == 40  # 81 / 2

        # Pass not legal before min moves
        assert not game.is_legal_move(Move.pass_move())


class TestCaptures:
    def test_simple_capture_corner(self) -> None:
        game = Game(9, 9)

        # Black surrounds White stone at (0, 0)
        game.make_move(Move.place(1, 0))  # B
        game.make_move(Move.place(0, 0))  # W - will be captured
        game.make_move(Move.place(0, 1))  # B - captures

        board = game.board()
        assert board.get_piece(0, 0) is None  # Captured

    def test_capture_restores_on_unmake(self) -> None:
        game = Game(9, 9)

        game.make_move(Move.place(1, 0))  # B
        game.make_move(Move.place(0, 0))  # W
        game.make_move(Move.place(0, 1))  # B - captures

        assert game.board().get_piece(0, 0) is None

        game.unmake_move()
        assert game.board().get_piece(0, 0) == WHITE


class TestKoRule:
    def test_ko_prevents_immediate_recapture(self) -> None:
        game = Game(5, 5)

        # Build ko shape
        game.make_move(Move.place(1, 0))  # B
        game.make_move(Move.place(2, 0))  # W
        game.make_move(Move.place(0, 1))  # B
        game.make_move(Move.place(1, 1))  # W - will be captured
        game.make_move(Move.place(1, 2))  # B
        game.make_move(Move.place(2, 2))  # W
        game.make_move(Move.place(4, 4))  # B (elsewhere)
        game.make_move(Move.place(3, 1))  # W

        # Black captures at (2, 1)
        game.make_move(Move.place(2, 1))

        # Ko point should be set
        ko = game.ko_point()
        assert ko is not None
        assert ko == (1, 1)

        # White cannot immediately recapture
        recapture = Move.place(1, 1)
        assert not game.is_legal_move(recapture)


class TestSuicide:
    def test_suicide_prevented(self) -> None:
        game = Game(5, 5)

        # Black surrounds corner at (0, 0)
        game.make_move(Move.place(1, 0))  # B
        game.make_move(Move.place(4, 4))  # W (elsewhere)
        game.make_move(Move.place(0, 1))  # B

        # White playing (0, 0) would have 0 liberties and captures nothing
        assert not game.is_legal_move(Move.place(0, 0))

    def test_suicide_allowed_if_captures(self) -> None:
        game = Game(5, 5)

        # Set up position where Black can play a stone surrounded by
        # White on all 4 sides, but it captures W(1,1)
        game.make_move(Move.place(1, 0))  # B
        game.make_move(Move.place(2, 0))  # W
        game.make_move(Move.place(0, 1))  # B
        game.make_move(Move.place(1, 1))  # W
        game.make_move(Move.place(1, 2))  # B
        game.make_move(Move.place(2, 2))  # W
        game.make_move(Move.place(4, 4))  # B (elsewhere)
        game.make_move(Move.place(3, 1))  # W

        #   0 1 2 3 4
        # 0 . B W . .
        # 1 B W . W .
        # 2 . B W . .
        #
        # Black plays (2,1): 0 immediate liberties, but captures W(1,1)
        assert game.is_legal_move(Move.place(2, 1))
        game.make_move(Move.place(2, 1))

        assert game.board().get_piece(1, 1) is None  # W(1,1) captured
        assert game.board().get_piece(2, 1) == BLACK  # B(2,1) survives


class TestGameClone:
    def test_clone(self) -> None:
        game = Game(9, 9)
        game.make_move(Move.place(4, 4))
        game.make_move(Move.place(3, 3))

        cloned = game.clone()

        assert cloned.turn() == game.turn()
        assert cloned.is_over() == game.is_over()
        assert cloned.width() == game.width()
        assert cloned.height() == game.height()

    def test_clone_independence(self) -> None:
        game = Game(9, 9)
        game.make_move(Move.place(4, 4))

        cloned = game.clone()
        cloned.make_move(Move.place(3, 3))

        # Original should not be affected
        assert game.turn() == WHITE
        assert cloned.turn() == BLACK


class TestGameHash:
    def test_same_state_same_hash(self) -> None:
        game1 = Game(9, 9)
        game2 = Game(9, 9)

        assert hash(game1) == hash(game2)

    def test_different_state_different_hash(self) -> None:
        game1 = Game(9, 9)
        game2 = Game(9, 9)
        game2.make_move(Move.place(4, 4))

        assert hash(game1) != hash(game2)
