use boxcars::{ActorId, ObjectId};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConvertError {
    #[error("failed to parse replay: {0}")]
    Parse(#[from] boxcars::ParseError),

    #[error("replay does not contain parsed network frames")]
    MissingNetworkFrames,

    #[error("replay is missing integer ReplayVersion header property")]
    MissingReplayVersion,

    #[error("object id {0} is outside replay object table")]
    InvalidObjectId(ObjectId),

    #[error("updated actor {0} does not exist at replay frame")]
    MissingActor(ActorId),

    #[error("actor {0} has no initial location and no replicated rigid-body state")]
    MissingRigidBodyLocation(ActorId),

    #[error("actor {actor_id} has non-finite {field} in replicated rigid-body state")]
    NonFiniteRigidBody {
        actor_id: ActorId,
        field: &'static str,
    },
}
