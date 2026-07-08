use boxcars::{ActorId, Attribute, NewActor};
use glam::{EulerRot, Quat};
use rocketsim::{Mat3A, Vec3A};

use crate::ConvertError;

#[derive(Debug, Clone, Copy)]
pub(crate) struct AccumulatedPhys {
    pos: Option<Vec3A>,
    rot_mat: Mat3A,
    vel: Vec3A,
    ang_vel: Vec3A,
    is_sleeping: Option<bool>,
    freshness: PhysFreshness,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct PhysFreshness {
    pub(crate) pos_frame: Option<usize>,
    pub(crate) pos_time: Option<f32>,
    pub(crate) rot_frame: Option<usize>,
    pub(crate) rot_time: Option<f32>,
    pub(crate) vel_frame: Option<usize>,
    pub(crate) vel_time: Option<f32>,
    pub(crate) ang_vel_frame: Option<usize>,
    pub(crate) ang_vel_time: Option<f32>,
    pub(crate) sleeping_frame: Option<usize>,
    pub(crate) sleeping_time: Option<f32>,
}

impl PhysFreshness {
    pub(crate) fn any_current(self, replay_frame: usize) -> bool {
        self.pos_frame == Some(replay_frame)
            || self.rot_frame == Some(replay_frame)
            || self.vel_frame == Some(replay_frame)
            || self.ang_vel_frame == Some(replay_frame)
    }

    pub(crate) fn any_phys_time(self) -> Option<f32> {
        [
            self.pos_time,
            self.rot_time,
            self.vel_time,
            self.ang_vel_time,
        ]
        .into_iter()
        .flatten()
        .max_by(f32::total_cmp)
    }
}

impl AccumulatedPhys {
    #[allow(clippy::cast_precision_loss)]
    pub(crate) fn from_spawn(new_actor: &NewActor, replay_frame: usize, replay_time: f32) -> Self {
        let pos = new_actor
            .initial_trajectory
            .location
            .map(|location| Vec3A::new(location.x as f32, location.y as f32, location.z as f32));
        let rot_mat = new_actor
            .initial_trajectory
            .rotation
            .map(spawn_rotation_to_mat3a);
        Self {
            pos,
            rot_mat: rot_mat.unwrap_or(Mat3A::IDENTITY),
            vel: Vec3A::ZERO,
            ang_vel: Vec3A::ZERO,
            is_sleeping: None,
            freshness: PhysFreshness {
                pos_frame: pos.map(|_| replay_frame),
                pos_time: pos.map(|_| replay_time),
                rot_frame: rot_mat.map(|_| replay_frame),
                rot_time: rot_mat.map(|_| replay_time),
                ..PhysFreshness::default()
            },
        }
    }

    pub(crate) fn apply_rigid_body(
        &mut self,
        actor_id: ActorId,
        attribute: &Attribute,
        replay_version: i32,
        replay_frame: usize,
        replay_time: f32,
    ) -> Result<(), ConvertError> {
        let Attribute::RigidBody(rb_state) = attribute else {
            return Ok(());
        };

        self.pos = Some(scale_replay_vec(
            Vec3A::new(
                rb_state.location.x,
                rb_state.location.y,
                rb_state.location.z,
            ),
            replay_version,
            100.0,
        ));
        ensure_finite(actor_id, "location", self.pos.unwrap())?;
        self.freshness.pos_frame = Some(replay_frame);
        self.freshness.pos_time = Some(replay_time);

        if let Some(linear_velocity) = rb_state.linear_velocity {
            self.vel = scale_replay_vec(
                Vec3A::new(linear_velocity.x, linear_velocity.y, linear_velocity.z),
                replay_version,
                10.0,
            );
            ensure_finite(actor_id, "linear_velocity", self.vel)?;
            self.freshness.vel_frame = Some(replay_frame);
            self.freshness.vel_time = Some(replay_time);
        }

        self.rot_mat = if replay_version >= 8 {
            quat_to_mat3a(
                actor_id,
                rb_state.rotation.x,
                rb_state.rotation.y,
                rb_state.rotation.z,
                rb_state.rotation.w,
            )?
        } else {
            rotator_to_mat3a(
                actor_id,
                rb_state.rotation.x,
                rb_state.rotation.y,
                rb_state.rotation.z,
            )?
        };
        self.freshness.rot_frame = Some(replay_frame);
        self.freshness.rot_time = Some(replay_time);

        if let Some(angular_velocity) = rb_state.angular_velocity {
            self.ang_vel = Vec3A::new(
                angular_velocity.x / 100.0,
                angular_velocity.y / 100.0,
                angular_velocity.z / 100.0,
            );
            ensure_finite(actor_id, "angular_velocity", self.ang_vel)?;
            self.freshness.ang_vel_frame = Some(replay_frame);
            self.freshness.ang_vel_time = Some(replay_time);
        }

        self.is_sleeping = Some(rb_state.sleeping);
        self.freshness.sleeping_frame = Some(replay_frame);
        self.freshness.sleeping_time = Some(replay_time);

        Ok(())
    }

    pub(crate) fn freshness(self) -> PhysFreshness {
        self.freshness
    }

    pub(crate) fn pos(self) -> Option<Vec3A> {
        self.pos
    }

    pub(crate) fn is_sleeping(self) -> Option<bool> {
        self.is_sleeping
    }

    pub(crate) fn to_phys_state(
        self,
        actor_id: ActorId,
    ) -> Result<rocketsim::PhysState, ConvertError> {
        Ok(rocketsim::PhysState {
            pos: self
                .pos
                .ok_or(ConvertError::MissingRigidBodyLocation(actor_id))?,
            rot_mat: self.rot_mat,
            vel: self.vel,
            ang_vel: self.ang_vel,
        })
    }
}

fn spawn_rotation_to_mat3a(rotation: boxcars::Rotation) -> Mat3A {
    let angle = |value: Option<i8>| {
        value.map_or(0.0, |value| {
            f32::from(value) * std::f32::consts::TAU / 256.0
        })
    };
    Mat3A::from_quat(Quat::from_euler(
        EulerRot::ZYX,
        angle(rotation.yaw),
        angle(rotation.pitch),
        angle(rotation.roll),
    ))
}

fn scale_replay_vec(value: Vec3A, replay_version: i32, pre_v7_scale: f32) -> Vec3A {
    if replay_version >= 7 {
        value
    } else {
        value * pre_v7_scale
    }
}

fn ensure_finite(actor_id: ActorId, field: &'static str, value: Vec3A) -> Result<(), ConvertError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(ConvertError::NonFiniteRigidBody { actor_id, field })
    }
}

fn rotator_to_mat3a(
    actor_id: ActorId,
    pitch: f32,
    yaw: f32,
    roll: f32,
) -> Result<Mat3A, ConvertError> {
    if !pitch.is_finite() || !yaw.is_finite() || !roll.is_finite() {
        return Err(ConvertError::NonFiniteRigidBody {
            actor_id,
            field: "rotation",
        });
    }
    // Verified equivalent to the old Rocket League rotator formula:
    // w = cr*cp*cy + sr*sp*sy
    // x = sr*cp*cy - cr*sp*sy
    // y = cr*sp*cy + sr*cp*sy
    // z = cr*cp*sy - sr*sp*cy
    // The argument order is intentionally yaw, pitch, roll for ZYX.
    Ok(Mat3A::from_quat(Quat::from_euler(
        EulerRot::ZYX,
        yaw,
        pitch,
        roll,
    )))
}

fn quat_to_mat3a(actor_id: ActorId, x: f32, y: f32, z: f32, w: f32) -> Result<Mat3A, ConvertError> {
    let quat = Quat::from_xyzw(x, y, z, w);
    let norm_squared = quat.length_squared();
    if !norm_squared.is_finite() || norm_squared <= f32::EPSILON {
        return Err(ConvertError::NonFiniteRigidBody {
            actor_id,
            field: "rotation",
        });
    }

    Ok(Mat3A::from_quat(quat.normalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glam_euler_order_matches_legacy_rotator_formula() {
        let pitch = 0.31;
        let yaw = -1.2;
        let roll = 2.4;

        let glam_mat = rotator_to_mat3a(ActorId(1), pitch, yaw, roll).unwrap();
        let legacy_mat = legacy_rotator_to_mat3a(pitch, yaw, roll);

        assert_mat3a_near(glam_mat, legacy_mat);
    }

    #[test]
    fn glam_quat_matrix_matches_legacy_column_major_formula() {
        let x = 0.1;
        let y = -0.2;
        let z = 0.3;
        let w = 0.9;

        let glam_mat = quat_to_mat3a(ActorId(1), x, y, z, w).unwrap();
        let legacy_mat = legacy_quat_to_mat3a(x, y, z, w);

        assert_mat3a_near(glam_mat, legacy_mat);
    }

    #[test]
    fn rejects_non_finite_rotators() {
        let err = rotator_to_mat3a(ActorId(99), f32::NAN, 0.0, 0.0).unwrap_err();
        assert!(matches!(
            err,
            ConvertError::NonFiniteRigidBody {
                actor_id: ActorId(99),
                field: "rotation"
            }
        ));
    }

    #[test]
    fn rejects_non_finite_quaternions() {
        let err = quat_to_mat3a(ActorId(99), f32::NAN, 0.0, 0.0, 1.0).unwrap_err();
        assert!(matches!(
            err,
            ConvertError::NonFiniteRigidBody {
                actor_id: ActorId(99),
                field: "rotation"
            }
        ));
    }

    fn legacy_rotator_to_mat3a(pitch: f32, yaw: f32, roll: f32) -> Mat3A {
        let sin_pitch = f32::sin(pitch / 2.0);
        let cos_pitch = f32::cos(pitch / 2.0);
        let sin_yaw = f32::sin(yaw / 2.0);
        let cos_yaw = f32::cos(yaw / 2.0);
        let sin_roll = f32::sin(roll / 2.0);
        let cos_roll = f32::cos(roll / 2.0);

        let w = (cos_roll * cos_pitch * cos_yaw) + (sin_roll * sin_pitch * sin_yaw);
        let x = (sin_roll * cos_pitch * cos_yaw) - (cos_roll * sin_pitch * sin_yaw);
        let y = (cos_roll * sin_pitch * cos_yaw) + (sin_roll * cos_pitch * sin_yaw);
        let z = (cos_roll * cos_pitch * sin_yaw) - (sin_roll * sin_pitch * cos_yaw);

        legacy_quat_to_mat3a(x, y, z, w)
    }

    fn legacy_quat_to_mat3a(x: f32, y: f32, z: f32, w: f32) -> Mat3A {
        let norm = (x * x + y * y + z * z + w * w).sqrt();
        let x = x / norm;
        let y = y / norm;
        let z = z / norm;
        let w = w / norm;

        let x2 = x + x;
        let y2 = y + y;
        let z2 = z + z;
        let xx = x * x2;
        let xy = x * y2;
        let xz = x * z2;
        let yy = y * y2;
        let yz = y * z2;
        let zz = z * z2;
        let wx = w * x2;
        let wy = w * y2;
        let wz = w * z2;

        Mat3A::from_cols(
            Vec3A::new(1.0 - (yy + zz), xy + wz, xz - wy),
            Vec3A::new(xy - wz, 1.0 - (xx + zz), yz + wx),
            Vec3A::new(xz + wy, yz - wx, 1.0 - (xx + yy)),
        )
    }

    fn assert_mat3a_near(actual: Mat3A, expected: Mat3A) {
        assert_vec3a_near(actual.x_axis, expected.x_axis);
        assert_vec3a_near(actual.y_axis, expected.y_axis);
        assert_vec3a_near(actual.z_axis, expected.z_axis);
    }

    fn assert_vec3a_near(actual: Vec3A, expected: Vec3A) {
        let diff = (actual - expected).abs();
        assert!(
            diff.cmple(Vec3A::splat(1e-5)).all(),
            "actual={actual:?} expected={expected:?} diff={diff:?}"
        );
    }
}
