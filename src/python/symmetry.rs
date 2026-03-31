use numpy::ndarray::{s, Array1, Array2, Array3, Array4};
use numpy::{
    IntoPyArray, PyArray1, PyArray2, PyArray3, PyArray4, PyReadonlyArray1, PyReadonlyArray2,
    PyReadonlyArray3, PyReadonlyArray4,
};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

#[derive(Clone, Copy)]
struct SpatialTransform {
    rotation: usize,
    flip: bool,
}

fn validate_batch_shapes(
    states_shape: &[usize],
    policies_shape: &[usize],
    values_shape: &[usize],
    opponent_policies_shape: &[usize],
    opponent_policy_masks_shape: &[usize],
    ownership_shape: &[usize],
    score_differences_shape: &[usize],
) -> PyResult<(usize, usize, usize, usize, usize)> {
    if states_shape.len() != 4 {
        return Err(PyValueError::new_err(
            "states must have shape [batch, planes, height, width]",
        ));
    }
    if policies_shape.len() != 2 {
        return Err(PyValueError::new_err(
            "policies must have shape [batch, actions]",
        ));
    }
    if values_shape.len() != 1 {
        return Err(PyValueError::new_err("values must have shape [batch]"));
    }
    if opponent_policies_shape.len() != 2 {
        return Err(PyValueError::new_err(
            "opponent_policies must have shape [batch, actions]",
        ));
    }
    if opponent_policy_masks_shape.len() != 1 {
        return Err(PyValueError::new_err(
            "opponent_policy_masks must have shape [batch]",
        ));
    }
    if ownership_shape.len() != 3 {
        return Err(PyValueError::new_err(
            "ownership must have shape [batch, height, width]",
        ));
    }
    if score_differences_shape.len() != 1 {
        return Err(PyValueError::new_err(
            "score_differences must have shape [batch]",
        ));
    }

    let sample_count = states_shape[0];
    let plane_count = states_shape[1];
    let height = states_shape[2];
    let width = states_shape[3];

    if policies_shape[0] != sample_count
        || values_shape[0] != sample_count
        || opponent_policies_shape[0] != sample_count
        || opponent_policy_masks_shape[0] != sample_count
        || ownership_shape[0] != sample_count
        || score_differences_shape[0] != sample_count
    {
        return Err(PyValueError::new_err(
            "all inputs must have the same batch dimension",
        ));
    }

    if ownership_shape[1] != height || ownership_shape[2] != width {
        return Err(PyValueError::new_err(
            "ownership must match the board height and width from states",
        ));
    }

    if policies_shape[1] != opponent_policies_shape[1] {
        return Err(PyValueError::new_err(
            "policies and opponent_policies must have the same action dimension",
        ));
    }

    let board_area = height * width;
    if policies_shape[1] != board_area && policies_shape[1] != board_area + 1 {
        return Err(PyValueError::new_err(
            "go policies must have board_area or board_area + 1 actions",
        ));
    }

    Ok((sample_count, plane_count, height, width, policies_shape[1]))
}

fn spatial_transforms(height: usize, width: usize) -> Vec<SpatialTransform> {
    if height == width {
        let mut transforms = Vec::with_capacity(8);
        for flip in [false, true] {
            for rotation in 0..4 {
                transforms.push(SpatialTransform { rotation, flip });
            }
        }
        return transforms;
    }

    vec![
        SpatialTransform {
            rotation: 0,
            flip: false,
        },
        SpatialTransform {
            rotation: 2,
            flip: false,
        },
        SpatialTransform {
            rotation: 0,
            flip: true,
        },
        SpatialTransform {
            rotation: 2,
            flip: true,
        },
    ]
}

fn transform_position(
    row: usize,
    col: usize,
    height: usize,
    width: usize,
    transform: SpatialTransform,
) -> (usize, usize) {
    if height == width {
        let size = height;
        let mut transformed_row = row;
        let mut transformed_col = if transform.flip { size - 1 - col } else { col };
        for _ in 0..transform.rotation {
            let next_row = size - 1 - transformed_col;
            let next_col = transformed_row;
            transformed_row = next_row;
            transformed_col = next_col;
        }
        return (transformed_row, transformed_col);
    }

    let mut transformed_row = row;
    let mut transformed_col = if transform.flip { width - 1 - col } else { col };
    if transform.rotation == 2 {
        transformed_row = height - 1 - transformed_row;
        transformed_col = width - 1 - transformed_col;
    }
    (transformed_row, transformed_col)
}

#[pyfunction]
pub fn augment_symmetries<'py>(
    py: Python<'py>,
    states: PyReadonlyArray4<'py, f32>,
    policies: PyReadonlyArray2<'py, f32>,
    values: PyReadonlyArray1<'py, f32>,
    opponent_policies: PyReadonlyArray2<'py, f32>,
    opponent_policy_masks: PyReadonlyArray1<'py, f32>,
    ownership: PyReadonlyArray3<'py, f32>,
    score_differences: PyReadonlyArray1<'py, f32>,
) -> PyResult<(
    Bound<'py, PyArray4<f32>>,
    Bound<'py, PyArray2<f32>>,
    Bound<'py, PyArray1<f32>>,
    Bound<'py, PyArray2<f32>>,
    Bound<'py, PyArray1<f32>>,
    Bound<'py, PyArray3<f32>>,
    Bound<'py, PyArray1<f32>>,
)> {
    let states = states.as_array();
    let policies = policies.as_array();
    let values = values.as_array();
    let opponent_policies = opponent_policies.as_array();
    let opponent_policy_masks = opponent_policy_masks.as_array();
    let ownership = ownership.as_array();
    let score_differences = score_differences.as_array();

    let (sample_count, plane_count, height, width, action_size) = validate_batch_shapes(
        states.shape(),
        policies.shape(),
        values.shape(),
        opponent_policies.shape(),
        opponent_policy_masks.shape(),
        ownership.shape(),
        score_differences.shape(),
    )?;

    let board_area = height * width;
    let has_pass = action_size == board_area + 1;
    let transforms = spatial_transforms(height, width);
    let expansion_factor = transforms.len();
    let augmented_sample_count = sample_count * expansion_factor;

    let mut augmented_states =
        Array4::<f32>::zeros((augmented_sample_count, plane_count, height, width));
    let mut augmented_policies = Array2::<f32>::zeros((augmented_sample_count, action_size));
    let mut augmented_values = Array1::<f32>::zeros(augmented_sample_count);
    let mut augmented_opponent_policies =
        Array2::<f32>::zeros((augmented_sample_count, action_size));
    let mut augmented_opponent_policy_masks = Array1::<f32>::zeros(augmented_sample_count);
    let mut augmented_ownership = Array3::<f32>::zeros((augmented_sample_count, height, width));
    let mut augmented_score_differences = Array1::<f32>::zeros(augmented_sample_count);

    for (transform_index, transform) in transforms.iter().copied().enumerate() {
        let sample_offset = transform_index * sample_count;
        augmented_values
            .slice_mut(s![sample_offset..sample_offset + sample_count])
            .assign(&values);
        augmented_opponent_policy_masks
            .slice_mut(s![sample_offset..sample_offset + sample_count])
            .assign(&opponent_policy_masks);
        augmented_score_differences
            .slice_mut(s![sample_offset..sample_offset + sample_count])
            .assign(&score_differences);

        for sample_idx in 0..sample_count {
            let augmented_sample_idx = sample_offset + sample_idx;
            if has_pass {
                augmented_policies[[augmented_sample_idx, board_area]] =
                    policies[[sample_idx, board_area]];
                augmented_opponent_policies[[augmented_sample_idx, board_area]] =
                    opponent_policies[[sample_idx, board_area]];
            }

            for plane_idx in 0..plane_count {
                for row_idx in 0..height {
                    for col_idx in 0..width {
                        let (transformed_row, transformed_col) =
                            transform_position(row_idx, col_idx, height, width, transform);
                        augmented_states[[
                            augmented_sample_idx,
                            plane_idx,
                            transformed_row,
                            transformed_col,
                        ]] = states[[sample_idx, plane_idx, row_idx, col_idx]];
                    }
                }
            }

            for row_idx in 0..height {
                for col_idx in 0..width {
                    let source_action_idx = row_idx * width + col_idx;
                    let (transformed_row, transformed_col) =
                        transform_position(row_idx, col_idx, height, width, transform);
                    let transformed_action_idx = transformed_row * width + transformed_col;

                    augmented_policies[[augmented_sample_idx, transformed_action_idx]] =
                        policies[[sample_idx, source_action_idx]];
                    augmented_opponent_policies[[augmented_sample_idx, transformed_action_idx]] =
                        opponent_policies[[sample_idx, source_action_idx]];
                    augmented_ownership[[augmented_sample_idx, transformed_row, transformed_col]] =
                        ownership[[sample_idx, row_idx, col_idx]];
                }
            }
        }
    }

    Ok((
        augmented_states.into_pyarray(py),
        augmented_policies.into_pyarray(py),
        augmented_values.into_pyarray(py),
        augmented_opponent_policies.into_pyarray(py),
        augmented_opponent_policy_masks.into_pyarray(py),
        augmented_ownership.into_pyarray(py),
        augmented_score_differences.into_pyarray(py),
    ))
}
