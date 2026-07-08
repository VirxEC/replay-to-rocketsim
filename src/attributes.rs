pub(crate) const RB_STATE_ATTR: &str = "TAGame.RBActor_TA:ReplicatedRBState";
pub(crate) const CAR_THROTTLE_ATTR: &str = "TAGame.Vehicle_TA:ReplicatedThrottle";
pub(crate) const CAR_STEER_ATTR: &str = "TAGame.Vehicle_TA:ReplicatedSteer";
pub(crate) const CAR_HANDBRAKE_ATTR: &str = "TAGame.Vehicle_TA:bReplicatedHandbrake";
pub(crate) const CAR_COMPONENT_VEHICLE_ATTR: &str = "TAGame.CarComponent_TA:Vehicle";
pub(crate) const CAR_COMPONENT_ACTIVE_ATTR: &str = "TAGame.CarComponent_TA:ReplicatedActive";
pub(crate) const CAR_BOOST_AMOUNT_ATTR: &str = "TAGame.CarComponent_Boost_TA:ReplicatedBoostAmount";
pub(crate) const CAR_REPLICATED_BOOST_ATTR: &str = "TAGame.CarComponent_Boost_TA:ReplicatedBoost";
pub(crate) const CAR_PRI_ATTR: &str = "Engine.Pawn:PlayerReplicationInfo";
pub(crate) const PRI_TEAM_ATTR: &str = "Engine.PlayerReplicationInfo:Team";
pub(crate) const PRI_CLIENT_LOADOUT_ATTR: &str = "TAGame.PRI_TA:ClientLoadout";
pub(crate) const PRI_CLIENT_LOADOUTS_ATTR: &str = "TAGame.PRI_TA:ClientLoadouts";
pub(crate) const CAR_DEMOLISH_ATTR: &str = "TAGame.Car_TA:ReplicatedDemolish";
pub(crate) const CAR_DEMOLISH_EXTENDED_ATTR: &str = "TAGame.Car_TA:ReplicatedDemolishExtended";
pub(crate) const CAR_DEMOLISH_CUSTOM_FX_ATTR: &str = "TAGame.Car_TA:ReplicatedDemolish_CustomFX";
pub(crate) const CAR_DEMOLISH_GOAL_EXPLOSION_ATTR: &str =
    "TAGame.Car_TA:ReplicatedDemolishGoalExplosion";
pub(crate) const CAR_INPUT_RESTRICTION_ATTR: &str = "TAGame.Vehicle_TA:InputRestriction";
pub(crate) const CAR_IS_DRIVING_ATTR: &str = "TAGame.Vehicle_TA:bDriving";
pub(crate) const CAR_DODGES_REFRESHED_COUNTER_ATTR: &str = "TAGame.Car_TA:DodgesRefreshedCounter";
pub(crate) const CAR_UNLIMITED_JUMPS_ATTR: &str = "TAGame.Car_TA:bUnlimitedJumps";
pub(crate) const CAR_UNLIMITED_TIME_FOR_DODGE_ATTR: &str = "TAGame.Car_TA:bUnlimitedTimeForDodge";
pub(crate) const DODGE_TORQUE_ATTR: &str = "TAGame.CarComponent_Dodge_TA:DodgeTorque";
pub(crate) const DODGE_IMPULSE_ATTR: &str = "TAGame.CarComponent_Dodge_TA:DodgeImpulse";
pub(crate) const DOUBLE_JUMP_IMPULSE_ATTR: &str =
    "TAGame.CarComponent_DoubleJump_TA:DoubleJumpImpulse";
pub(crate) const FLIP_CAR_RIGHT_ATTR: &str = "TAGame.CarComponent_FlipCar_TA:bFlipRight";
pub(crate) const FLIP_CAR_TIME_ATTR: &str = "TAGame.CarComponent_FlipCar_TA:FlipCarTime";
pub(crate) const PICKUP_ATTR: &str = "TAGame.VehiclePickup_TA:ReplicatedPickupData";
pub(crate) const PICKUP_NEW_ATTR: &str = "TAGame.VehiclePickup_TA:NewReplicatedPickupData";

pub(crate) const BALL_HIT_TEAM_NUM_ATTR: &str = "TAGame.Ball_TA:HitTeamNum";
pub(crate) const BALL_SCALE_ATTR: &str = "TAGame.Ball_TA:ReplicatedBallScale";
pub(crate) const BALL_GRAVITY_SCALE_ATTR: &str = "TAGame.Ball_TA:ReplicatedBallGravityScale";
pub(crate) const BALL_MAX_SPEED_SCALE_ATTR: &str =
    "TAGame.Ball_TA:ReplicatedBallMaxLinearSpeedScale";
pub(crate) const BALL_WORLD_BOUNCE_SCALE_ATTR: &str = "TAGame.Ball_TA:ReplicatedWorldBounceScale";
pub(crate) const BALL_ADDED_CAR_BOUNCE_SCALE_ATTR: &str =
    "TAGame.Ball_TA:ReplicatedAddedCarBounceScale";
pub(crate) const BALL_HIT_SPIN_SCALE_ATTR: &str = "TAGame.Ball_TA:BallHitSpinScale";
pub(crate) const BALL_AIR_RESISTANCE_ATTR: &str = "TAGame.Ball_TA:AirResistance";
pub(crate) const BALL_WARN_RESET_ATTR: &str = "TAGame.Ball_TA:bWarnBallReset";

pub(crate) const GAME_SECONDS_REMAINING_ATTR: &str = "TAGame.GameEvent_Soccar_TA:SecondsRemaining";
pub(crate) const GAME_STATE_TIME_REMAINING_ATTR: &str =
    "TAGame.GameEvent_TA:ReplicatedGameStateTimeRemaining";
pub(crate) const GAME_IS_OVERTIME_ATTR: &str = "TAGame.GameEvent_Soccar_TA:bOverTime";
pub(crate) const GAME_BALL_HAS_BEEN_HIT_ATTR: &str = "TAGame.GameEvent_Soccar_TA:bBallHasBeenHit";
pub(crate) const GAME_MODE_ATTR: &str = "TAGame.GameEvent_TA:GameMode";
pub(crate) const GAME_STATE_NAME_ATTR: &str = "TAGame.GameEvent_TA:ReplicatedStateName";
pub(crate) const GAME_STATE_INDEX_ATTR: &str = "TAGame.GameEvent_TA:ReplicatedStateIndex";
pub(crate) const GAME_SCORED_ON_TEAM_ATTR: &str =
    "TAGame.GameEvent_Soccar_TA:ReplicatedScoredOnTeam";
pub(crate) const GAME_MATCH_ENDED_ATTR: &str = "TAGame.GameEvent_Soccar_TA:bMatchEnded";
pub(crate) const GRI_PLAYLIST_ATTR: &str = "ProjectX.GRI_X:ReplicatedGamePlaylist";
pub(crate) const GRI_MUTATOR_INDEX_ATTR: &str = "ProjectX.GRI_X:ReplicatedGameMutatorIndex";

pub(crate) const TEAM_SCORE_ATTR: &str = "Engine.TeamInfo:Score";

pub(crate) const PRI_UNIQUE_ID_ATTR: &str = "Engine.PlayerReplicationInfo:UniqueId";
pub(crate) const PRI_PLAYER_NAME_ATTR: &str = "Engine.PlayerReplicationInfo:PlayerName";
pub(crate) const PRI_MATCH_SCORE_ATTR: &str = "TAGame.PRI_TA:MatchScore";
pub(crate) const PRI_MATCH_GOALS_ATTR: &str = "TAGame.PRI_TA:MatchGoals";
pub(crate) const PRI_MATCH_ASSISTS_ATTR: &str = "TAGame.PRI_TA:MatchAssists";
pub(crate) const PRI_MATCH_SAVES_ATTR: &str = "TAGame.PRI_TA:MatchSaves";
pub(crate) const PRI_MATCH_SHOTS_ATTR: &str = "TAGame.PRI_TA:MatchShots";
pub(crate) const PRI_PING_ATTR: &str = "Engine.PlayerReplicationInfo:Ping";
pub(crate) const PRI_PLAYER_ID_ATTR: &str = "Engine.PlayerReplicationInfo:PlayerID";
pub(crate) const PRI_SCORE_ATTR: &str = "Engine.PlayerReplicationInfo:Score";
pub(crate) const PRI_IS_BOT_ATTR: &str = "Engine.PlayerReplicationInfo:bBot";
pub(crate) const PRI_IS_SPECTATOR_ATTR: &str = "Engine.PlayerReplicationInfo:bIsSpectator";
pub(crate) const PRI_MATCH_DEMOLISHES_ATTR: &str = "TAGame.PRI_TA:MatchDemolishes";
pub(crate) const PRI_CAR_DEMOLITIONS_ATTR: &str = "TAGame.PRI_TA:CarDemolitions";
pub(crate) const PRI_SELF_DEMOLITIONS_ATTR: &str = "TAGame.PRI_TA:SelfDemolitions";

pub(crate) const BOOST_MODIFIER_ATTR: &str = "TAGame.CarComponent_Boost_TA:BoostModifier";
pub(crate) const BOOST_RESTRICTION_ATTR: &str = "TAGame.CarComponent_Boost_TA:BoostRestriction";
pub(crate) const BOOST_RECHARGE_DELAY_ATTR: &str = "TAGame.CarComponent_Boost_TA:RechargeDelay";
pub(crate) const BOOST_RECHARGE_RATE_ATTR: &str = "TAGame.CarComponent_Boost_TA:RechargeRate";
pub(crate) const BOOST_UNLIMITED_REF_COUNT_ATTR: &str =
    "TAGame.CarComponent_Boost_TA:UnlimitedBoostRefCount";
pub(crate) const BOOST_NO_BOOST_ATTR: &str = "TAGame.CarComponent_Boost_TA:bNoBoost";
pub(crate) const BOOST_UNLIMITED_ATTR: &str = "TAGame.CarComponent_Boost_TA:bUnlimitedBoost";
pub(crate) const BOOST_RECHARGE_GROUND_ONLY_ATTR: &str =
    "TAGame.CarComponent_Boost_TA:bRechargeGroundOnly";

pub(crate) const PRI_CLIENT_LOADOUT_ONLINE_ATTR: &str = "TAGame.PRI_TA:ClientLoadoutOnline";
pub(crate) const PRI_CLIENT_LOADOUTS_ONLINE_ATTR: &str = "TAGame.PRI_TA:ClientLoadoutsOnline";
