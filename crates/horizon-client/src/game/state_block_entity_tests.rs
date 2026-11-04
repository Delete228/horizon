//! Explicit, prepared-resource Game integration. Run this ignored test alone:
//! HORIZON_TEST_RESOURCES=<26.3> HORIZON_API_DISCOVERY=http://127.0.0.1:9
//! cargo test -p horizon-client --lib --no-default-features actual_state_factory_lifecycle -- --ignored --nocapture
//! The fixture installs process-wide trait mods, so it deliberately never runs in the ordinary suite.
use super::{Audio, Game, Options, Progress, Settings, load_resources};
use crate::mod_api::{
    self, BlockEntityEvent, BlockEntityFromStateEvent, Hooks, Host, StatefulBlockEntityEvent,
};
use horizon_client::{
    chunk::Chunk,
    containers::BlockEntity,
    mod_content::{ContentMod, build},
    world::World,
};
use horizon_mods::content::{Block, Content, StateProperty};
use horizon_world::{
    attachments::{Attachments, Holder},
    nbt::{Compound, Tag},
    registry::BlockStateId,
};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

const HEATER: &str = "fixture:heater";
const OTHER: &str = "fixture:other_heater";
const LEGACY: &str = "fixture:legacy";
const ENTITY: &str = "fixture:heater_entity";
const POS: [i32; 3] = [2, 64, 2];
const KEY: (i32, i32, i32) = (2, 64, 2);
struct Factory {
    declarations: Arc<AtomicUsize>,
    constructions: Arc<AtomicUsize>,
    fallbacks: Arc<AtomicUsize>,
}
impl Hooks for Factory {
    fn stateful_block_entity(&self, _: &dyn Host, event: &mut StatefulBlockEntityEvent<'_>) {
        if matches!(event.block, HEATER | OTHER) {
            self.declarations.fetch_add(1, Ordering::SeqCst);
            event.stateful = true;
        }
    }
    fn block_entity_from_state(
        &self,
        event: &mut BlockEntityFromStateEvent<'_>,
    ) -> Option<(String, Compound)> {
        self.constructions.fetch_add(1, Ordering::SeqCst);
        assert!(matches!(event.block, HEATER | OTHER));
        (event.properties.get("filled").is_some_and(|v| v == "true")).then(|| {
            (
                ENTITY.into(),
                Compound::from([("fuel".into(), Tag::Int(1))]),
            )
        })
    }
    fn block_entity(
        &self,
        _: &mut dyn Host,
        event: &mut BlockEntityEvent<'_>,
    ) -> Option<(String, Compound)> {
        if matches!(event.block, HEATER | OTHER | LEGACY) {
            self.fallbacks.fetch_add(1, Ordering::SeqCst);
            Some(("fixture:legacy_entity".into(), Compound::new()))
        } else {
            None
        }
    }
}
fn reset(game: &mut Game<'_>) {
    game.world = World::new(game.res.facts().air, 1);
    game.world
        .chunks
        .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
}
fn fuel(game: &Game<'_>) -> Option<i32> {
    match game.world.block_entities.get(&KEY) {
        Some(BlockEntity::Mod(entity)) => {
            assert_eq!(entity.id, ENTITY);
            match entity.nbt.get(&"fuel".into()) {
                Some(Tag::Int(value)) => Some(*value),
                _ => None,
            }
        }
        _ => None,
    }
}
fn mark(game: &mut Game<'_>) {
    let Some(BlockEntity::Mod(entity)) = game.world.block_entities.get_mut(&KEY) else {
        panic!("no fixture entity");
    };
    entity.nbt.insert("fuel".into(), Tag::Int(617));
}
fn client_state(game: &mut Game<'_>, previous: Option<BlockStateId>, state: BlockStateId) {
    game.world.set(POS[0], POS[1], POS[2], state).unwrap();
    game.sync_block_entity(KEY, previous, state);
}

#[test]
#[ignore = "prepared resources; installs trait mods and must run in its own test process"]
fn actual_state_factory_lifecycle() {
    use super::native_world_edit::{NativeBlockEdit, NativeBlockRun};
    let base = std::path::PathBuf::from(
        std::env::var_os("HORIZON_TEST_RESOURCES").expect("prepared 26.3 resources required"),
    );
    assert!(
        std::env::var("HORIZON_API_DISCOVERY")
            .unwrap_or_default()
            .starts_with("http://127.0.0.1:"),
        "the memory-client fixture must use a local-only discovery endpoint"
    );
    let root = std::env::temp_dir().join(format!(
        "horizon-state-factory-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut content = Content::declared("fixture");
    let blocks: Vec<_> = [HEATER, OTHER, LEGACY]
        .into_iter()
        .map(|id| Block {
            id: id.into(),
            template: "minecraft:stone".into(),
            state_properties: BTreeMap::from([("filled".into(), StateProperty::Boolean)]),
            item: true,
            ..Default::default()
        })
        .collect();
    content.declare(&blocks, &[]).unwrap();
    build(
        &base,
        &[ContentMod {
            id: "fixture".into(),
            content,
        }],
        &root,
    )
    .unwrap();
    let settings = Settings::default();
    let loaded = load_resources(
        root.clone(),
        None,
        &root,
        &settings,
        std::time::Instant::now(),
        false,
        true,
        &Progress::new(),
        &mut |_, _| unreachable!("no menu handoff"),
    )
    .unwrap();
    let registry = loaded.pack.assets.registry;
    let state = |name: &str, filled: bool| {
        let mut state = registry
            .state(registry.default_state(name).unwrap())
            .unwrap()
            .clone();
        state.properties.insert("filled".into(), filled.to_string());
        registry.id(&state).unwrap()
    };
    let empty = state(HEATER, false);
    let full = state(HEATER, true);
    let other = state(OTHER, true);
    let legacy = state(LEGACY, false);
    let stone = registry.default_state("minecraft:stone").unwrap();
    let declarations = Arc::new(AtomicUsize::new(0));
    let constructions = Arc::new(AtomicUsize::new(0));
    let fallbacks = Arc::new(AtomicUsize::new(0));
    let factory = Factory {
        declarations: declarations.clone(),
        constructions: constructions.clone(),
        fallbacks: fallbacks.clone(),
    };
    assert!(
        mod_api::MODS
            .set(Arc::from(vec![Box::new(factory) as Box<dyn Hooks>]))
            .is_ok()
    );
    let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
    options.resources = root.clone();
    options.execution_role = super::execution_role::ExecutionRole::IntegratedServer;
    options.split_guest = true;
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
    let declarations_before = declarations.load(Ordering::SeqCst);
    reset(&mut game);
    assert!(game.new_block_entity_for_state(empty).is_none());
    assert!(matches!(
        game.new_block_entity_for_state(full),
        Some(BlockEntity::Mod(_))
    ));
    assert!(matches!(
        game.new_block_entity_for_state(legacy),
        Some(BlockEntity::Mod(_))
    ));
    assert!(game.new_block_entity_for_state(stone).is_none());
    assert!(matches!(
        game.new_block_entity_for_state(registry.default_state("minecraft:chest").unwrap()),
        Some(BlockEntity::Chest(_))
    ));
    assert_eq!(
        fallbacks.load(Ordering::SeqCst),
        1,
        "declared null never falls back to API15"
    );
    assert_eq!(
        declarations.load(Ordering::SeqCst),
        declarations_before + 2,
        "once per block, not per property state"
    );
    println!(
        "state factory: actual generated hooks, declared null, legacy and vanilla defaults PASS"
    );

    // Real native server writes: a property-only change creates a missing entity,
    // but never replaces an existing same-block entity when fresh creation is null.
    game.options.dedicated = Some(Vec::new());
    let write =
        |game: &mut Game<'_>, state| game.native_set_block("minecraft:overworld", POS, state, 2);
    assert!(write(&mut game, empty));
    assert_eq!(fuel(&game), None);
    assert!(write(&mut game, full));
    assert_eq!(fuel(&game), Some(1));
    mark(&mut game);
    assert!(write(&mut game, empty));
    assert_eq!(fuel(&game), Some(617));
    assert!(write(&mut game, full));
    assert_eq!(fuel(&game), Some(617));
    assert!(write(&mut game, other));
    assert_eq!(fuel(&game), Some(1));
    assert!(write(&mut game, stone));
    assert!(!game.world.block_entities.contains_key(&KEY));
    println!(
        "state factory: native writes create missing / retain same block / replace other block PASS"
    );

    assert!(write(&mut game, empty));
    assert_eq!(fuel(&game), None);
    assert!(game.native_set_block_property(POS, "filled", "true"));
    assert_eq!(fuel(&game), Some(1));
    mark(&mut game);
    assert!(game.native_set_block_property_with(POS, "filled", "false", 2));
    assert_eq!(fuel(&game), Some(617));
    assert!(game.native_set_block_property(POS, "filled", "true"));
    assert_eq!(fuel(&game), Some(617));
    let before = constructions.load(Ordering::SeqCst);
    assert!(!game.native_set_block_property(POS, "filled", "true"));
    assert!(!game.native_set_block_property(POS, "filled", "invalid"));
    assert_eq!(constructions.load(Ordering::SeqCst), before);
    assert!(write(&mut game, stone));
    println!(
        "state factory: public property helpers create missing and retain existing entity PASS"
    );

    assert!(write(&mut game, empty));
    let before = constructions.load(Ordering::SeqCst);
    game.with_level(|b, level, ticks| {
        let at = horizon_core::position::BlockPos::new(POS[0], POS[1], POS[2]);
        assert!(b.set_block(level, ticks, at, full, 2));
        let Some(BlockEntity::Mod(entity)) = level.world.block_entities.get_mut(&KEY) else {
            panic!("set_block must create the entity before the same hook continues");
        };
        assert_eq!(entity.id, ENTITY);
        entity.nbt.insert("fuel".into(), Tag::Int(811));
        assert!(b.set_block(level, ticks, at, empty, 2));
        assert!(level.native_set_block_property(ticks, POS, "filled", "true", 2));
    });
    assert_eq!(fuel(&game), Some(811));
    assert_eq!(
        constructions.load(Ordering::SeqCst),
        before + 1,
        "same-block transitions must not duplicate the constructor"
    );
    assert!(write(&mut game, stone));
    assert!(write(&mut game, empty));
    game.with_level(|_, level, ticks| {
        assert!(level.native_set_block_property(ticks, POS, "filled", "true", 2));
        assert!(
            matches!(
                level.world.block_entities.get(&KEY),
                Some(BlockEntity::Mod(_))
            ),
            "a level-hook property write creates its entity inline"
        );
    });
    assert_eq!(fuel(&game), Some(1));
    assert!(write(&mut game, stone));
    println!(
        "state factory: raw behaviour writes and level-hook property writes create inline PASS"
    );

    // The ordinary BlockItem path must still load supplied components after
    // inline construction. Its initial false state creates none; the item's
    // block-state component changes it to true, then BE data replaces fuel.
    let air = game.world.air;
    assert!(game.place_state(POS, air, true).unwrap());
    game.world.set(POS[0], POS[1] - 1, POS[2], stone).unwrap();
    let mut stack = horizon_client::items::ItemStack::new(HEATER, 1);
    stack.data_mut().set_other(
        "minecraft:block_state",
        Some(Tag::compound([("filled", Tag::String("true".into()))])),
    );
    stack.data_mut().set_other(
        "minecraft:block_entity_data",
        Some(Tag::compound([
            ("id", Tag::String(ENTITY.into())),
            ("fuel", Tag::Int(923)),
        ])),
    );
    game.inventory[0] = Some(stack);
    game.selected = 0;
    let hit = super::Hit {
        pos: [POS[0], POS[1] - 1, POS[2]],
        face: [0, 1, 0],
        point: [
            f64::from(POS[0]) + 0.5,
            f64::from(POS[1]),
            f64::from(POS[2]) + 0.5,
        ],
    };
    assert!(game.use_hand(Some(&hit)).unwrap());
    assert_eq!(game.world.block(POS[0], POS[1], POS[2]), full);
    assert_eq!(fuel(&game), Some(923));
    assert!(write(&mut game, stone));
    println!(
        "state factory: actual BlockItem placement preserves supplied block-state and entity data PASS"
    );

    assert!(game.place_state(POS, empty, true).unwrap());
    assert_eq!(fuel(&game), None);
    assert!(game.place_state(POS, full, true).unwrap());
    mark(&mut game);
    assert!(game.place_state(POS, empty, true).unwrap());
    assert_eq!(fuel(&game), Some(617));
    // A normal opened server builds command registries before bulk-edit parsing.
    // This fixture constructs Game directly, so enter that lifecycle explicitly.
    game.build_registries();
    let edit = |palette: &str| NativeBlockEdit {
        palette: vec![palette.into()],
        filters: vec![],
        runs: vec![NativeBlockRun {
            x: POS[0],
            z: POS[2],
            y: POS[1],
            height: 1,
            surface: None,
            block: 0,
            only: None,
        }],
    };
    assert_eq!(
        game.native_edit_blocks(
            "minecraft:overworld",
            &edit("fixture:other_heater[filled=true]")
        )
        .unwrap(),
        1
    );
    assert_eq!(fuel(&game), Some(1));
    assert_eq!(
        game.native_edit_blocks("minecraft:overworld", &edit("fixture:heater[filled=false]"))
            .unwrap(),
        1
    );
    assert_eq!(fuel(&game), None);
    assert_eq!(
        game.native_edit_blocks("minecraft:overworld", &edit("fixture:heater[filled=true]"))
            .unwrap(),
        1
    );
    mark(&mut game);
    assert_eq!(
        game.native_edit_blocks("minecraft:overworld", &edit("fixture:heater[filled=false]"))
            .unwrap(),
        1
    );
    assert_eq!(fuel(&game), Some(617));
    println!("state factory: actual command placement and bulk edit lifecycle PASS");

    // EntityBlock is a class property even when a state creates no instance:
    // both empty/filled heaters reject piston moves; ordinary glass moves.
    let mut piston_descriptor = registry
        .state(registry.default_state("minecraft:piston").unwrap())
        .unwrap()
        .clone();
    piston_descriptor
        .properties
        .insert("facing".into(), "east".into());
    let piston = registry.id(&piston_descriptor).unwrap();
    let power = registry.default_state("minecraft:redstone_block").unwrap();
    let glass = registry.default_state("minecraft:glass").unwrap();
    for (target_state, blocked, has_instance) in [
        (empty, true, false),
        (full, true, true),
        (glass, false, false),
    ] {
        reset(&mut game);
        // Server block events require a lit chunk and its eight loaded neighbors.
        // A direct Game fixture has no streaming/ticket tick to establish those.
        for z in -1..=1 {
            for x in -1..=1 {
                let mut chunk = Chunk::empty(x, z);
                chunk.lit = true;
                game.world.chunks.insert((x, z), Arc::new(chunk));
            }
        }
        game.tickets.update(None);
        game.with_level(|b, level, ticks| {
            let pos = horizon_core::position::BlockPos::new;
            assert!(b.set_block(level, ticks, pos(9, 64, 8), target_state, 2));
            assert_eq!(
                level.world.block_entities.get(&(9, 64, 8)).is_some(),
                has_instance
            );
            assert!(b.set_block(level, ticks, pos(8, 64, 8), piston, 2));
            assert!(b.set_block(level, ticks, pos(7, 64, 8), power, 3));
            b.run_block_events(level, ticks);
        });
        let base = registry.state(game.world.block(8, 64, 8)).unwrap();
        assert_eq!(base.name, "minecraft:piston");
        assert_eq!(
            base.properties.get("extended").map(String::as_str),
            Some(if blocked { "false" } else { "true" }),
            "piston target {target_state:?}, blocked={blocked}"
        );
        if blocked {
            assert_eq!(game.world.block(9, 64, 8), target_state);
        } else {
            assert_ne!(game.world.block(9, 64, 8), target_state);
        }
    }
    println!(
        "state factory: real powered piston block events reject empty/filled class and move glass PASS"
    );

    // Only an in-memory connection: no real multiplayer server or player account.
    game.options.dedicated = None;
    let (client, server) = horizon_protocol::connection::Pipe::pair();
    game.connect_local(client, "StateFactoryFixture").unwrap();
    reset(&mut game);
    client_state(&mut game, None, empty);
    assert_eq!(fuel(&game), None);
    client_state(&mut game, Some(empty), full);
    assert_eq!(fuel(&game), Some(1));
    mark(&mut game);
    let holder = Holder::BlockEntity(POS);
    let attachment = Tag::compound([("sample", Tag::compound([("value", Tag::Int(701))]))]);
    game.remote.as_mut().unwrap().attachments.insert(
        holder,
        Attachments::from_nbt(Compound::from([("fixture".into(), attachment)])),
    );
    client_state(&mut game, Some(full), empty);
    assert_eq!(fuel(&game), Some(617));
    assert!(
        game.remote
            .as_ref()
            .unwrap()
            .attachments
            .contains_key(&holder)
    );
    client_state(&mut game, None, empty);
    assert_eq!(fuel(&game), Some(617));
    assert!(
        game.remote
            .as_ref()
            .unwrap()
            .attachments
            .contains_key(&holder)
    );
    client_state(&mut game, Some(empty), other);
    assert_eq!(fuel(&game), Some(1));
    assert!(
        !game
            .remote
            .as_ref()
            .unwrap()
            .attachments
            .contains_key(&holder)
    );
    game.remote
        .as_mut()
        .unwrap()
        .attachments
        .insert(holder, Attachments::default());
    client_state(&mut game, Some(other), stone);
    assert!(!game.world.block_entities.contains_key(&KEY));
    assert!(
        !game
            .remote
            .as_ref()
            .unwrap()
            .attachments
            .contains_key(&holder)
    );
    drop(server);
    game.remote.take();
    println!("state factory: actual client sync and replacement attachment lifetime PASS");
    assert!(constructions.load(Ordering::SeqCst) > 8);
    drop(game);
    std::fs::remove_dir_all(root).unwrap();
}
