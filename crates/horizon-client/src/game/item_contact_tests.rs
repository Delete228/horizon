//! Run separately: installs process-wide trait mods and needs private resources.
use super::*;
use crate::mod_api::{self, Flow, Hooks, Host, ItemAfterVerticalCollisionEvent};
use horizon_client::{chunk::Chunk, join_seq::Joined};
use horizon_world::nbt::Tag;
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicU8, Ordering},
};

#[derive(Default)]
struct ContactProbe {
    mode: AtomicU8,
    calls: Mutex<Vec<(bool, u32, [i32; 3], [f64; 3], u32)>>,
}
struct LaterContact(Arc<Mutex<Vec<u32>>>);
type RawContact = (Arc<ContactProbe>, Arc<Mutex<Vec<u32>>>);
static RAW_CONTACT: OnceLock<RawContact> = OnceLock::new();

/// The official-weave dev harness calls this from a raw hook with no globally
/// installed trait mods. It shares assertions/effects, not registration.
pub(super) fn raw_contact(
    game: &mut Game<'_>,
    dimension: &str,
    client: bool,
    entity: u32,
    state: horizon_world::registry::BlockStateId,
    block_pos: [i32; 3],
    item_pos: [f64; 3],
    stack: &mut ItemStack,
    discard: &mut bool,
) {
    let Some((probe, later)) = RAW_CONTACT.get() else {
        return;
    };
    assert!(mod_api::mods().is_empty());
    let mut event = ItemAfterVerticalCollisionEvent {
        dimension,
        client,
        entity,
        state,
        block_pos,
        item_pos,
        stack,
    };
    if matches!(
        probe.item_after_vertical_collision(game, &mut event),
        Flow::Cancel
    ) {
        *discard = true;
    } else {
        later.lock().unwrap().push(event.stack.count);
    }
}
impl Hooks for LaterContact {
    fn item_after_vertical_collision(
        &self,
        _: &mut dyn Host,
        event: &mut ItemAfterVerticalCollisionEvent<'_>,
    ) -> Flow {
        self.0.lock().unwrap().push(event.stack.count);
        Flow::Continue
    }
}
impl Hooks for Arc<ContactProbe> {
    fn item_after_vertical_collision(
        &self,
        host: &mut dyn Host,
        event: &mut ItemAfterVerticalCollisionEvent<'_>,
    ) -> Flow {
        assert_eq!(event.dimension, "minecraft:overworld");
        self.calls.lock().unwrap().push((
            event.client,
            event.entity,
            event.block_pos,
            event.item_pos,
            event.stack.count,
        ));
        match self.mode.load(Ordering::Relaxed) {
            1 => {
                event.stack.count -= 1;
            }
            2 => return Flow::Cancel,
            3 => event.stack.count = 0,
            4 => {
                assert!(
                    host.summon(
                        event.dimension,
                        "minecraft:item",
                        [4.5, 68.0, 4.5],
                        [(
                            "Item".into(),
                            Tag::compound([
                                ("id", Tag::String("minecraft:diamond".into())),
                                ("count", Tag::Int(1))
                            ])
                        )]
                        .into()
                    )
                    .is_some()
                );
            }
            5 if !event.client => return Flow::Cancel,
            6 => {
                if let Some(rest) =
                    host.insert_item(event.block_pos, Direction::Up, event.stack.clone())
                {
                    *event.stack = rest;
                } else {
                    return Flow::Cancel;
                }
            }
            _ => {}
        }
        Flow::Continue
    }
}
#[test]
#[ignore = "private prepared resources; process-wide trait probe, run alone"]
fn actual_owned_item_contact_stack_discard_world_and_callback_spawns() {
    let _ = raw_contact; // Also type-checked when the official-weave dev harness is absent.
    let root = PathBuf::from(std::env::var_os("HORIZON_TEST_RESOURCES").unwrap());
    let settings = Settings::default();
    let loaded = load_resources(
        root.clone(),
        None,
        &root,
        &settings,
        Instant::now(),
        false,
        true,
        &Progress::new(),
        &mut |_, _| unreachable!(),
    )
    .unwrap();
    let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
    options.resources = root.clone();
    options.execution_role = execution_role::ExecutionRole::IntegratedServer;
    let mut game = Game::new(
        &loaded.resources,
        loaded.pack,
        loaded.gui.unwrap(),
        options,
        settings,
        Audio::silent(&root),
        None,
    )
    .unwrap();
    game.options.dedicated = Some(Vec::new()); // same authority guard as the real server
    game.world = World::new(game.res.facts().air, 1);
    game.world
        .chunks
        .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
    let barrel = game
        .pack
        .assets
        .registry
        .default_state("minecraft:barrel")
        .unwrap();
    assert!(game.place_state([4, 63, 4], barrel, true).unwrap());
    let probe = Arc::new(ContactProbe::default());
    let later = Arc::new(Mutex::new(Vec::new()));
    if std::env::var_os("HORIZON_ITEM_CONTACT_RAW_PROBE").is_some() {
        assert!(mod_api::mods().is_empty());
        assert!(RAW_CONTACT.set((probe.clone(), later.clone())).is_ok());
    } else {
        assert!(
            mod_api::MODS
                .set(Arc::from(vec![
                    Box::new(probe.clone()) as Box<dyn Hooks>,
                    Box::new(LaterContact(later.clone()))
                ]))
                .is_ok()
        );
    }
    let mut input = ItemStack::new("minecraft:iron_ingot", 13);
    input.data_mut().set_other(
        "fixture:opaque",
        Some(Tag::compound([("retain", Tag::Long(99))])),
    );
    let spawn = |game: &mut Game<'_>| {
        game.dropped.clear();
        game.spawn_item(input.clone(), [4.5, 64.1, 4.5], [0.0, -0.3, 0.0], 20);
        game.ticked_entities_through = game.next_entity;
        game.dropped.last().unwrap().clone()
    };
    // A refusal changes neither the actual stack/components nor its identity.
    let old = spawn(&mut game);
    game.tick_items();
    let item = &game.dropped[0];
    assert_eq!(item.id, old.id);
    assert_eq!(item.join_seq(), old.join_seq());
    assert_eq!(item.stack, input);
    assert_eq!(item.position, Vec3::new(4.5, 64.0, 4.5));
    assert_eq!(
        probe.calls.lock().unwrap()[0],
        (false, old.id, [4, 63, 4], [4.5, 64.0, 4.5], 13)
    );
    // Grounded retries follow actual movement cadence even when a raw hook is
    // the only subscriber. Keep the entity's ordinary ID/join number intact.
    let resting = spawn(&mut game);
    game.dropped[0].position.y = 64.0;
    game.dropped[0].velocity = Vec3::ZERO;
    game.dropped[0].on_ground = true;
    let mut ticks = Vec::new();
    for tick in 1..=8_u32 {
        let calls = probe.calls.lock().unwrap().len();
        game.tick_items();
        if probe.calls.lock().unwrap().len() > calls {
            ticks.push(tick);
        }
    }
    assert_eq!(
        ticks,
        (1..=8_u32)
            .filter(|t| t.wrapping_add(resting.id).is_multiple_of(4))
            .collect::<Vec<_>>()
    );
    probe.mode.store(1, Ordering::Relaxed);
    let old = spawn(&mut game);
    game.tick_items();
    assert_eq!(game.dropped[0].stack.count, 12);
    assert!(game.dropped[0].stack.same_item_same_components(&input));
    assert_eq!(game.dropped[0].id, old.id);
    assert_eq!(
        later.lock().unwrap().last(),
        Some(&12),
        "later hooks see the live stack mutation"
    );
    for mode in [2, 3] {
        probe.mode.store(mode, Ordering::Relaxed);
        spawn(&mut game);
        let before = later.lock().unwrap().len();
        game.tick_items();
        assert!(game.dropped.is_empty());
        assert!(game.orbs.is_empty());
        assert_eq!(
            later.lock().unwrap().len(),
            before + usize::from(mode == 3),
            "explicit discard skips later hooks; empty-stack removal follows normal mutation dispatch"
        );
    }
    // Normal Host/container path from the synchronous callback, with a real
    // full/partial/empty container rather than an admission stub.
    probe.mode.store(6, Ordering::Relaxed);
    let mut fill = input.clone();
    fill.count = 27 * 64 - 2;
    assert!(
        game.native_insert_item([4, 63, 4], Direction::Up, fill)
            .is_none()
    );
    spawn(&mut game);
    game.tick_items();
    assert_eq!(game.dropped[0].stack.count, 11);
    assert!(game.dropped[0].stack.same_item_same_components(&input));
    let slots = game
        .native_container_slots("minecraft:overworld", [4, 63, 4], Direction::Up)
        .unwrap();
    assert_eq!(
        slots
            .iter()
            .filter_map(|s| s.stack.as_ref())
            .map(|s| s.count)
            .sum::<u32>(),
        27 * 64
    );
    let old = spawn(&mut game);
    game.tick_items();
    assert_eq!(
        game.dropped[0].stack, old.stack,
        "full container refuses without loss"
    );
    assert!(game.place_state([4, 63, 4], game.world.air, true).unwrap());
    assert!(game.place_state([4, 63, 4], barrel, true).unwrap());
    // Remove loot created by replacing the full barrel; this is fixture setup.
    spawn(&mut game);
    game.tick_items();
    assert!(game.dropped.is_empty());
    assert_eq!(
        game.native_container_slots("minecraft:overworld", [4, 63, 4], Direction::Up)
            .unwrap()
            .iter()
            .filter_map(|s| s.stack.as_ref())
            .map(|s| s.count)
            .sum::<u32>(),
        13
    );
    // New drops survive all three owner exit paths, retain join order, and
    // remain newborn until the next tick. No recursive movement is performed.
    for player_mode in [None, Some(0), Some(3)] {
        game.player = player_mode
            .map(|_| PlayerMotion::new(game.motion, Vec3::new(12.0, 70.0, 12.0)).unwrap());
        if let Some(mode) = player_mode {
            game.set_mode_id(mode);
        }
        probe.mode.store(4, Ordering::Relaxed);
        let old = spawn(&mut game);
        game.tick_items();
        assert_eq!(game.dropped.len(), 2);
        assert_eq!(game.dropped[0].id, old.id);
        assert!(game.dropped[0].join_seq() < game.dropped[1].join_seq());
        assert_eq!(game.dropped[1].stack.item.as_ref(), "minecraft:diamond");
        assert_eq!(game.dropped[1].age, 0);
        assert_eq!(game.dropped[1].ticks, 0);
        assert_eq!(game.dropped[1].position, Vec3::new(4.5, 68.0, 4.5));
        probe.mode.store(0, Ordering::Relaxed);
        game.ticked_entities_through = game.next_entity;
        game.tick_items();
        assert_eq!(game.dropped[1].age, 1);
    }
    // Normal Remote constructor and normal client tick, without a joined
    // handshake. Prediction can mutate/discard; a server-only handler declines.
    use horizon_protocol::connection::Pipe;
    game.options.dedicated = None;
    game.options.execution_role = execution_role::ExecutionRole::Client;
    game.world_tables = None;
    let (client, server) = Pipe::pair();
    game.connect_local(client, "ItemContactTest").unwrap();
    game.world = World::new(game.res.facts().air, 1);
    game.world
        .chunks
        .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
    game.world.set(4, 63, 4, barrel).unwrap();
    for (mode, discard, count) in [
        (1, false, 12),
        (2, true, 13),
        (5, false, 13),
        (6, false, 13),
    ] {
        probe.mode.store(mode, Ordering::Relaxed);
        spawn(&mut game);
        game.tick_remote().unwrap();
        assert_eq!(game.dropped.is_empty(), discard);
        if !discard {
            assert_eq!(game.dropped[0].stack.count, count);
        }
        assert!(probe.calls.lock().unwrap().last().unwrap().0);
    }
    server.shutdown();
    if RAW_CONTACT.get().is_some() {
        assert!(mod_api::mods().is_empty());
    }
}
