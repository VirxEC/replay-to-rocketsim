//! Compare replay-authored state with the `RocketSim` prediction before replay sync.
//!
//! Usage: `cargo run --release --example audit_prediction -- <replay> [<replay> ...]`
//!
//! The comparison is intentionally tolerant: replay physics is sparse and `RocketSim` is not an
//! exact replica of the game. Lifecycle values are stricter because they are useful invariants.

use std::{env, fs};

use replay_to_rocketsim::rocketsim::{Mat3A, Vec3A, init_from_default};
use replay_to_rocketsim::{ConversionOutput, Converter};

const POSITION_TOLERANCE: f32 = 80.0;
const VELOCITY_TOLERANCE: f32 = 350.0;
const ANGULAR_VELOCITY_TOLERANCE: f32 = 8.0;
const ROTATION_TOLERANCE: f32 = 0.35;
const TIMER_TOLERANCE: f32 = 0.20;
const MAX_RESPAWN_TIMER: f32 = 10.0;
const MAX_PRINTED_DISCREPANCIES: usize = 100;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths = env::args().skip(1).collect::<Vec<_>>();
    if paths.is_empty() {
        return Err(
            "usage: cargo run --example audit_prediction -- <replay> [<replay> ...]".into(),
        );
    }

    init_from_default(true)?;
    let mut aggregate = Summary::default();
    for path in paths {
        let bytes = fs::read(&path)?;
        let output = Converter::new().convert_bytes(&bytes)?;
        let summary = audit(&path, &output);
        aggregate.merge(&summary);
    }

    println!(
        "\naggregate: {} frames, {} cars, {} discrepancies ({} severe)",
        aggregate.frames, aggregate.car_rows, aggregate.discrepancies, aggregate.severe
    );
    Ok(())
}

#[derive(Default)]
struct Summary {
    frames: usize,
    car_rows: usize,
    discrepancies: usize,
    severe: usize,
}

impl Summary {
    fn merge(&mut self, other: &Self) {
        self.frames += other.frames;
        self.car_rows += other.car_rows;
        self.discrepancies += other.discrepancies;
        self.severe += other.severe;
    }
}

fn audit(path: &str, output: &ConversionOutput) -> Summary {
    let mut summary = Summary {
        frames: output.frames.len(),
        ..Summary::default()
    };
    let mut printed = 0;

    for (frame_idx, (((predicted, observed), replay_cars), prediction_valid)) in output
        .predicted_states
        .iter()
        .zip(&output.states)
        .zip(&output.cars)
        .zip(&output.prediction_valid)
        .enumerate()
    {
        if !prediction_valid {
            continue;
        }
        let ball_meta = &output.frame_metadata[frame_idx].ball.rigid_body;
        if is_fresh(ball_meta.pos_update_age) {
            compare_vec3(
                &mut summary,
                &mut printed,
                path,
                output.frames[frame_idx].replay_frame,
                usize::MAX,
                "ball",
                "position",
                predicted.ball.phys.pos,
                observed.ball.phys.pos,
                POSITION_TOLERANCE,
            );
        }
        if is_fresh(ball_meta.vel_update_age) {
            compare_vec3(
                &mut summary,
                &mut printed,
                path,
                output.frames[frame_idx].replay_frame,
                usize::MAX,
                "ball",
                "velocity",
                predicted.ball.phys.vel,
                observed.ball.phys.vel,
                VELOCITY_TOLERANCE,
            );
        }
        for (car_idx, ((_, predicted_car), (_, observed_car))) in
            predicted.cars.iter().zip(&observed.cars).enumerate()
        {
            summary.car_rows += 1;
            let meta = replay_cars.get(car_idx);
            let label = meta
                .and_then(|car| car.player_name.as_deref())
                .unwrap_or("unknown");
            let frame = output.frames[frame_idx].replay_frame;

            // These fields are copied from replay on every car row, so they are a direct
            // replay-vs-prediction comparison rather than a comparison of two derived values.
            if let Some(meta) = meta {
                // Replay rigid-body updates are sparse. Only compare a component on frames where
                // that component arrived from the replay; otherwise the observed value is merely
                // the converter's accumulated value and is not new evidence.
                if is_fresh(meta.rigid_body.pos_update_age) {
                    compare_vec3(
                        &mut summary,
                        &mut printed,
                        path,
                        frame,
                        car_idx,
                        label,
                        "position",
                        predicted_car.phys.pos,
                        observed_car.phys.pos,
                        POSITION_TOLERANCE,
                    );
                }
                if is_fresh(meta.rigid_body.vel_update_age) {
                    compare_vec3(
                        &mut summary,
                        &mut printed,
                        path,
                        frame,
                        car_idx,
                        label,
                        "velocity",
                        predicted_car.phys.vel,
                        observed_car.phys.vel,
                        VELOCITY_TOLERANCE,
                    );
                }
                if is_fresh(meta.rigid_body.rot_update_age) {
                    compare_mat3(
                        &mut summary,
                        &mut printed,
                        path,
                        frame,
                        car_idx,
                        label,
                        predicted_car.phys.rot_mat,
                        observed_car.phys.rot_mat,
                        ROTATION_TOLERANCE,
                    );
                }
                if is_fresh(meta.rigid_body.ang_vel_update_age) {
                    compare_vec3(
                        &mut summary,
                        &mut printed,
                        path,
                        frame,
                        car_idx,
                        label,
                        "angular_velocity",
                        predicted_car.phys.ang_vel,
                        observed_car.phys.ang_vel,
                        ANGULAR_VELOCITY_TOLERANCE,
                    );
                }

                let timer_delta =
                    (observed_car.demo_respawn_timer - predicted_car.demo_respawn_timer).abs();
                if timer_delta > TIMER_TOLERANCE {
                    discrepancy(
                        &mut summary,
                        &mut printed,
                        path,
                        frame,
                        car_idx,
                        label,
                        &format!(
                            "demo_respawn_timer predicted={:.3} replay={:.3} delta={:.3}",
                            predicted_car.demo_respawn_timer, meta.demo_respawn_timer, timer_delta
                        ),
                        false,
                    );
                }
                if !meta.demo_respawn_timer.is_finite()
                    || meta.demo_respawn_timer > MAX_RESPAWN_TIMER
                {
                    discrepancy(
                        &mut summary,
                        &mut printed,
                        path,
                        frame,
                        car_idx,
                        label,
                        &format!(
                            "IMPOSSIBLE demo_respawn_timer={:.3} (max expected {:.1}s)",
                            meta.demo_respawn_timer, MAX_RESPAWN_TIMER
                        ),
                        true,
                    );
                }
            }

            compare_bool(
                &mut summary,
                &mut printed,
                path,
                frame,
                car_idx,
                label,
                "is_demoed",
                predicted_car.is_demoed,
                observed_car.is_demoed,
            );
        }
    }

    println!(
        "{}: {} frames, {} cars, {} discrepancies ({} severe)",
        path, summary.frames, summary.car_rows, summary.discrepancies, summary.severe
    );
    summary
}

#[allow(clippy::too_many_arguments)]
fn compare_vec3(
    summary: &mut Summary,
    printed: &mut usize,
    path: &str,
    frame: usize,
    car: usize,
    label: &str,
    field: &str,
    predicted: Vec3A,
    observed: Vec3A,
    tolerance: f32,
) {
    let delta = (observed - predicted).length();
    if delta > tolerance || !delta.is_finite() {
        discrepancy(
            summary,
            printed,
            path,
            frame,
            car,
            label,
            &format!(
                "{field} delta={delta:.2} predicted={predicted:?} replay={observed:?} tol={tolerance:.1}"
            ),
            false,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn compare_mat3(
    summary: &mut Summary,
    printed: &mut usize,
    path: &str,
    frame: usize,
    car: usize,
    label: &str,
    predicted: Mat3A,
    observed: Mat3A,
    tolerance: f32,
) {
    let delta = (observed.x_axis - predicted.x_axis)
        .length()
        .max((observed.y_axis - predicted.y_axis).length())
        .max((observed.z_axis - predicted.z_axis).length());
    if delta > tolerance || !delta.is_finite() {
        discrepancy(
            summary,
            printed,
            path,
            frame,
            car,
            label,
            &format!("rotation delta={delta:.3} tol={tolerance:.2}"),
            false,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn compare_bool(
    summary: &mut Summary,
    printed: &mut usize,
    path: &str,
    frame: usize,
    car: usize,
    label: &str,
    field: &str,
    predicted: bool,
    observed: bool,
) {
    if predicted != observed {
        discrepancy(
            summary,
            printed,
            path,
            frame,
            car,
            label,
            &format!("{field} predicted={predicted} replay={observed}"),
            false,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn discrepancy(
    summary: &mut Summary,
    printed: &mut usize,
    path: &str,
    frame: usize,
    car: usize,
    label: &str,
    detail: &str,
    severe: bool,
) {
    summary.discrepancies += 1;
    summary.severe += usize::from(severe);
    if *printed < MAX_PRINTED_DISCREPANCIES {
        println!("{path} frame={frame} car={car} ({label}) {detail}");
        *printed += 1;
    }
}

fn is_fresh(age: Option<f32>) -> bool {
    age.is_some_and(|value| value.abs() < 0.0001)
}
