//! Inverse aerial control solver that reconstructs pitch/yaw/roll inputs from
//! observed angular velocity deltas between replay frames.
//!
//! Based on the Sam Mish / `ZealanL` implementation ported from rlgym-tools:
//! <https://github.com/RLGym/rlgym-tools/blob/main/rlgym_tools/rocket_league/math/inverse_aerial_controls.py>

use rocketsim::{Mat3A, Vec3A};

/// Torque coefficient for roll
const T_R: f32 = -36.079_567;
/// Torque coefficient for pitch
const T_P: f32 = -12.145_998;
/// Torque coefficient for yaw
const T_Y: f32 = 8.919_628;
/// Drag coefficient for roll
const D_R: f32 = -4.471_663;
/// Drag coefficient for pitch
const D_P: f32 = -2.798_194_2;
/// Drag coefficient for yaw
const D_Y: f32 = -1.886_491_9;

/// Reconstructs (pitch, yaw, roll) analog stick inputs from the angular velocity
/// delta observed across a replay frame.
///
/// The solver inverts the Rocket League aerial control model: given the car's
/// rotation matrices and angular velocity at two consecutive frames, it computes
/// the stick inputs (pitch, yaw, roll) that would produce the observed change
/// in angular velocity.
///
/// `dt` is the time delta between the two frames (typically `1/30` or a
/// replay-authored frame delta). `is_flipping` enables flip-cancel detection
/// which overrides the pitch input when the observed pitch angular velocity is
/// decelerating (indicating the player released the stick to cancel a dodge).
#[allow(clippy::similar_names)]
pub(crate) fn aerial_inputs(
    ang_vel_start: Vec3A,
    ang_vel_end: Vec3A,
    rot_mat_start: Mat3A,
    rot_mat_end: Mat3A,
    dt: f32,
    is_flipping: bool,
) -> (f32, f32, f32) {
    // 1. Scale up to get full inputs when the car is near max angular velocity
    let mut scale = 1.0;
    if ang_vel_end.length() >= rocketsim::consts::car::MAX_ANG_SPEED - 0.01 {
        scale = 1.25;
    }

    // 2. Net torque in world coords
    let tau = (ang_vel_end * scale - ang_vel_start) / dt;

    // 3–4. Transform torque and start angular velocity to local coords using the
    // transpose of rot_mat_start (the inverse for orthonormal rotation matrices).
    let tau_local = world_to_local(tau, rot_mat_start);
    let ang_vel_local = world_to_local(ang_vel_start, rot_mat_start);

    // 5. Subtract damping from applied torque to get the net effect of stick input
    let rhs = Vec3A::new(
        tau_local.x - D_R * ang_vel_local.x,
        tau_local.y - D_P * ang_vel_local.y,
        tau_local.z - D_Y * ang_vel_local.z,
    );

    // 6. Solve for analog stick inputs (u = roll, v = pitch, w = yaw)
    //    The pitch and yaw denominators incorporate velocity-dependent damping.
    let mut u = Vec3A::new(
        rhs.x / T_R,
        rhs.y / (T_P + rhs.y.signum() * ang_vel_local.y * D_P),
        rhs.z / (T_Y - rhs.z.signum() * ang_vel_local.z * D_Y),
    );

    // 7. Clip to valid controller range
    u = u.clamp(Vec3A::splat(-1.0), Vec3A::splat(1.0));

    // 8. Flip-cancel detection: if pitch angular velocity is decelerating
    //    (start magnitude > end magnitude), the player released the stick,
    //    so force pitch to the start direction.
    if is_flipping {
        let local_ang_vel_end = world_to_local(ang_vel_end, rot_mat_end);
        if ang_vel_local.y.abs() > local_ang_vel_end.y.abs() {
            u.y = ang_vel_local.y.signum();
        }
    }

    // 9. Return (pitch, yaw, roll) — the solver produces (roll, pitch, yaw)
    //    internally, so we reorder to match the RL convention.
    (u.y, u.z, u.x)
}

/// Transforms a world-space vector to the local frame of a rotation matrix
/// by multiplying with the transpose (inverse for orthonormal matrices).
///
/// Each local component is the dot product of the world vector with the
/// corresponding column of the rotation matrix.
#[inline]
fn world_to_local(v: Vec3A, rot_mat: Mat3A) -> Vec3A {
    Vec3A::new(
        v.dot(rot_mat.x_axis),
        v.dot(rot_mat.y_axis),
        v.dot(rot_mat.z_axis),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The scale threshold — `MAX_ANG_SPEED` minus 0.01 — from the algorithm.
    const SCALE_THRESHOLD: f32 = rocketsim::consts::car::MAX_ANG_SPEED - 0.01;

    #[test]
    fn zero_delta_returns_zero_inputs() {
        // No change in angular velocity → no input needed.
        let ident = Mat3A::IDENTITY;
        let result = aerial_inputs(Vec3A::ZERO, Vec3A::ZERO, ident, ident, 1.0 / 30.0, false);
        assert!(
            (result.0.abs() < 1e-5) && (result.1.abs() < 1e-5) && (result.2.abs() < 1e-5),
            "expected zero inputs, got {result:?}"
        );
    }

    #[test]
    fn max_ang_vel_triggers_scale() {
        // When ang_vel_end length is at MAX_ANG_SPEED - 0.005 (above threshold),
        // the 1.25× scale should be applied.
        let near_max = Vec3A::new(SCALE_THRESHOLD + 0.005, 0.0, 0.0);
        let ident = Mat3A::IDENTITY;
        // With a positive torque, the solver should yield positive roll (u.x).
        let result = aerial_inputs(Vec3A::ZERO, near_max, ident, ident, 1.0 / 30.0, false);
        // The result should be non-trivial because scale > 1
        assert!(
            result.2.abs() > 0.001,
            "expected non-zero roll at max ang vel"
        );
    }

    #[test]
    fn below_threshold_uses_unit_scale() {
        // Below the threshold, scale stays at 1.0.
        let moderate = Vec3A::new(SCALE_THRESHOLD - 1.0, 0.0, 0.0);
        let ident = Mat3A::IDENTITY;
        let result = aerial_inputs(Vec3A::ZERO, moderate, ident, ident, 1.0 / 30.0, false);
        // Should produce some roll input
        assert!(result.2.abs() > 0.001, "expected non-zero roll");
    }

    #[test]
    fn outputs_are_clipped() {
        // A huge angular velocity delta should produce clipped outputs.
        let huge = Vec3A::new(1000.0, 0.0, 0.0);
        let ident = Mat3A::IDENTITY;
        let result = aerial_inputs(Vec3A::ZERO, huge, ident, ident, 1.0 / 30.0, false);
        assert!(
            result.0.abs() <= 1.0 && result.1.abs() <= 1.0 && result.2.abs() <= 1.0,
            "all outputs should be clipped to [-1, 1], got {result:?}"
        );
    }

    #[test]
    fn flip_cancel_overrides_pitch() {
        // Simulate a flip where pitch angular velocity is decelerating.
        let ident = Mat3A::IDENTITY;
        // Start with positive pitch ang vel, end with much less.
        let start = Vec3A::new(0.0, 3.0, 0.0);
        let end = Vec3A::new(0.0, 1.0, 0.0);
        let result = aerial_inputs(start, end, ident, ident, 1.0 / 30.0, true);
        // Pitch should be signum of start = 1.0
        assert!(
            (result.0 - 1.0).abs() < 1e-5,
            "flip cancel should force pitch to 1.0, got {}",
            result.0
        );
    }

    #[test]
    fn flip_cancel_not_triggered_when_accelerating() {
        // If pitch ang vel is increasing, flip cancel should NOT activate.
        let ident = Mat3A::IDENTITY;
        let start = Vec3A::new(0.0, 1.0, 0.0);
        let end = Vec3A::new(0.0, 3.0, 0.0);
        let result = aerial_inputs(start, end, ident, ident, 1.0 / 30.0, true);
        // Pitch should NOT be forced to signum(start) since |end| > |start|
        assert!(
            (result.0 - 1.0).abs() > 0.0,
            "flip cancel should not override pitch when accelerating"
        );
    }

    #[test]
    fn pitch_yaw_roll_axis_reordering() {
        // Apply a pure torque around each axis and verify the reordering.
        // roll torque (x) → output.2 (roll)
        // pitch torque (y) → output.0 (pitch)
        // yaw torque (z) → output.1 (yaw)
        let ident = Mat3A::IDENTITY;
        let dt = 1.0 / 30.0;

        // Pure roll torque (x-axis input)
        let roll_torque = Vec3A::new(50.0, 0.0, 0.0);
        let r = aerial_inputs(Vec3A::ZERO, roll_torque * dt, ident, ident, dt, false);
        assert!(
            r.2.abs() > r.0.abs() && r.2.abs() > r.1.abs(),
            "roll torque should produce dominant roll output, got {r:?}"
        );

        // Pure pitch torque (y-axis input)
        let pitch_torque = Vec3A::new(0.0, 50.0, 0.0);
        let r = aerial_inputs(Vec3A::ZERO, pitch_torque * dt, ident, ident, dt, false);
        assert!(
            r.0.abs() > r.1.abs() && r.0.abs() > r.2.abs(),
            "pitch torque should produce dominant pitch output, got {r:?}"
        );

        // Pure yaw torque (z-axis input)
        let yaw_torque = Vec3A::new(0.0, 0.0, 50.0);
        let r = aerial_inputs(Vec3A::ZERO, yaw_torque * dt, ident, ident, dt, false);
        assert!(
            r.1.abs() > r.0.abs() && r.1.abs() > r.2.abs(),
            "yaw torque should produce dominant yaw output, got {r:?}"
        );
    }

    #[test]
    fn rotation_does_not_affect_invariant() {
        // If we rotate the car and apply the same body-frame torque, the
        // solver should produce the same (pitch, yaw, roll) outputs.
        let dt = 1.0 / 30.0;

        // A 45-degree rotation around Z (yaw)
        // Mat3A::from_rotation_z(FRAC_PI_4) but we can construct manually.
        let frac_pi_4 = std::f32::consts::FRAC_PI_4;
        let (s, c) = (frac_pi_4.sin(), frac_pi_4.cos());
        let rot = Mat3A::from_cols(
            Vec3A::new(c, s, 0.0),
            Vec3A::new(-s, c, 0.0),
            Vec3A::new(0.0, 0.0, 1.0),
        );

        // A pitch torque in local frame means world torque is rotated
        let local_pitch_torque = Vec3A::new(0.0, 50.0 * dt, 0.0);
        // World torque = rot * local_torque (since rot's columns are local basis)
        let world_torque = rot * local_pitch_torque;

        let ident = Mat3A::IDENTITY;
        let r_ident = aerial_inputs(Vec3A::ZERO, local_pitch_torque, ident, ident, dt, false);
        let r_rot = aerial_inputs(Vec3A::ZERO, world_torque, rot, rot, dt, false);

        let diff =
            (r_ident.0 - r_rot.0).abs() + (r_ident.1 - r_rot.1).abs() + (r_ident.2 - r_rot.2).abs();
        assert!(
            diff < 1e-4,
            "rotation-invariant test failed: ident={r_ident:?} rot={r_rot:?} diff={diff}"
        );
    }
}
