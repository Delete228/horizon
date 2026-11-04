use super::*;

#[test]
#[ignore = "loads the player's privately prepared vanilla resources"]
fn clipped_item_contact_phase_support_and_grounded_retry() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.local/horizon-client")
        .join(horizon_core::VANILLA_VERSION);
    let assets = crate::assets::Assets::load(&root).expect("private prepared test resources");
    let facts = BlockFacts::load(&root, assets.registry).unwrap();
    let collision = crate::collisions::load(&root, assets.registry).unwrap();
    let motion = crate::motion::MotionResources::load_with_facts(&root, &assets, &facts).unwrap();
    let state = |name| assets.registry.default_state(name).unwrap();
    let mut world = World::new(state("minecraft:air"), 1);
    world.chunks.insert(
        (0, 0),
        std::sync::Arc::new(crate::chunk::Chunk::empty(0, 0)),
    );
    for x in 3..=5 {
        for z in 3..=5 {
            world.set(x, 63, z, state("minecraft:stone")).unwrap();
        }
    }
    world.set(4, 64, 4, state("minecraft:cobweb")).unwrap();
    let begin = |item: &mut ItemEntity, physics| {
        item.tick_move_before_block_effects(
            &world,
            &facts,
            &collision,
            physics,
            (0.0, 0.0),
            &[],
            None,
            Some(&motion.blocks),
        )
    };
    let mut item = super::honey_slide_tests::falling_item(4.5, 64.1, 4.5, 0.0, -0.3, 0.0);
    let step = begin(&mut item, false);
    assert_eq!(step.vertical_contact, Some(BlockPos::new(4, 63, 4)));
    assert_eq!(
        item.velocity.y, 0.0,
        "default collision response already ran"
    );
    assert_eq!(
        item.stuck,
        Vec3::ZERO,
        "block effects must not run before callback"
    );
    assert_eq!(item.age, 10);
    item.finish_move_block_effects(&world, &facts, step);
    assert_eq!(item.stuck, Vec3::new(0.25, f64::from(0.05_f32), 0.25));
    item.tick_settle(&world, &facts, false, step);
    assert_eq!(item.age, 11);
    // Grounded moves are skipped on three of four ticks. A blocked machine
    // retries only on actual movement attempts, not an invented periodic scan.
    let mut item = super::honey_slide_tests::falling_item(3.5, 64.0, 3.5, 0.0, 0.0, 0.0);
    item.on_ground = true;
    item.ticks = 0;
    item.id = 0;
    let mut contacts = Vec::new();
    let mut velocities = Vec::new();
    for tick in 1..=8 {
        let step = begin(&mut item, false);
        if let Some(pos) = step.vertical_contact {
            contacts.push((tick, pos));
        }
        item.finish_move_block_effects(&world, &facts, step);
        item.tick_settle(&world, &facts, false, step);
        velocities.push(item.velocity.y);
    }
    assert_eq!(
        contacts,
        [(4, BlockPos::new(3, 63, 3)), (8, BlockPos::new(3, 63, 3))]
    );
    assert_eq!(
        velocities,
        [-0.04, -0.08, -0.12, 0.0, -0.04, -0.08, -0.12, 0.0]
    );
    let mut ghost = super::honey_slide_tests::falling_item(3.5, 64.1, 3.5, 0.0, -0.3, 0.0);
    assert_eq!(begin(&mut ghost, true).vertical_contact, None);
    // Support uses the overlapping block, not the block containing the center.
    // The nearest support/tie-break is supplied by the ordinary scanner.
    let mut edge = super::honey_slide_tests::falling_item(2.99, 64.1, 4.5, 0.0, -0.3, 0.0);
    assert_eq!(
        begin(&mut edge, false).vertical_contact,
        Some(BlockPos::new(3, 63, 4))
    );
    // Upward clipping still notifies using getOnPosLegacy, not the ceiling.
    world.set(7, 65, 4, state("minecraft:stone")).unwrap();
    let mut upward = super::honey_slide_tests::falling_item(7.5, 64.7, 4.5, 0.0, 0.5, 0.0);
    let step = upward.tick_move_before_block_effects(
        &world,
        &facts,
        &collision,
        false,
        (0.0, 0.0),
        &[],
        None,
        Some(&motion.blocks),
    );
    assert_eq!(step.vertical_contact, Some(BlockPos::new(7, 64, 4)));
    assert!(!upward.on_ground);
}
