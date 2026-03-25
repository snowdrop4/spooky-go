pub const MIN_BOARD_DIM: u8 = 4;
pub const MAX_BOARD_DIM: u8 = 32;

pub const MIN_GTP_BOARD_SIZE: u8 = MIN_BOARD_DIM;
pub const MAX_GTP_BOARD_SIZE: u8 = 25;

#[inline]
pub const fn board_dimension_is_valid(dimension: u8) -> bool {
    dimension >= MIN_BOARD_DIM && dimension <= MAX_BOARD_DIM
}

#[track_caller]
pub fn assert_supported_board_dimensions(width: u8, height: u8) {
    assert!(
        board_dimension_is_valid(width),
        "Board width must be between {} and {}",
        MIN_BOARD_DIM,
        MAX_BOARD_DIM
    );
    assert!(
        board_dimension_is_valid(height),
        "Board height must be between {} and {}",
        MIN_BOARD_DIM,
        MAX_BOARD_DIM
    );
}
