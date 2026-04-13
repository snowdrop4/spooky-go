from spooky_go import GLOBAL_INPUT_FEATURES, SPATIAL_INPUT_PLANES, Game, Move


def get_plane_value(
    data: list[float],
    plane: int,
    row: int,
    col: int,
    height: int,
    width: int,
) -> float:
    return data[plane * height * width + row * width + col]


class TestConstants:
    def test_feature_counts(self) -> None:
        assert SPATIAL_INPUT_PLANES == 18
        assert GLOBAL_INPUT_FEATURES == 10


class TestGameEncoding:
    def test_encode_spatial_planes_shape(self) -> None:
        game = Game(9, 9)
        data, num_planes, height, width = game.encode_spatial_planes()

        assert num_planes == SPATIAL_INPUT_PLANES
        assert height == 9
        assert width == 9
        assert len(data) == num_planes * height * width

    def test_encode_global_state_features_shape(self) -> None:
        game = Game(9, 9)
        features = game.encode_global_state_features()
        assert len(features) == GLOBAL_INPUT_FEATURES

    def test_empty_board_has_on_board_plane_and_rules(self) -> None:
        game = Game(9, 9)
        spatial, _num_planes, height, width = game.encode_spatial_planes()
        global_features = game.encode_global_state_features()

        for row in range(height):
            for col in range(width):
                assert get_plane_value(spatial, 0, row, col, height, width) == 1.0
                assert get_plane_value(spatial, 1, row, col, height, width) == 0.0
                assert get_plane_value(spatial, 2, row, col, height, width) == 0.0

        assert global_features[5] == -0.5
        assert global_features[6] == 0.0
        assert global_features[7] == 1.0
        assert global_features[8] == 0.0

    def test_encode_spatial_planes_with_pieces(self) -> None:
        game = Game(9, 9)
        game.make_move(Move.place(4, 4))
        game.make_move(Move.place(3, 4))

        spatial, _num_planes, height, width = game.encode_spatial_planes()

        assert get_plane_value(spatial, 1, 4, 4, height, width) == 1.0
        assert get_plane_value(spatial, 2, 4, 3, height, width) == 1.0
        assert get_plane_value(spatial, 5, 4, 4, height, width) == 1.0
        assert get_plane_value(spatial, 5, 4, 3, height, width) == 1.0

    def test_last_move_plane_and_pass_history(self) -> None:
        game = Game.with_options(
            width=5,
            height=5,
            komi=7.5,
            min_moves_before_pass_possible=0,
            max_moves=100,
            superko=True,
        )
        game.make_move(Move.pass_move())
        game.make_move(Move.place(1, 2))

        spatial, _num_planes, height, width = game.encode_spatial_planes()
        global_features = game.encode_global_state_features()

        assert get_plane_value(spatial, 7, 2, 1, height, width) == 1.0
        assert global_features[1] == 1.0

    def test_encode_spatial_planes_different_sizes(self) -> None:
        game_9 = Game(9, 9)
        data_9, num_planes_9, height_9, width_9 = game_9.encode_spatial_planes()

        assert height_9 == 9
        assert width_9 == 9
        assert len(data_9) == num_planes_9 * 9 * 9

        game_19 = Game(19, 19)
        data_19, num_planes_19, height_19, width_19 = game_19.encode_spatial_planes()

        assert height_19 == 19
        assert width_19 == 19
        assert len(data_19) == num_planes_19 * 19 * 19


class TestActionDecoding:
    def test_decode_action_place(self) -> None:
        game = Game(9, 9)
        move = game.decode_action(0)

        assert move is not None
        assert move.col() == 0
        assert move.row() == 0

    def test_decode_action_pass(self) -> None:
        game = Game(9, 9)
        move = game.decode_action(81)

        assert move is not None
        assert move.is_pass()

    def test_decode_action_invalid(self) -> None:
        game = Game(9, 9)
        move = game.decode_action(100)
        assert move is None

    def test_total_actions(self) -> None:
        game_9 = Game(9, 9)
        assert game_9.total_actions() == 82

        game_19 = Game(19, 19)
        assert game_19.total_actions() == 362


class TestEncodingConsistency:
    def test_encoding_deterministic(self) -> None:
        game = Game(9, 9)
        game.make_move(Move.place(4, 4))
        game.make_move(Move.place(3, 3))

        spatial1 = game.encode_spatial_planes()
        spatial2 = game.encode_spatial_planes()
        global1 = game.encode_global_state_features()
        global2 = game.encode_global_state_features()

        assert spatial1 == spatial2
        assert global1 == global2

    def test_encoding_after_unmake(self) -> None:
        game = Game(9, 9)
        initial_spatial = game.encode_spatial_planes()
        initial_global = game.encode_global_state_features()

        game.make_move(Move.place(4, 4))
        game.make_move(Move.place(3, 3))
        game.unmake_move()
        game.unmake_move()

        final_spatial = game.encode_spatial_planes()
        final_global = game.encode_global_state_features()
        assert initial_spatial == final_spatial
        assert initial_global == final_global

    def test_different_positions_different_encoding(self) -> None:
        game1 = Game(9, 9)
        game1.make_move(Move.place(0, 0))

        game2 = Game(9, 9)
        game2.make_move(Move.place(1, 0))

        spatial1 = game1.encode_spatial_planes()
        spatial2 = game2.encode_spatial_planes()

        assert spatial1 != spatial2
