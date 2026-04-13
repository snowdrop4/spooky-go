use crate::bitboard::{Bitboard, BoardGeometry};
use crate::game::Game;
use crate::player::Player;
use crate::position::Position;
use crate::r#move::Move;

pub const SPATIAL_INPUT_PLANES: usize = 18;
pub const GLOBAL_INPUT_FEATURES: usize = 10;
pub const RECENT_MOVE_COUNT: usize = 5;

const MAX_LIBERTY_BUCKET: u32 = 3;
const MAX_LADDER_DEPTH: usize = 16;
const LADDER_HISTORY_PLANES: usize = 3;

const PLANE_ON_BOARD: usize = 0;
const PLANE_OWN_STONES: usize = 1;
const PLANE_OPP_STONES: usize = 2;
const PLANE_ONE_LIBERTY: usize = 3;
const PLANE_TWO_LIBERTIES: usize = 4;
const PLANE_THREE_LIBERTIES: usize = 5;
const PLANE_KO_OR_SUPERKO: usize = 6;
const PLANE_LAST_MOVE_START: usize = 7;
const PLANE_LADDERABLE_START: usize = 12;
const PLANE_LADDER_CAPTURE: usize = 15;
const PLANE_PASS_ALIVE_SELF: usize = 16;
const PLANE_PASS_ALIVE_OPP: usize = 17;

const GLOBAL_PASS_HISTORY_START: usize = 0;
const GLOBAL_SELF_KOMI: usize = 5;
const GLOBAL_SIMPLE_KO: usize = 6;
const GLOBAL_POSITIONAL_SUPERKO: usize = 7;
const GLOBAL_SUICIDE_ALLOWED: usize = 8;
const GLOBAL_KOMI_PARITY: usize = 9;

#[derive(Clone, Copy)]
struct GroupInfo<const NW: usize> {
    owner: Player,
    stones: Bitboard<NW>,
    liberties: Bitboard<NW>,
}

#[derive(Clone)]
struct EmptyRegion<const NW: usize> {
    points: Bitboard<NW>,
    bordering_chains: Vec<usize>,
    vital_chains: Vec<usize>,
}

#[hotpath::measure]
pub fn encode_spatial_game_planes<const NW: usize>(
    game: &mut Game<NW>,
) -> (Vec<f32>, usize, usize, usize) {
    let width = game.width() as usize;
    let height = game.height() as usize;
    let board_size = width * height;
    let mut data = vec![0.0f32; SPATIAL_INPUT_PLANES * board_size];
    let geo = BoardGeometry::new(game.width(), game.height());
    let perspective = game.turn();

    fill_plane(&mut data, PLANE_ON_BOARD, board_size, 1.0);
    mark_stones_and_liberties(&mut data, game, &geo, board_size, perspective);
    mark_ko_and_superko(&mut data, game, &geo, board_size);
    mark_recent_move_planes(&mut data, game, board_size);
    mark_ladder_planes(&mut data, game, board_size);

    let self_pass_alive = pass_alive_area(game, &geo, perspective);
    mark_bitboard(&mut data, PLANE_PASS_ALIVE_SELF, self_pass_alive, board_size);
    let opponent_pass_alive = pass_alive_area(game, &geo, perspective.opposite());
    mark_bitboard(
        &mut data,
        PLANE_PASS_ALIVE_OPP,
        opponent_pass_alive,
        board_size,
    );

    (data, SPATIAL_INPUT_PLANES, height, width)
}

#[hotpath::measure]
pub fn encode_global_state_features<const NW: usize>(game: &mut Game<NW>) -> Vec<f32> {
    let perspective = game.turn();
    let self_komi = komi_from_perspective(game, perspective);
    let parity = komi_parity_feature(game, self_komi);
    let history = game.move_history();
    let mut features = vec![0.0f32; GLOBAL_INPUT_FEATURES];

    for (offset, move_) in history.iter().rev().take(RECENT_MOVE_COUNT).enumerate() {
        if move_.is_pass() {
            features[GLOBAL_PASS_HISTORY_START + offset] = 1.0;
        }
    }

    features[GLOBAL_SELF_KOMI] = self_komi / 15.0;
    features[GLOBAL_SIMPLE_KO] = if game.superko() { 0.0 } else { 1.0 };
    features[GLOBAL_POSITIONAL_SUPERKO] = if game.superko() { 1.0 } else { 0.0 };
    features[GLOBAL_SUICIDE_ALLOWED] = 0.0;
    features[GLOBAL_KOMI_PARITY] = parity;
    features
}

fn komi_from_perspective<const NW: usize>(game: &Game<NW>, perspective: Player) -> f32 {
    match perspective {
        Player::Black => -game.komi(),
        Player::White => game.komi(),
    }
}

fn komi_parity_feature<const NW: usize>(game: &Game<NW>, self_komi: f32) -> f32 {
    let board_area = game.width() as i32 * game.height() as i32;
    let komi_half_points = (self_komi * 2.0).round() as i32;
    (board_area + komi_half_points).rem_euclid(2) as f32
}

fn fill_plane(data: &mut [f32], plane: usize, board_size: usize, value: f32) {
    let start = plane * board_size;
    let end = start + board_size;
    data[start..end].fill(value);
}

fn mark_bitboard<const NW: usize>(
    data: &mut [f32],
    plane: usize,
    bits: Bitboard<NW>,
    board_size: usize,
) {
    let base = plane * board_size;
    for idx in bits.iter_ones() {
        data[base + idx] = 1.0;
    }
}

fn mark_position(data: &mut [f32], plane: usize, pos: Position, board_size: usize, width: u8) {
    let idx = pos.to_index(width);
    data[plane * board_size + idx] = 1.0;
}

fn mark_stones_and_liberties<const NW: usize>(
    data: &mut [f32],
    game: &Game<NW>,
    geo: &BoardGeometry<NW>,
    board_size: usize,
    perspective: Player,
) {
    for group in collect_groups(game, geo, Player::Black)
        .into_iter()
        .chain(collect_groups(game, geo, Player::White))
    {
        let color_plane = if group.owner == perspective {
            PLANE_OWN_STONES
        } else {
            PLANE_OPP_STONES
        };
        mark_bitboard(data, color_plane, group.stones, board_size);
        mark_bitboard(
            data,
            liberty_plane(group.liberties.count()),
            group.stones,
            board_size,
        );
    }
}

fn liberty_plane(liberties: u32) -> usize {
    match liberties.min(MAX_LIBERTY_BUCKET) {
        1 => PLANE_ONE_LIBERTY,
        2 => PLANE_TWO_LIBERTIES,
        _ => PLANE_THREE_LIBERTIES,
    }
}

fn collect_groups<const NW: usize>(
    game: &Game<NW>,
    geo: &BoardGeometry<NW>,
    player: Player,
) -> Vec<GroupInfo<NW>> {
    let board = game.board();
    let mut groups = Vec::new();
    let mut remaining = board.stones_for(player);
    let empty = board.empty_squares(geo.board_mask);

    while let Some(idx) = remaining.lowest_bit_index() {
        let seed = Bitboard::single(idx);
        let stones = geo.flood_fill(seed, board.stones_for(player));
        remaining = remaining.andnot(stones);
        let liberties = geo.neighbors(&stones) & empty;
        groups.push(GroupInfo {
            owner: player,
            stones,
            liberties,
        });
    }

    groups
}

fn group_at<const NW: usize>(
    game: &Game<NW>,
    geo: &BoardGeometry<NW>,
    pos: Position,
) -> Option<GroupInfo<NW>> {
    let board = game.board();
    let owner = board.get_piece(&pos)?;
    let stones = geo.flood_fill(Bitboard::single(pos.to_index(game.width())), board.stones_for(owner));
    let liberties = geo.neighbors(&stones) & board.empty_squares(geo.board_mask);
    Some(GroupInfo {
        owner,
        stones,
        liberties,
    })
}

fn mark_ko_and_superko<const NW: usize>(
    data: &mut [f32],
    game: &Game<NW>,
    geo: &BoardGeometry<NW>,
    board_size: usize,
) {
    let ko_idx = game.ko_point().map(|pos| pos.to_index(game.width()));
    if let Some(ko_point) = game.ko_point() {
        mark_position(data, PLANE_KO_OR_SUPERKO, ko_point, board_size, game.width());
    }

    if !game.superko() {
        return;
    }

    let simple_game = clone_with_superko(game, false);
    let board = game.board();
    let empty = board.empty_squares(geo.board_mask);

    for idx in empty.iter_ones() {
        if ko_idx.is_some_and(|ko_idx| ko_idx == idx) {
            continue;
        }
        let pos = Position::from_index(idx, game.width());
        let move_ = Move::place(pos.col, pos.row);
        if !game.is_legal_move(&move_) && simple_game.is_legal_move(&move_) {
            mark_position(data, PLANE_KO_OR_SUPERKO, pos, board_size, game.width());
        }
    }
}

fn clone_with_superko<const NW: usize>(game: &Game<NW>, superko: bool) -> Game<NW> {
    let mut clone = Game::with_options(
        game.width(),
        game.height(),
        game.komi(),
        game.min_moves_before_pass_possible(),
        game.max_moves(),
        superko,
    );
    let board = game.board();
    let black_positions = positions_from_bitboard(board.black_stones(), game.width());
    let white_positions = positions_from_bitboard(board.white_stones(), game.width());
    clone
        .set_setup_position(&black_positions, &white_positions, game.turn())
        .expect("clone_with_superko: setup position must be valid");
    clone
}

fn positions_from_bitboard<const NW: usize>(bits: Bitboard<NW>, width: u8) -> Vec<Position> {
    bits.iter_ones()
        .map(|idx| Position::from_index(idx, width))
        .collect()
}

fn mark_recent_move_planes<const NW: usize>(data: &mut [f32], game: &Game<NW>, board_size: usize) {
    for (offset, move_) in game.move_history().iter().rev().take(RECENT_MOVE_COUNT).enumerate() {
        if let Some(pos) = move_.position() {
            mark_position(
                data,
                PLANE_LAST_MOVE_START + offset,
                pos,
                board_size,
                game.width(),
            );
        }
    }
}

fn mark_ladder_planes<const NW: usize>(data: &mut [f32], game: &mut Game<NW>, board_size: usize) {
    let history = game.move_history();
    let steps_back = (LADDER_HISTORY_PLANES - 1).min(history.len());
    let moves_to_replay = history[(history.len() - steps_back)..].to_vec();

    for history_offset in 0..=steps_back {
        let geo = BoardGeometry::new(game.width(), game.height());
        let ladderable = collect_ladderable_groups(game, &geo);
        let ladder_plane = PLANE_LADDERABLE_START + history_offset;
        let mut ladder_capture_points: Bitboard<NW> = Bitboard::empty();

        for group in &ladderable {
            mark_bitboard(data, ladder_plane, group.stones, board_size);
            if history_offset != 0 {
                continue;
            }

            for liberty_idx in group.liberties.iter_ones() {
                let liberty = Position::from_index(liberty_idx, game.width());
                if is_ladder_capture(
                    game,
                    group,
                    Move::place(liberty.col, liberty.row),
                    MAX_LADDER_DEPTH,
                ) {
                    ladder_capture_points.set(liberty_idx);
                }
            }
        }

        if history_offset == 0 {
            mark_bitboard(
                data,
                PLANE_LADDER_CAPTURE,
                ladder_capture_points,
                board_size,
            );
        }

        if history_offset < steps_back {
            let did_unmake = game.unmake_move();
            debug_assert!(did_unmake);
        }
    }

    for move_ in &moves_to_replay {
        let replayed = game.make_move(move_);
        debug_assert!(replayed);
    }
}

fn collect_ladderable_groups<const NW: usize>(
    game: &Game<NW>,
    geo: &BoardGeometry<NW>,
) -> Vec<GroupInfo<NW>> {
    let target_player = game.turn().opposite();
    collect_groups(game, geo, target_player)
        .into_iter()
        .filter(|group| {
            let liberty_count = group.liberties.count();
            liberty_count == 1 || liberty_count == 2
        })
        .filter(|group| {
            group.liberties.iter_ones().any(|idx| {
                let liberty = Position::from_index(idx, game.width());
                is_ladder_capture(
                    game,
                    group,
                    Move::place(liberty.col, liberty.row),
                    MAX_LADDER_DEPTH,
                )
            })
        })
        .collect()
}

fn is_ladder_capture<const NW: usize>(
    game: &Game<NW>,
    target_group: &GroupInfo<NW>,
    capture_move: Move,
    depth: usize,
) -> bool {
    if depth == 0 || target_group.owner == game.turn() {
        return false;
    }
    if !game.is_legal_move(&capture_move) {
        return false;
    }

    let mut next = game.clone();
    if !next.make_move(&capture_move) {
        return false;
    }

    let next_geo = BoardGeometry::new(next.width(), next.height());
    let Some(group_after) = surviving_target_group(&next, &next_geo, target_group) else {
        return true;
    };
    if group_after.liberties.count() >= 3 {
        return false;
    }

    !defender_can_escape(&next, &next_geo, &group_after, depth - 1)
}

fn defender_can_escape<const NW: usize>(
    game: &Game<NW>,
    geo: &BoardGeometry<NW>,
    target_group: &GroupInfo<NW>,
    depth: usize,
) -> bool {
    if depth == 0 {
        return true;
    }
    if target_group.owner != game.turn() {
        return false;
    }
    if target_group.liberties.count() >= 3 {
        return true;
    }

    for move_ in escape_candidates(game, geo, target_group) {
        let mut next = game.clone();
        if !next.make_move(&move_) {
            continue;
        }
        let next_geo = BoardGeometry::new(next.width(), next.height());
        let Some(group_after) = surviving_target_group(&next, &next_geo, target_group) else {
            continue;
        };
        if group_after.liberties.count() >= 3 {
            return true;
        }

        let attacker_can_continue = group_after.liberties.iter_ones().any(|idx| {
            let liberty = Position::from_index(idx, next.width());
            is_ladder_capture(
                &next,
                &group_after,
                Move::place(liberty.col, liberty.row),
                depth - 1,
            )
        });
        if !attacker_can_continue {
            return true;
        }
    }

    false
}

fn escape_candidates<const NW: usize>(
    game: &Game<NW>,
    geo: &BoardGeometry<NW>,
    target_group: &GroupInfo<NW>,
) -> Vec<Move> {
    let board = game.board();
    let opponent = target_group.owner.opposite();
    let mut candidate_bits = target_group.liberties;
    let mut adjacent_opponent_groups = geo.neighbors(&target_group.stones) & board.stones_for(opponent);
    let empty = board.empty_squares(geo.board_mask);

    while let Some(idx) = adjacent_opponent_groups.lowest_bit_index() {
        let stones = geo.flood_fill(Bitboard::single(idx), board.stones_for(opponent));
        adjacent_opponent_groups = adjacent_opponent_groups.andnot(stones);
        let liberties = geo.neighbors(&stones) & empty;
        if liberties.count() == 1 {
            candidate_bits |= liberties;
        }
    }

    candidate_bits
        .iter_ones()
        .filter_map(|idx| {
            let pos = Position::from_index(idx, game.width());
            let move_ = Move::place(pos.col, pos.row);
            if game.is_legal_move(&move_) {
                Some(move_)
            } else {
                None
            }
        })
        .collect()
}

fn surviving_target_group<const NW: usize>(
    game: &Game<NW>,
    geo: &BoardGeometry<NW>,
    previous_group: &GroupInfo<NW>,
) -> Option<GroupInfo<NW>> {
    let survivors = previous_group.stones & game.board().stones_for(previous_group.owner);
    let repr_idx = survivors.lowest_bit_index()?;
    group_at(game, geo, Position::from_index(repr_idx, game.width()))
}

fn pass_alive_area<const NW: usize>(
    game: &Game<NW>,
    geo: &BoardGeometry<NW>,
    player: Player,
) -> Bitboard<NW> {
    let chains = collect_groups(game, geo, player);
    if chains.is_empty() {
        return Bitboard::empty();
    }

    let regions = candidate_eye_regions(game, geo, player, &chains);
    if regions.is_empty() {
        return Bitboard::empty();
    }

    let mut active_chains = vec![true; chains.len()];
    let mut active_regions = vec![true; regions.len()];

    loop {
        let mut changed = false;

        for (region_index, region) in regions.iter().enumerate() {
            if !active_regions[region_index] {
                continue;
            }
            if region
                .bordering_chains
                .iter()
                .any(|chain_index| !active_chains[*chain_index])
            {
                active_regions[region_index] = false;
                changed = true;
            }
        }

        for chain_index in 0..chains.len() {
            if !active_chains[chain_index] {
                continue;
            }
            let vital_region_count = regions
                .iter()
                .enumerate()
                .filter(|(region_index, region)| {
                    active_regions[*region_index] && region.vital_chains.contains(&chain_index)
                })
                .count();
            if vital_region_count < 2 {
                active_chains[chain_index] = false;
                changed = true;
            }
        }

        if !changed {
            break;
        }
    }

    let mut pass_alive = Bitboard::empty();
    for (chain_index, chain) in chains.iter().enumerate() {
        if active_chains[chain_index] {
            pass_alive |= chain.stones;
        }
    }
    for (region_index, region) in regions.iter().enumerate() {
        if active_regions[region_index] {
            pass_alive |= region.points;
        }
    }
    pass_alive
}

fn candidate_eye_regions<const NW: usize>(
    game: &Game<NW>,
    geo: &BoardGeometry<NW>,
    player: Player,
    chains: &[GroupInfo<NW>],
) -> Vec<EmptyRegion<NW>> {
    let board = game.board();
    let empty = board.empty_squares(geo.board_mask);
    let own = board.stones_for(player);
    let opp = board.stones_for(player.opposite());
    let mut regions = Vec::new();
    let mut remaining = empty;

    while let Some(idx) = remaining.lowest_bit_index() {
        let points = geo.flood_fill(Bitboard::single(idx), empty);
        remaining = remaining.andnot(points);
        let border = geo.neighbors(&points);
        if (border & own).is_empty() || (border & opp).is_nonzero() {
            continue;
        }

        let bordering_chains: Vec<usize> = chains
            .iter()
            .enumerate()
            .filter_map(|(chain_index, chain)| {
                if (geo.neighbors(&chain.stones) & points).is_nonzero() {
                    Some(chain_index)
                } else {
                    None
                }
            })
            .collect();
        if bordering_chains.is_empty() {
            continue;
        }

        let vital_chains: Vec<usize> = chains
            .iter()
            .enumerate()
            .filter_map(|(chain_index, chain)| {
                if points.andnot(chain.liberties).is_empty() {
                    Some(chain_index)
                } else {
                    None
                }
            })
            .collect();
        if vital_chains.is_empty() {
            continue;
        }

        regions.push(EmptyRegion {
            points,
            bordering_chains,
            vital_chains,
        });
    }

    regions
}

#[hotpath::measure]
pub fn encode_move(move_: &Move, board_width: u8, board_height: u8) -> usize {
    match move_ {
        Move::Place { col, row } => *row as usize * board_width as usize + *col as usize,
        Move::Pass => board_width as usize * board_height as usize,
    }
}

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

    fn plane_value(
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
    fn test_spatial_and_global_feature_counts() {
        assert_eq!(SPATIAL_INPUT_PLANES, 18);
        assert_eq!(GLOBAL_INPUT_FEATURES, 10);
    }

    #[test]
    fn test_encode_empty_board() {
        let mut game = Game::<{ nw_for_board(9, 9) }>::new(9, 9);
        let (data, planes, height, width) = encode_spatial_game_planes(&mut game);
        let global = encode_global_state_features(&mut game);

        assert_eq!(planes, SPATIAL_INPUT_PLANES);
        assert_eq!(height, 9);
        assert_eq!(width, 9);
        assert_eq!(data.len(), planes * height * width);
        assert_eq!(global.len(), GLOBAL_INPUT_FEATURES);

        for row in 0..height {
            for col in 0..width {
                assert_eq!(plane_value(&data, PLANE_ON_BOARD, row, col, height, width), 1.0);
                assert_eq!(plane_value(&data, PLANE_OWN_STONES, row, col, height, width), 0.0);
                assert_eq!(plane_value(&data, PLANE_OPP_STONES, row, col, height, width), 0.0);
            }
        }

        assert_eq!(global[GLOBAL_SELF_KOMI], -0.5);
        assert_eq!(global[GLOBAL_SIMPLE_KO], 0.0);
        assert_eq!(global[GLOBAL_POSITIONAL_SUPERKO], 1.0);
        assert_eq!(global[GLOBAL_SUICIDE_ALLOWED], 0.0);
    }

    #[test]
    fn test_encode_stones_and_liberties() {
        let mut game = Game::<{ nw_for_board(9, 9) }>::new(9, 9);
        assert!(game.make_move(&Move::place(4, 4)));
        assert!(game.make_move(&Move::place(3, 4)));

        let (data, _planes, height, width) = encode_spatial_game_planes(&mut game);

        assert_eq!(plane_value(&data, PLANE_OWN_STONES, 4, 4, height, width), 1.0);
        assert_eq!(plane_value(&data, PLANE_OPP_STONES, 4, 3, height, width), 1.0);
        assert_eq!(plane_value(&data, PLANE_THREE_LIBERTIES, 4, 4, height, width), 1.0);
        assert_eq!(plane_value(&data, PLANE_THREE_LIBERTIES, 4, 3, height, width), 1.0);
    }

    #[test]
    fn test_recent_move_planes_and_pass_history() {
        let mut game = Game::<{ nw_for_board(5, 5) }>::with_options(5, 5, 7.5, 0, 100, true);
        assert!(game.make_move(&Move::pass()));
        assert!(game.make_move(&Move::place(1, 2)));

        let (data, _planes, height, width) = encode_spatial_game_planes(&mut game);
        let global = encode_global_state_features(&mut game);

        assert_eq!(plane_value(&data, PLANE_LAST_MOVE_START, 2, 1, height, width), 1.0);
        assert_eq!(global[GLOBAL_PASS_HISTORY_START + 1], 1.0);
    }

    #[test]
    fn test_ko_plane_marks_immediate_recapture() {
        let mut game = Game::<{ nw_for_board(5, 5) }>::new(5, 5);
        assert!(game.make_move(&Move::place(1, 0)));
        assert!(game.make_move(&Move::place(2, 0)));
        assert!(game.make_move(&Move::place(0, 1)));
        assert!(game.make_move(&Move::place(1, 1)));
        assert!(game.make_move(&Move::place(1, 2)));
        assert!(game.make_move(&Move::place(2, 2)));
        assert!(game.make_move(&Move::place(4, 4)));
        assert!(game.make_move(&Move::place(3, 1)));
        assert!(game.make_move(&Move::place(2, 1)));

        let (data, _planes, height, width) = encode_spatial_game_planes(&mut game);
        assert_eq!(plane_value(&data, PLANE_KO_OR_SUPERKO, 1, 1, height, width), 1.0);
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
        assert_eq!(decode_move(encoded_pass, width, height), Some(pass));
    }

    #[test]
    fn test_total_actions() {
        assert_eq!(total_actions(9, 9), 82);
        assert_eq!(total_actions(19, 19), 362);
        assert_eq!(total_actions(5, 5), 26);
    }

    #[test]
    fn test_encoding_deterministic() {
        let mut game = Game::<{ nw_for_board(9, 9) }>::new(9, 9);
        assert!(game.make_move(&Move::place(4, 4)));
        assert!(game.make_move(&Move::place(3, 3)));

        let spatial1 = encode_spatial_game_planes(&mut game);
        let spatial2 = encode_spatial_game_planes(&mut game);
        let global1 = encode_global_state_features(&mut game);
        let global2 = encode_global_state_features(&mut game);

        assert_eq!(spatial1, spatial2);
        assert_eq!(global1, global2);
    }
}
