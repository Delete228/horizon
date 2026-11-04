# Owned item contact after vertical collision

The pending `item_after_vertical_collision` fragment provides a synchronous,
bounded extension for dropped items. It runs after a clipped vertical movement
and its normal collision response, before honey/stuck/bubble block effects,
friction, merging, age-based removal and player pickup. It includes upward
clipping and later grounded movement attempts. No-physics movement and the
three skipped grounded ticks do not produce an event.

This deliberately adds a callback at the former block-after-fall phase; it does
not claim that Minecraft 26.3 has the older `updateEntityAfterFallOn` method.
The 26.3 `Entity.move`, `restituteMovementAfterCollisions`,
`checkSupportingBlock`, `getOnPos(float)` and `ItemEntity.tick` determine the
underlying movement, support selection and tick ordering. The implementation
reuses Horizon's existing ground/support primitives. The Game owner always uses
the split movement path: an empty trait-mod registry does not imply that no raw
native Mixin was woven into the hook. The public low-level movement wrapper is
retained; the old private Game wrapper remains a test-only reference.

The hook receives the actual mutable stack while its item is owned outside the
world's item list. `Flow::Cancel` means discard this item, without undoing its
completed collision. A stack reduced to zero is also discarded. Later hooks
observe earlier stack mutations unless one explicitly discards the item.
Unchanged item ID, join sequence, position, velocity and other entity state
remain owned by the ordinary item lifecycle. Both sides can receive the event;
the `client` flag identifies prediction, not inventory authority.

Normal callback-spawned items are appended after the active batch with their
original join numbers. They start ticking on the following tick. No current-item
lookup, deferred event queue, thread-local state or unsafe Game pointer is used.
This hook does not cover players, mobs or piston movement and does not itself
implement any machine's intake or nearby-item search.

The prepared-resource movement test covers the response/effect boundary,
grounded retry cadence, no-physics, overhanging support and upward clipping.
The actual Game test exercises the normal server owner loop and a normal
memory-pipe Remote client: unchanged/refused/partial/empty stacks with opaque
components; insertion into a real barrel; explicit discard and later-hook
ownership; client prediction versus a server-only handler; and callback spawns
through the no-player, survival-player and spectator owner exit paths.

Run the resource-dependent tests in separate processes because the ordinary
Game probe installs process-local trait mods:

```sh
cargo test --offline --locked -p horizon-client-lib --lib \
  -- --ignored --exact entities::vertical_contact_tests::clipped_item_contact_phase_support_and_grounded_retry
cargo test --offline --locked -p horizon-client --lib \
  -- --ignored --exact game::item_contact_tests::actual_owned_item_contact_stack_discard_world_and_callback_spawns
cargo test --offline --locked -p horizon-client --lib \
  -- --ignored --exact game::item_contact_nomods_tests::ordinary_item_ticks_without_mods_preserve_motion_effects_and_saved_state
cargo test --offline --locked -p horizon-client -p horizon-client-lib --lib --bins --tests
cargo test --offline --locked -p horizon-client -p horizon-client-lib --doc
cargo clippy --offline --locked -p horizon-client -p horizon-client-lib --all-targets -- -D warnings
cargo run --offline --locked -p xtask -- mod-api
cargo test --offline --locked -p horizon-mod-api-gen -p horizon-mod-api-gen-tests --lib
python3 tools/test_ground_support_parity.py
python3 tools/test_entity_move_parity.py
python3 tools/test_movement_support_parity.py
```

Use privately prepared 26.3 resources under `.local/horizon-client/26.3`, and set
`HORIZON_TEST_RESOURCES` for the Game fixture when those resources are elsewhere.
Every game/test run uses disposable `HORIZON_HOME`, `HORIZON_LAUNCHER_HOME`,
`XDG_DATA_HOME` and `XDG_CONFIG_HOME`, with `HORIZON_NO_AUDIO=1` and
`HORIZON_LAUNCHER_NO_SELF_UPDATE=1`. No server launch, EULA write, window, Create
bundle, third-party decoder or installed player world is involved. The existing
LAN particle test in the broad suite opens a local TCP listener.

The three unchanged vanilla harnesses compare 342,709 support-selection,
151,444 ground-state, 543,653 movement and 51,572 support-position output lines
per real client/server JAR (2,178,756 comparisons). Their scope is the underlying
movement/support algorithms, not a claim of machine intake parity or live-game
rendering. Global API/catalog history stamping remains with the normal upstream
process; this pending fragment requires an exact source pin.

Validation on the standalone 8438f415 runtime candidate: the broad library,
binary and integration suite passed 2,501 tests with 71 deliberately ignored;
both new resource-dependent tests passed separately; all-target Clippy for
`horizon-client` and `horizon-client-lib` passed with warnings denied and no
exceptions; doc tests and the 27 generator/dispatch tests passed. This is not a
strict-check claim for an injected Create bundle or a proof of a future Basin
or Millstone adapter.

The raw-Mixin follow-up is separately exercised with the official, unmodified
`horizon-mixins apply` executable and a minimal private probe bundle:

```sh
python3 tools/test_item_contact_mixin.py --mixins PATH_TO_HORIZON_MIXINS \
  --resources PATH_TO_PREPARED_26_3 --target-dir OWNED_CARGO_TARGET \
  --baseline 6f8befda896c4131d13a27bf7b1d0312cbc24056
```

The probe is called only through the woven native hook. Its Game and Remote
fixtures assert that the global trait registry stays empty; no fake trait
registration enables the callback. The baseline compiles and then fails because
the probe receives zero calls. The corrected tick passes the same stack,
inventory, discard, spawn and grounded cadence assertions. This is actual raw
weaving evidence, separate from the trait dispatch fixture and from future
Create machine acceptance.

For the no-mod follow-up, 80 ordinary Game ticks over stone, slime, honey, ice,
cobweb, water and bubble columns match the previous wrapper's saved NBT,
motion, collision flags and synchronization state. A non-gating debug-only
measurement over seven samples of 2,000 synthetic clipped moves measured median
9.847 microseconds for the wrapper versus 17.623 microseconds with support
selection. These are one local debug run, not release performance guarantees.
The new no-mod fixture, enhanced trait fixture, actual woven fixture, and strict
all-target Clippy for both affected crates pass. The earlier 2,501-test suite and
vanilla oracles were not repeated for this small owner-path correction.
