//! No-mod trajectories stay equivalent to the existing public item tick wrapper.
use super::*;
use horizon_client::{chunk::Chunk, entities::ItemEntity};
use std::{hint::black_box, path::PathBuf, sync::Arc};

#[test]
#[ignore = "private prepared resources; run in a process without trait mods"]
fn ordinary_item_ticks_without_mods_preserve_motion_effects_and_saved_state() {
    assert!(crate::mod_api::mods().is_empty());
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
    game.world = World::new(game.res.facts().air, 1);
    game.world
        .chunks
        .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
    game.player = None;
    let registry = game.pack.assets.registry;
    let state = |name| registry.default_state(name).unwrap();
    for (x, z, support, inside) in [
        (2, 2, "minecraft:stone", None),
        (6, 2, "minecraft:slime_block", None),
        (10, 2, "minecraft:honey_block", None),
        (14, 2, "minecraft:ice", None),
        (2, 7, "minecraft:stone", Some("minecraft:cobweb")),
        (7, 7, "minecraft:stone", Some("minecraft:water")),
        (12, 7, "minecraft:stone", Some("minecraft:bubble_column")),
    ] {
        game.world.set(x, 63, z, state(support)).unwrap();
        if let Some(inside) = inside {
            game.world.set(x, 64, z, state(inside)).unwrap();
        }
        game.spawn_item(
            ItemStack::new("minecraft:iron_ingot", 3),
            [f64::from(x) + 0.5, 64.4, f64::from(z) + 0.5],
            [0.005, -0.3, 0.0],
            20,
        );
    }
    let mut expected: Vec<ItemEntity> = game.dropped.iter().cloned().collect();
    for tick in 1..=80 {
        game.world.game_time += 1;
        game.ticked_entities_through = game.next_entity;
        for item in &mut expected {
            assert!(super::entity::tick_item(
                item,
                &game.world,
                game.pack.assets.registry,
                game.res.facts(),
                game.collision,
                &game.res.motion().fluids,
                false,
                &[],
                Some(game.border.geometry()),
            ));
        }
        game.tick_items();
        assert_eq!(game.dropped.len(), expected.len());
        for (actual, expected) in game.dropped.iter().zip(&expected) {
            assert_eq!(
                crate::entity_nbt::item_entity_to_nbt(actual, [0; 4], None),
                crate::entity_nbt::item_entity_to_nbt(expected, [0; 4], None),
                "tick {tick}"
            );
            assert_eq!(
                (
                    actual.previous,
                    actual.stuck,
                    actual.vertical_collision,
                    actual.horizontal_collision,
                    actual.needs_sync
                ),
                (
                    expected.previous,
                    expected.stuck,
                    expected.vertical_collision,
                    expected.horizontal_collision,
                    expected.needs_sync
                ),
                "tick {tick}"
            );
        }
    }
    // Non-gating debug measurement: same falling item and collision scene,
    // wrapper versus split movement plus normal support selection. Cloning is
    // symmetric; no elapsed-time threshold can fail this regression.
    let mut base = game.dropped[0].clone();
    base.position = Vec3::new(2.5, 64.1, 2.5);
    base.velocity = Vec3::new(0.0, -0.3, 0.0);
    base.on_ground = false;
    base.stuck = Vec3::ZERO;
    base.supporting_block = None;
    base.on_ground_no_blocks = false;
    let mut samples = [Vec::new(), Vec::new()];
    for _ in 0..7 {
        for (split, samples) in samples.iter_mut().enumerate() {
            let start = Instant::now();
            for _ in 0..2_000 {
                let mut item = base.clone();
                if split == 0 {
                    item.tick_move(
                        &game.world,
                        game.res.facts(),
                        game.collision,
                        false,
                        (0.0, 0.0),
                        &[],
                        Some(game.border.geometry()),
                    );
                } else {
                    let step = item.tick_move_before_block_effects(
                        &game.world,
                        game.res.facts(),
                        game.collision,
                        false,
                        (0.0, 0.0),
                        &[],
                        Some(game.border.geometry()),
                        Some(&game.res.motion().blocks),
                    );
                    item.finish_move_block_effects(&game.world, game.res.facts(), step);
                }
                black_box(item);
            }
            samples.push(start.elapsed().as_nanos() / 2_000);
        }
    }
    for sample in &mut samples {
        sample.sort_unstable();
    }
    println!(
        "ITEM_CONTACT_COST {{\"profile\":\"debug\",\"iterations_per_sample\":2000,\"samples\":7,\"baseline_median_ns\":{},\"support_median_ns\":{}}}",
        samples[0][3], samples[1][3]
    );
    assert!(crate::mod_api::mods().is_empty());
}
