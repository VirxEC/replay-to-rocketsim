use boxcars::attributes::{RemoteId, UniqueId};
use rocketsim::{CarBodyConfig, Team, Vec3A};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlayerUniqueId {
    pub system_id: u8,
    pub remote_id: PlayerRemoteId,
    pub local_id: u8,
}

impl PlayerUniqueId {
    #[must_use]
    pub fn platform(&self) -> &'static str {
        self.remote_id.platform()
    }

    #[must_use]
    pub fn stable_key(&self) -> String {
        format!(
            "{}:{}:{}",
            self.platform(),
            self.remote_id.stable_value(),
            self.local_id
        )
    }
}

impl From<&UniqueId> for PlayerUniqueId {
    fn from(value: &UniqueId) -> Self {
        Self {
            system_id: value.system_id,
            remote_id: PlayerRemoteId::from(&value.remote_id),
            local_id: value.local_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PlayerRemoteId {
    PlayStation { online_id: u64, name: String },
    PsyNet { online_id: u64 },
    SplitScreen(u32),
    Steam(u64),
    Switch { online_id: u64 },
    Xbox(u64),
    QQ(u64),
    Epic(String),
}

impl PlayerRemoteId {
    #[must_use]
    pub fn platform(&self) -> &'static str {
        match self {
            Self::PlayStation { .. } => "PlayStation",
            Self::PsyNet { .. } => "PsyNet",
            Self::SplitScreen(_) => "SplitScreen",
            Self::Steam(_) => "Steam",
            Self::Switch { .. } => "Switch",
            Self::Xbox(_) => "Xbox",
            Self::QQ(_) => "QQ",
            Self::Epic(_) => "Epic",
        }
    }

    fn stable_value(&self) -> String {
        match self {
            Self::PlayStation { online_id, name } => format!("{online_id}:{name}"),
            Self::PsyNet { online_id } | Self::Switch { online_id } => online_id.to_string(),
            Self::SplitScreen(id) => id.to_string(),
            Self::Steam(id) | Self::Xbox(id) | Self::QQ(id) => id.to_string(),
            Self::Epic(id) => id.clone(),
        }
    }
}

impl From<&RemoteId> for PlayerRemoteId {
    fn from(value: &RemoteId) -> Self {
        match value {
            RemoteId::PlayStation(id) => Self::PlayStation {
                online_id: id.online_id,
                name: id.name.clone(),
            },
            RemoteId::PsyNet(id) => Self::PsyNet {
                online_id: id.online_id,
            },
            RemoteId::SplitScreen(id) => Self::SplitScreen(*id),
            RemoteId::Steam(id) => Self::Steam(*id),
            RemoteId::Switch(id) => Self::Switch {
                online_id: id.online_id,
            },
            RemoteId::Xbox(id) => Self::Xbox(*id),
            RemoteId::QQ(id) => Self::QQ(*id),
            RemoteId::Epic(id) => Self::Epic(id.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerMetadata {
    pub pri_actor_id: i32,
    pub unique_id: Option<PlayerUniqueId>,
    pub name: Option<String>,
    pub team: Option<Team>,
    pub player_id: Option<i32>,
    pub score: Option<i32>,
    pub is_bot: Option<bool>,
    pub is_spectator: Option<bool>,
    pub match_score: Option<i32>,
    pub match_goals: Option<i32>,
    pub match_assists: Option<i32>,
    pub match_saves: Option<i32>,
    pub match_shots: Option<i32>,
    pub match_demolishes: Option<i32>,
    pub car_demolitions: Option<i32>,
    pub self_demolitions: Option<i32>,
    pub ping: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TeamMetadata {
    pub actor_id: i32,
    pub team: Team,
    pub score: Option<i32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FrameRigidBodyMetadata {
    pub is_sleeping: Option<bool>,
    pub pos_update_age: Option<f32>,
    pub rot_update_age: Option<f32>,
    pub vel_update_age: Option<f32>,
    pub ang_vel_update_age: Option<f32>,
    pub sleeping_update_age: Option<f32>,
    pub any_phys_update_age: Option<f32>,
    pub is_repeat: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FrameCarMetadata {
    pub car_idx: usize,
    pub car_actor_id: i32,
    pub pri_actor_id: Option<i32>,
    pub unique_id: Option<PlayerUniqueId>,
    pub player_name: Option<String>,
    pub team: Team,
    pub body_product_id: Option<u32>,
    pub body_config: CarBodyConfig,
    pub rigid_body: FrameRigidBodyMetadata,
    pub input_restriction: Option<u8>,
    pub is_driving: Option<bool>,
    pub is_demoed: bool,
    pub demo_respawn_timer: f32,
    pub jump_is_active: Option<bool>,
    pub dodge_is_active: Option<bool>,
    pub dodge_torque: Option<Vec3A>,
    pub dodge_impulse: Option<Vec3A>,
    pub double_jump_is_active: Option<bool>,
    pub double_jump_impulse: Option<Vec3A>,
    pub flip_car_is_active: Option<bool>,
    pub flip_car_time: Option<f32>,
    pub flip_car_right: Option<bool>,
    pub dodges_refreshed_counter: Option<i32>,
    pub unlimited_jumps: Option<bool>,
    pub unlimited_time_for_dodge: Option<bool>,
    pub boost_modifier: Option<f32>,
    pub boost_restriction: Option<u8>,
    pub boost_recharge_delay: Option<f32>,
    pub boost_recharge_rate: Option<f32>,
    pub boost_unlimited_ref_count: Option<i32>,
    pub no_boost: Option<bool>,
    pub unlimited_boost: Option<bool>,
    pub recharge_ground_only: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayGameMetadata {
    pub id: Option<String>,
    pub replay_version: i32,
    pub num_frames: Option<i32>,
    pub replay_name: Option<String>,
    pub map_name: Option<String>,
    pub date: Option<String>,
    pub match_type: Option<String>,
    pub team_0_score: Option<i32>,
    pub team_1_score: Option<i32>,
    pub goals: Vec<ReplayGoal>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayGoal {
    pub frame: i32,
    pub player_name: Option<String>,
    pub is_orange: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameGameEvent {
    pub seconds_remaining: Option<i32>,
    pub replicated_game_state_time_remaining: Option<i32>,
    pub is_overtime: Option<bool>,
    pub ball_has_been_hit: Option<bool>,
    pub game_mode: Option<ReplayGameMode>,
    pub replicated_state_name: Option<i32>,
    pub replicated_state_index: Option<u8>,
    pub scored_on_team: Option<u8>,
    pub match_ended: Option<bool>,
    pub playlist: Option<i32>,
    pub mutator_index: Option<i32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FrameBallMetadata {
    pub rigid_body: FrameRigidBodyMetadata,
    pub hit_team_num: Option<u8>,
    pub scale: Option<f32>,
    pub gravity_scale: Option<f32>,
    pub max_linear_speed_scale: Option<f32>,
    pub world_bounce_scale: Option<f32>,
    pub added_car_bounce_scale: Option<f32>,
    pub hit_spin_scale: Option<f32>,
    pub air_resistance: Option<Vec3A>,
    pub warn_ball_reset: Option<bool>,
    /// Ball is sleeping and not at the origin — indicates a goal was scored.
    /// `true` when `is_sleeping == Some(true) && pos != (0, 0, *)`.
    /// Used to detect goal frames earlier than the replay metadata claims.
    /// Defaults to `false` when ball rigid-body data has not been received yet.
    pub ball_goal_sleep: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameScore {
    pub blue: Option<i32>,
    pub orange: Option<i32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ReplayGameMode {
    #[default]
    Unknown,
    Soccar,
    Hoops,
    Heatseeker,
    Snowday,
    Dropshot,
    Other(u8, u8),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReplayFrameMetadata {
    pub game_event: FrameGameEvent,
    pub ball: FrameBallMetadata,
    pub score: FrameScore,
    pub mutators: ReplayMutatorMetadata,
    pub events: Vec<FrameReplayEvent>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FrameReplayEvent {
    Demo(FrameDemoEvent),
    BoostPickup(FrameBoostPickupEvent),
    CarAction(FrameCarActionEvent),
    CarLifecycle(FrameCarLifecycleEvent),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemoKind {
    Standard,
    Extended,
    CustomFx,
    GoalExplosion,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FrameDemoEvent {
    pub frame: usize,
    pub time: f32,
    pub kind: DemoKind,
    pub attacker_car_actor_id: Option<i32>,
    pub attacker_pri_actor_id: Option<i32>,
    pub attacker_unique_id: Option<PlayerUniqueId>,
    pub victim_car_actor_id: Option<i32>,
    pub victim_pri_actor_id: Option<i32>,
    pub victim_unique_id: Option<PlayerUniqueId>,
    pub self_demo: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoostPickupKind {
    PickedUp,
    Released,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FrameBoostPickupEvent {
    pub frame: usize,
    pub time: f32,
    pub boost_pad_actor_id: i32,
    pub boost_pad_index: Option<usize>,
    pub boost_pad_is_big: Option<bool>,
    pub boost_pad_pos: Option<Vec3A>,
    pub nearest_boost_pad_distance: Option<f32>,
    pub instigator_car_actor_id: Option<i32>,
    pub instigator_car_idx: Option<usize>,
    pub instigator_pri_actor_id: Option<i32>,
    pub instigator_unique_id: Option<PlayerUniqueId>,
    pub instigator_player_name: Option<String>,
    pub instigator_team: Option<Team>,
    pub instigator_boost_amount: Option<f32>,
    pub kind: BoostPickupKind,
    pub source: BoostPickupSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoostPickupSource {
    ReplicatedPickupData,
    NewReplicatedPickupData,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FrameCarActionEvent {
    pub frame: usize,
    pub time: f32,
    pub car_actor_id: i32,
    pub car_idx: Option<usize>,
    pub pri_actor_id: Option<i32>,
    pub unique_id: Option<PlayerUniqueId>,
    pub player_name: Option<String>,
    pub team: Option<Team>,
    pub action: CarActionKind,
    pub previous: bool,
    pub current: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CarActionKind {
    Jump,
    Boost,
    Handbrake,
    Dodge,
    DoubleJump,
    FlipCar,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FrameCarLifecycleEvent {
    pub frame: usize,
    pub time: f32,
    pub car_actor_id: i32,
    pub car_idx: Option<usize>,
    pub pri_actor_id: Option<i32>,
    pub unique_id: Option<PlayerUniqueId>,
    pub player_name: Option<String>,
    pub team: Option<Team>,
    pub kind: CarLifecycleKind,
    pub demo_respawn_timer: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CarLifecycleKind {
    DemoStarted,
    RespawnEnded,
    CarActorSpawned,
    CarActorDeleted,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ReplayMutatorMetadata {
    pub ball_scale: Option<f32>,
    pub ball_gravity_scale: Option<f32>,
    pub ball_max_linear_speed_scale: Option<f32>,
    pub boost_recharge_delay: Option<f32>,
    pub boost_recharge_rate: Option<f32>,
    pub unlimited_boost: Option<bool>,
    pub no_boost: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameplayPeriod {
    pub start_frame: usize,
    pub end_frame: usize,
    pub first_hit_frame: usize,
    pub goal_frame: Option<usize>,
}
