from spooky_go import BLACK, WHITE, Board


class TestBoardCreation:
    def test_new_board(self) -> None:
        board = Board(9, 9)
        assert board.width() == 9
        assert board.height() == 9

    def test_standard_board(self) -> None:
        board = Board.standard()
        assert board.width() == 19
        assert board.height() == 19

    def test_custom_size_board(self) -> None:
        board = Board(5, 7)
        assert board.width() == 5
        assert board.height() == 7


class TestBoardPieces:
    def test_empty_board(self) -> None:
        board = Board(9, 9)
        for row in range(9):
            for col in range(9):
                assert board.get_piece(col, row) is None

    def test_set_and_get_piece(self) -> None:
        board = Board(9, 9)
        board.set_piece(4, 4, BLACK)
        assert board.get_piece(4, 4) == BLACK

        board.set_piece(3, 3, WHITE)
        assert board.get_piece(3, 3) == WHITE

    def test_set_piece_to_none(self) -> None:
        board = Board(9, 9)
        board.set_piece(4, 4, BLACK)
        assert board.get_piece(4, 4) == BLACK

        board.set_piece(4, 4, None)
        assert board.get_piece(4, 4) is None

    def test_clear_board(self) -> None:
        board = Board(9, 9)
        board.set_piece(0, 0, BLACK)
        board.set_piece(1, 1, WHITE)
        board.set_piece(2, 2, BLACK)

        board.clear()

        for row in range(9):
            for col in range(9):
                assert board.get_piece(col, row) is None
