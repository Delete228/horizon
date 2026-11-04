//! Headless, synthetic server-Game checks for the custom fluid boundary.
//! Active only inside HORIZON_NATIVE_FLUID_FIXTURE's scoped component harness.
use super::Game;
use horizon_client::{
    Result,
    chunk::Chunk,
    containers::{BlockEntity, ModBlockEntity, ModBlockEntityFields, ModFluidTanks, ModRevision},
    fast_hash::{FxHashMap, FxHashSet},
    items::ItemStack,
    mod_api::{NativeFluidAction, NativeFluidDrain, NativeFluidStack, NativeFluidTankView},
    world::World,
};
use horizon_core::geometry::Direction;
use horizon_world::nbt::{self, Compound, JavaString, Tag};
use std::{cell::Cell, sync::Arc};

const P: [i32; 3] = [0, 64, 0];
const Q: [i32; 3] = [2, 64, 0];
const R: [i32; 3] = [4, 64, 0];
const S: [i32; 3] = [6, 64, 0];
const DIM: &str = "minecraft:overworld";
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Off,
    Normal,
    Unanswered,
    Invalid,
}
thread_local! {
    static MODE: Cell<Mode> = const { Cell::new(Mode::Off) };
    static CALLS: Cell<u64> = const { Cell::new(0) };
}
struct Scope;
impl Drop for Scope {
    fn drop(&mut self) {
        MODE.set(Mode::Off);
    }
}
fn active(pos: [i32; 3]) -> bool {
    let active = MODE.get() != Mode::Off && (pos == P || pos == Q);
    if active {
        CALLS.set(CALLS.get() + 1);
    }
    active
}
pub(super) fn declaration(block: &str, custom: &mut bool) -> bool {
    if MODE.get() == Mode::Off || block != "fixture:tank" {
        return false;
    }
    *custom = true;
    true
}
fn key(pos: [i32; 3]) -> (i32, i32, i32) {
    (pos[0], pos[1], pos[2])
}
fn read(game: &Game<'_>, pos: [i32; 3]) -> Option<NativeFluidStack> {
    let BlockEntity::Mod(entity) = game.world.block_entities.get(&key(pos))? else {
        return None;
    };
    let Tag::Compound(tank) = entity.nbt.get(&JavaString::from("CanonicalTank"))? else {
        return None;
    };
    let Tag::String(fluid) = tank.get(&JavaString::from("id"))? else {
        return None;
    };
    let Tag::Int(amount) = tank.get(&JavaString::from("amount"))? else {
        return None;
    };
    let Tag::Compound(components) = tank.get(&JavaString::from("components"))? else {
        return None;
    };
    Some(NativeFluidStack {
        fluid: fluid.to_string_lossy(),
        amount: *amount,
        components: components.clone(),
    })
}
fn encode(stack: &NativeFluidStack) -> Tag {
    Tag::compound([
        ("id", Tag::String(stack.fluid.as_str().into())),
        ("amount", Tag::Int(stack.amount)),
        ("components", Tag::Compound(stack.components.clone())),
    ])
}
fn store(game: &mut Game<'_>, pos: [i32; 3], stack: &NativeFluidStack) {
    game.native_with_mod_block_entity(pos, |entity| {
        entity.nbt.insert("CanonicalTank".into(), encode(stack));
    });
    game.native_block_entity_sync(pos);
}
pub(super) fn view(
    game: &Game<'_>,
    pos: [i32; 3],
    side: Option<Direction>,
    out: &mut Option<Vec<NativeFluidTankView>>,
) -> bool {
    if !active(pos) {
        return false;
    }
    if MODE.get() == Mode::Unanswered || side == Some(Direction::Down) {
        return true;
    }
    if let Some(stack) = read(game, pos) {
        *out = Some(vec![NativeFluidTankView {
            tank: 0,
            content: (stack.amount > 0).then_some(stack),
            capacity: 1000,
            can_fill: true,
            can_drain: true,
        }]);
    }
    true
}
pub(super) fn fill(
    game: &mut Game<'_>,
    pos: [i32; 3],
    side: Option<Direction>,
    resource: &NativeFluidStack,
    action: NativeFluidAction,
    out: &mut Option<i32>,
) -> bool {
    if !active(pos) {
        return false;
    }
    if MODE.get() == Mode::Unanswered || side == Some(Direction::Down) {
        return true;
    }
    if MODE.get() == Mode::Invalid {
        *out = Some(resource.amount.saturating_add(1));
        return true;
    }
    if let Some(mut held) = read(game, pos) {
        let accepted = if held.amount == 0
            || (held.fluid == resource.fluid && held.components == resource.components)
        {
            resource.amount.min(1000 - held.amount).max(0)
        } else {
            0
        };
        if accepted > 0 && action == NativeFluidAction::Execute {
            held.fluid.clone_from(&resource.fluid);
            held.components.clone_from(&resource.components);
            held.amount += accepted;
            store(game, pos, &held);
        }
        *out = Some(accepted);
    }
    true
}
pub(super) fn drain(
    game: &mut Game<'_>,
    pos: [i32; 3],
    side: Option<Direction>,
    request: &NativeFluidDrain,
    action: NativeFluidAction,
    out: &mut Option<NativeFluidStack>,
) -> bool {
    if !active(pos) {
        return false;
    }
    if MODE.get() == Mode::Unanswered || side == Some(Direction::Down) {
        return true;
    }
    if let Some(mut held) = read(game, pos) {
        let maximum = match request {
            NativeFluidDrain::Any(maximum) => *maximum,
            NativeFluidDrain::Exact(selected)
                if selected.fluid == held.fluid && selected.components == held.components =>
            {
                selected.amount
            }
            _ => return true,
        };
        let amount = maximum.min(held.amount);
        if amount > 0 {
            let mut result = held.clone();
            result.amount = amount;
            if MODE.get() == Mode::Invalid {
                result.amount = maximum.saturating_add(1);
            } else if action == NativeFluidAction::Execute {
                held.amount -= amount;
                store(game, pos, &held);
            }
            *out = Some(result);
        }
    }
    true
}
#[derive(Debug, PartialEq)]
struct SavedEntity {
    bytes: Vec<u8>,
    items: Vec<Option<ItemStack>>,
    data: Vec<i32>,
    revision: ModRevision,
}
#[derive(Debug, PartialEq)]
struct Snapshot {
    entities: Vec<SavedEntity>,
    dirty: FxHashSet<(i32, i32)>,
    log: Option<Vec<(i32, i32, i32)>>,
    random: String,
    events: String,
}
fn snapshot(game: &Game<'_>) -> Snapshot {
    let entities = [P, Q, R, S]
        .into_iter()
        .filter_map(|pos| {
            let BlockEntity::Mod(entity) = game.world.block_entities.get(&key(pos))? else {
                return None;
            };
            Some(SavedEntity {
                bytes: nbt::write_network(
                    &Tag::Compound(entity.nbt.clone()),
                    nbt::Limits::default(),
                )
                .unwrap(),
                items: entity.items.clone(),
                data: entity.data.clone(),
                revision: entity.revision(),
            })
        })
        .collect();
    Snapshot {
        entities,
        dirty: game.world.modified.clone(),
        log: game.world.block_log.clone(),
        random: format!("{:?}", game.world.random),
        events: format!("{:?}", game.level_events),
    }
}
fn plain(fluid: &str, amount: i32) -> NativeFluidStack {
    NativeFluidStack {
        fluid: fluid.into(),
        amount,
        components: Compound::new(),
    }
}
fn entity(amount: i32, components: &Compound) -> BlockEntity {
    BlockEntity::Mod(Box::new(ModBlockEntity::new(ModBlockEntityFields {
        id: "fixture:tank".into(),
        nbt: Compound::from([
            (
                "CanonicalTank".into(),
                encode(&NativeFluidStack {
                    fluid: "sample:ink".into(),
                    amount,
                    components: components.clone(),
                }),
            ),
            // A competing ordinary store must never receive custom fluid.
            ("fluids".into(), Tag::List(vec![])),
        ]),
        items: vec![Some(ItemStack::new("minecraft:diamond", 3))],
        data: vec![77],
    })))
}
impl Game<'_> {
    pub(super) fn native_fluid_fixture(&mut self, output: &str) -> Result<()> {
        assert!(!self.options.execution_role.is_client());
        self.world = World::new(self.res.facts().air, 382);
        self.world
            .chunks
            .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
        self.dim = super::Dim::Overworld;
        let registry = self.pack.assets.registry;
        let state = |id: &str| {
            registry
                .default_state(id)
                .ok_or_else(|| format!("missing {id}"))
        };
        let stone = state("minecraft:stone")?;
        let iron = state("minecraft:iron_block")?;
        for (pos, state) in [(P, stone), (Q, stone), (R, iron), (S, iron)] {
            self.world.set(pos[0], pos[1], pos[2], state)?;
        }
        let table = Arc::new(ModFluidTanks {
            tanks: vec![1000, 1000],
            faces: std::array::from_fn(|_| vec![0, 1]),
            inputs: vec![0, 1],
            outputs: vec![0, 1],
        });
        self.world
            .block_entities
            .set_mod_fluids(FxHashMap::from_iter([
                (stone.0, table.clone()),
                (iron.0, table),
            ]));
        let mut custom_states = vec![false; registry.len()];
        custom_states[stone.0 as usize] = true;
        self.world.block_entities.set_custom_fluids(custom_states);
        let components = Compound::from([
            ("sample:color".into(), Tag::Short(701)),
            ("!sample:glow".into(), Tag::Compound(Compound::new())),
        ]);
        for (pos, amount) in [(P, 750), (Q, 50), (R, 0), (S, 0)] {
            self.world
                .block_entities
                .insert(key(pos), entity(amount, &components));
        }
        self.world.modified.clear();
        self.world.block_log = Some(Vec::new());
        MODE.set(Mode::Normal);
        let _scope = Scope;
        let mut groups = Vec::new();
        let mut opted_in = false;
        self.native_custom_block_fluids("fixture:tank", &mut opted_in);
        assert!(opted_in);
        let before = snapshot(self);
        let mut resource = read(self, P).unwrap();
        resource.amount = 500;
        assert_eq!(
            self.native_fluid_fill_stack(DIM, P, None, &resource, NativeFluidAction::Simulate),
            250
        );
        let selected = NativeFluidDrain::Exact(resource.clone());
        assert_eq!(
            self.native_fluid_drain_stack(DIM, P, None, &selected, NativeFluidAction::Simulate)
                .unwrap()
                .amount,
            500
        );
        let mut owned = self.native_fluid_view(DIM, P, None).unwrap();
        owned[0].content.as_mut().unwrap().components.clear();
        assert_eq!(snapshot(self), before);
        groups.push("simulation/owned-view preserves serialized NBT, inventory, revision, dirty/sync events and RNG");

        let calls_before_zero = CALLS.get();
        for amount in [0, -1] {
            let mut empty = resource.clone();
            empty.amount = amount;
            assert_eq!(
                self.native_fluid_fill_stack(DIM, P, None, &empty, NativeFluidAction::Execute),
                0
            );
            assert!(
                self.native_fluid_drain_stack(
                    DIM,
                    P,
                    None,
                    &NativeFluidDrain::Any(amount),
                    NativeFluidAction::Execute
                )
                .is_none()
            );
        }
        assert_eq!(
            CALLS.get(),
            calls_before_zero,
            "nonpositive requests must not dispatch"
        );
        let mut wrong = resource.clone();
        wrong.components.remove(&JavaString::from("!sample:glow"));
        assert_eq!(
            self.native_fluid_fill_stack(DIM, P, None, &wrong, NativeFluidAction::Execute),
            0
        );
        assert!(
            self.native_fluid_drain_stack(
                DIM,
                P,
                None,
                &NativeFluidDrain::Exact(wrong),
                NativeFluidAction::Execute
            )
            .is_none()
        );
        // API54 extraction may not strip the payload after a simulated read.
        assert!(
            self.native_fluid_extract(DIM, P, Direction::Up, 100)
                .is_none()
        );
        assert!(self.native_fluid_tanks(DIM, P, Direction::Up).is_none());
        assert_eq!(snapshot(self), before);
        groups.push("nonpositive/selected-removal/legacy tuple requests refuse without mutation");

        MODE.set(Mode::Unanswered);
        assert!(self.native_fluid_view(DIM, P, None).is_none());
        assert_eq!(
            self.native_fluid_insert(P, Direction::Up, "minecraft:water", 1000),
            1000
        );
        assert_eq!(
            self.native_fluid_fill_stack(DIM, P, None, &resource, NativeFluidAction::Execute),
            0
        );
        MODE.set(Mode::Normal);
        assert!(
            self.native_fluid_view(DIM, P, Some(Direction::Down))
                .is_none()
        );
        assert_eq!(
            self.native_fluid_fill_stack(
                DIM,
                P,
                Some(Direction::Down),
                &resource,
                NativeFluidAction::Execute
            ),
            0
        );
        assert!(
            self.native_move_fluids(DIM, P, Direction::Up, R, Direction::Up, 100)
                .is_none()
        );
        assert!(
            self.native_move_fluids(DIM, R, Direction::Up, P, Direction::Up, 100)
                .is_none()
        );
        assert_eq!(snapshot(self), before);
        groups.push("custom ownership precedes overlay even unanswered/refused; API55 refuses either custom endpoint");

        MODE.set(Mode::Invalid);
        assert_eq!(
            self.native_fluid_fill_stack(DIM, P, None, &resource, NativeFluidAction::Execute),
            0
        );
        assert!(
            self.native_fluid_drain_stack(DIM, P, None, &selected, NativeFluidAction::Execute)
                .is_none()
        );
        MODE.set(Mode::Normal);
        assert_eq!(snapshot(self), before);
        groups.push("invalid provider amounts are refused");

        let calls_before_guards = CALLS.get();
        for dimension in ["minecraft:the_nether", "minecraft:the_end"] {
            for pos in [P, R] {
                assert!(self.native_fluid_view(dimension, pos, None).is_none());
                assert_eq!(
                    self.native_fluid_fill_stack(
                        dimension,
                        pos,
                        None,
                        &resource,
                        NativeFluidAction::Execute
                    ),
                    0
                );
                assert!(
                    self.native_fluid_drain_stack(
                        dimension,
                        pos,
                        None,
                        &selected,
                        NativeFluidAction::Execute
                    )
                    .is_none()
                );
            }
        }
        let chunk = self.world.chunks.remove(&(0, 0)).unwrap();
        for pos in [P, R] {
            assert!(self.native_fluid_view(DIM, pos, None).is_none());
            assert_eq!(
                self.native_fluid_fill_stack(DIM, pos, None, &resource, NativeFluidAction::Execute),
                0
            );
            assert!(
                self.native_fluid_drain_stack(
                    DIM,
                    pos,
                    None,
                    &selected,
                    NativeFluidAction::Execute
                )
                .is_none()
            );
        }
        assert_eq!(
            CALLS.get(),
            calls_before_guards,
            "guarded calls must not dispatch"
        );
        self.world.chunks.insert((0, 0), chunk);
        assert_eq!(snapshot(self), before);
        let saved = self.world.block_entities.remove(&key(P)).unwrap();
        assert_eq!(
            self.native_fluid_fill_stack(DIM, P, None, &resource, NativeFluidAction::Execute),
            0
        );
        self.world.block_entities.insert(key(P), saved);
        groups.push("wrong dimension/unloaded chunk/missing live BE guarded before provider or ordinary overlay");

        assert_eq!(
            self.native_fluid_fill_stack(DIM, P, None, &resource, NativeFluidAction::Execute),
            250
        );
        assert_eq!(read(self, P).unwrap().amount, 1000);
        let full = snapshot(self);
        assert_eq!(
            self.native_fluid_fill_stack(DIM, P, None, &resource, NativeFluidAction::Execute),
            0
        );
        assert_eq!(snapshot(self), full);
        let drained = self
            .native_fluid_drain_stack(DIM, P, None, &selected, NativeFluidAction::Execute)
            .unwrap();
        assert_eq!(drained, resource);
        assert_eq!(read(self, P).unwrap().amount, 500);
        assert!(self.world.modified.contains(&(0, 0)));
        assert_eq!(self.world.block_log.as_ref().unwrap(), &[key(P), key(P)]);
        groups.push("execute preserves complete payload and marks/syncs only successful writes; full no-op is serialized-identical");

        let ordinary_before = snapshot(self);
        let mut patched = plain("minecraft:water", 600);
        patched.components = components.clone();
        assert_eq!(
            self.native_fluid_fill_stack(DIM, R, None, &patched, NativeFluidAction::Execute),
            0
        );
        assert!(
            self.native_fluid_drain_stack(
                DIM,
                R,
                None,
                &NativeFluidDrain::Exact(patched),
                NativeFluidAction::Execute
            )
            .is_none()
        );
        assert_eq!(
            self.native_fluid_fill_stack(
                DIM,
                R,
                None,
                &plain("minecraft:water", 1600),
                NativeFluidAction::Simulate
            ),
            1600
        );
        assert_eq!(snapshot(self), ordinary_before);
        assert_eq!(
            self.native_fluid_insert(R, Direction::Up, "minecraft:water", 1600),
            0
        );
        // Use a plain, matching, non-full custom tank: tuple component
        // refusal or lack of room must not mask API55's explicit boundary.
        store(self, Q, &plain("minecraft:water", 50));
        let before_custom_moves = snapshot(self);
        assert!(
            self.native_move_fluids(DIM, R, Direction::Up, Q, Direction::Up, 100)
                .is_none()
        );
        assert!(
            self.native_move_fluids(DIM, Q, Direction::Up, R, Direction::Up, 100)
                .is_none()
        );
        assert_eq!(snapshot(self), before_custom_moves);
        assert_eq!(
            self.native_fluid_fill_stack(
                DIM,
                Q,
                None,
                &plain("minecraft:water", 950),
                NativeFluidAction::Execute
            ),
            950
        );
        let plain_full = snapshot(self);
        assert_eq!(
            self.native_fluid_fill_stack(
                DIM,
                Q,
                None,
                &plain("minecraft:water", 500),
                NativeFluidAction::Execute
            ),
            0
        );
        assert_eq!(
            self.native_fluid_insert(Q, Direction::Up, "minecraft:water", 500),
            500
        );
        assert_eq!(snapshot(self), plain_full);
        groups.push("plain matching custom move refusal and full-tank no-overlay fallback");
        assert_eq!(
            self.native_move_fluids(DIM, R, Direction::Up, S, Direction::Up, 1250),
            Some(("minecraft:water".into(), 1250))
        );
        assert_eq!(
            self.native_fluid_extract(DIM, R, Direction::Up, 1000),
            Some(("minecraft:water".into(), 350))
        );
        assert_eq!(
            self.native_fluid_extract(DIM, S, Direction::Up, 2000),
            Some(("minecraft:water".into(), 1250))
        );
        groups.push("ordinary API54/55 fill/move/extract conservation and typed simulation/component refusal");

        let cauldron = [8, 64, 0];
        let empty_cauldron = state("minecraft:cauldron")?;
        self.world
            .set(cauldron[0], cauldron[1], cauldron[2], empty_cauldron)?;
        let cauldron_before_simulate = snapshot(self);
        assert_eq!(
            self.native_fluid_fill_stack(
                DIM,
                cauldron,
                None,
                &plain("minecraft:water", 1000),
                NativeFluidAction::Simulate
            ),
            1000
        );
        assert_eq!(
            self.world.block(cauldron[0], cauldron[1], cauldron[2]),
            empty_cauldron
        );
        assert_eq!(snapshot(self), cauldron_before_simulate);
        assert_eq!(
            self.native_fluid_insert(cauldron, Direction::Up, "minecraft:water", 1000),
            0
        );
        assert!(
            self.native_fluid_extract(DIM, cauldron, Direction::Up, 333)
                .is_none()
        );
        assert_eq!(
            self.native_fluid_extract(DIM, cauldron, Direction::Up, 334),
            Some(("minecraft:water".into(), 334))
        );
        assert_eq!(
            self.native_fluid_extract(DIM, cauldron, Direction::Up, 1000),
            Some(("minecraft:water".into(), 666))
        );
        groups.push("vanilla cauldron simulation and exact top-third draining regression");
        std::fs::write(
            output,
            serde_json::to_vec_pretty(
                &serde_json::json!({"passed":true,"groups":groups,"scope":"actual fresh server Game/chunks/ModBlockEntities; scoped synthetic provider, independent generated-dispatch tests"}),
            )?,
        )?;
        Ok(())
    }
}
