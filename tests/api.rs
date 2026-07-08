use replay_to_rocketsim::{
    BoostPickupKind, BoostPickupSource, DemoKind, FrameArenaEvent, FrameBoostPickupEvent,
    FrameDemoEvent, FrameReplayEvent, REPLAY_HZ, ROCKETSIM_HZ, ROCKETSIM_TICKS_PER_REPLAY_FRAME,
    ReplayFrameMetadata, replay_frame_to_rocketsim_tick, replay_time_to_rocketsim_tick,
};

#[test]
fn exposes_expected_replay_to_rocketsim_tick_rate_constants() {
    assert_eq!(REPLAY_HZ, 30);
    assert_eq!(ROCKETSIM_HZ, 120);
    assert_eq!(ROCKETSIM_TICKS_PER_REPLAY_FRAME, 4);
}

#[test]
fn maps_public_nominal_replay_frame_indices_to_rocketsim_ticks() {
    assert_eq!(replay_frame_to_rocketsim_tick(0), 0);
    assert_eq!(replay_frame_to_rocketsim_tick(1), 4);
    assert_eq!(replay_frame_to_rocketsim_tick(450), 1800);
}

#[test]
fn maps_public_replay_times_to_rocketsim_ticks() {
    assert_eq!(replay_time_to_rocketsim_tick(0.0), 0);
    assert_eq!(replay_time_to_rocketsim_tick(1.0 / 60.0), 2);
    assert_eq!(replay_time_to_rocketsim_tick(1.0), 120);
}

#[test]
fn exposes_typed_per_frame_rocketsim_events() {
    let event = FrameArenaEvent {
        tick: 123,
        event: replay_to_rocketsim::rocketsim::ArenaEvent::CarPickupBoost(
            replay_to_rocketsim::rocketsim::CarPickupBoostEvent {
                car_idx: 0,
                boost_pad_idx: 5,
            },
        ),
    };

    assert_eq!(event.tick, 123);
    assert!(matches!(
        event.event,
        replay_to_rocketsim::rocketsim::ArenaEvent::CarPickupBoost(_)
    ));
}

#[test]
fn exposes_typed_per_frame_replay_events() {
    let demo = FrameReplayEvent::Demo(FrameDemoEvent {
        frame: 10,
        time: 0.333,
        kind: DemoKind::Extended,
        attacker_car_actor_id: Some(1),
        attacker_pri_actor_id: Some(2),
        attacker_unique_id: None,
        victim_car_actor_id: Some(3),
        victim_pri_actor_id: Some(4),
        victim_unique_id: None,
        self_demo: Some(false),
    });
    let boost_pickup = FrameReplayEvent::BoostPickup(FrameBoostPickupEvent {
        frame: 11,
        time: 0.366,
        boost_pad_actor_id: 20,
        boost_pad_index: Some(5),
        boost_pad_is_big: Some(true),
        boost_pad_pos: None,
        nearest_boost_pad_distance: Some(0.0),
        instigator_car_actor_id: Some(1),
        instigator_car_idx: Some(0),
        instigator_pri_actor_id: Some(2),
        instigator_unique_id: None,
        instigator_player_name: Some("player".to_owned()),
        instigator_team: None,
        instigator_boost_amount: Some(12.0),
        kind: BoostPickupKind::PickedUp,
        source: BoostPickupSource::ReplicatedPickupData,
    });

    let metadata = ReplayFrameMetadata {
        events: vec![demo, boost_pickup],
        ..ReplayFrameMetadata::default()
    };

    assert_eq!(metadata.events.len(), 2);
    assert!(matches!(metadata.events[0], FrameReplayEvent::Demo(_)));
    assert!(matches!(
        metadata.events[1],
        FrameReplayEvent::BoostPickup(FrameBoostPickupEvent {
            kind: BoostPickupKind::PickedUp,
            boost_pad_index: Some(5),
            ..
        })
    ));
}
