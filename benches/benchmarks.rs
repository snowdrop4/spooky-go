use criterion::{criterion_group, criterion_main, Criterion};
use rand::prelude::IndexedRandom;
use rand::rngs::SmallRng;
use rand::SeedableRng;
use spooky_go::bitboard::nw_for_board;
use spooky_go::encode::{encode_global_state_features, encode_spatial_game_planes};
use spooky_go::game::Game;
use std::hint::black_box;

/// Play ~20 random moves on a fresh game to create a realistic mid-game position.
/// Uses a fixed seed for reproducibility across benchmark runs.
fn setup_midgame<const NW: usize>(width: u8, height: u8) -> Game<NW> {
    let mut game = Game::new(width, height);
    let mut rng = SmallRng::seed_from_u64(42);
    for _ in 0..20 {
        let moves = game.legal_moves();
        // Filter to placement moves only so the board fills up
        let placements: Vec<_> = moves.iter().filter(|m| !m.is_pass()).copied().collect();
        if placements.is_empty() {
            break;
        }
        let mv = placements
            .choose(&mut rng)
            .expect("setup_midgame: placement moves must not be empty");
        game.make_move(mv);
    }
    game
}

// ---------------------------------------------------------------------------
// Microbenchmarks
// ---------------------------------------------------------------------------

fn bench_legal_moves_9x9(c: &mut Criterion) {
    let game = setup_midgame::<{ nw_for_board(9, 9) }>(9, 9);
    c.bench_function("legal_moves_9x9", |b| {
        b.iter(|| black_box(game.legal_moves()))
    });
}

fn bench_legal_moves_19x19(c: &mut Criterion) {
    let game = setup_midgame::<{ nw_for_board(19, 19) }>(19, 19);
    c.bench_function("legal_moves_19x19", |b| {
        b.iter(|| black_box(game.legal_moves()))
    });
}

fn bench_make_move(c: &mut Criterion) {
    let game = setup_midgame::<{ nw_for_board(9, 9) }>(9, 9);
    // Pick the first legal placement move
    let moves = game.legal_moves();
    let mv = moves
        .iter()
        .find(|m| !m.is_pass())
        .copied()
        .expect("bench_make_move: non-pass move must exist");
    c.bench_function("make_move", |b| {
        b.iter_batched(
            || game.clone(),
            |mut g| {
                black_box(g.make_move(&mv));
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

fn bench_make_unmake(c: &mut Criterion) {
    let game = setup_midgame::<{ nw_for_board(9, 9) }>(9, 9);
    let moves = game.legal_moves();
    let mv = moves
        .iter()
        .find(|m| !m.is_pass())
        .copied()
        .expect("bench_make_unmake: non-pass move must exist");
    c.bench_function("make_unmake", |b| {
        b.iter_batched(
            || game.clone(),
            |mut g| {
                g.make_move(&mv);
                black_box(g.unmake_move());
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

fn bench_encode_state_9x9(c: &mut Criterion) {
    let game = setup_midgame::<{ nw_for_board(9, 9) }>(9, 9);
    c.bench_function("encode_state_9x9", |b| {
        b.iter_batched(
            || game.clone(),
            |mut g| {
                black_box((
                    encode_spatial_game_planes(&mut g),
                    encode_global_state_features(&mut g),
                ))
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

fn bench_encode_state_19x19(c: &mut Criterion) {
    let game = setup_midgame::<{ nw_for_board(19, 19) }>(19, 19);
    c.bench_function("encode_state_19x19", |b| {
        b.iter_batched(
            || game.clone(),
            |mut g| {
                black_box((
                    encode_spatial_game_planes(&mut g),
                    encode_global_state_features(&mut g),
                ))
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

fn bench_outcome(c: &mut Criterion) {
    let game = setup_midgame::<{ nw_for_board(9, 9) }>(9, 9);
    c.bench_function("outcome", |b| b.iter(|| black_box(game.outcome())));
}

// ---------------------------------------------------------------------------
// Integration benchmarks
// ---------------------------------------------------------------------------

fn bench_random_playout_9x9(c: &mut Criterion) {
    c.bench_function("random_playout_9x9", |b| {
        b.iter(|| {
            let mut game = Game::<{ nw_for_board(9, 9) }>::new(9, 9);
            let mut rng = SmallRng::seed_from_u64(123);
            while !game.is_over() {
                let moves = game.legal_moves();
                let mv = moves
                    .choose(&mut rng)
                    .expect("bench_random_playout_9x9: legal moves must not be empty");
                game.make_move(mv);
            }
            black_box(game.outcome())
        })
    });
}

fn bench_random_playout_19x19(c: &mut Criterion) {
    c.bench_function("random_playout_19x19", |b| {
        b.iter(|| {
            let mut game = Game::<{ nw_for_board(19, 19) }>::new(19, 19);
            let mut rng = SmallRng::seed_from_u64(123);
            while !game.is_over() {
                let moves = game.legal_moves();
                let mv = moves
                    .choose(&mut rng)
                    .expect("bench_random_playout_19x19: legal moves must not be empty");
                game.make_move(mv);
            }
            black_box(game.outcome())
        })
    });
}

fn bench_self_play_step(c: &mut Criterion) {
    let game = setup_midgame::<{ nw_for_board(9, 9) }>(9, 9);
    c.bench_function("self_play_step", |b| {
        b.iter_batched(
            || game.clone(),
            |mut g| {
                let moves = g.legal_moves();
                // Self-play consumes both tensors, so benchmark the split encoder
                // the same way the caller actually uses it.
                let _spatial = encode_spatial_game_planes(&mut g);
                let _global = encode_global_state_features(&mut g);
                // Pick the first legal placement (simulating a policy choice)
                let mv = moves
                    .iter()
                    .find(|m| !m.is_pass())
                    .copied()
                    .expect("bench_self_play_step: non-pass move must exist");
                g.make_move(&mv);
                black_box(&g);
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

criterion_group!(
    name = benches;
    config = Criterion::default().sample_size(100_000);
    targets =
        bench_legal_moves_9x9,
        bench_legal_moves_19x19,
        bench_make_move,
        bench_make_unmake,
        bench_encode_state_9x9,
        bench_encode_state_19x19,
        bench_outcome,
        bench_self_play_step,
);
criterion_group!(
    name = playouts;
    config = Criterion::default().sample_size(10_000);
    targets =
        bench_random_playout_9x9,
        bench_random_playout_19x19,
);
criterion_main!(benches, playouts);
