# State-dependent native block entity factories (pending API)

This local extension allows a mod block to create no block entity in one state
and a canonical mod entity in another. It does not assign a global API number;
the fragment is stamped only after an upstream merge.

## Ownership and factory

`Game::native_stateful_block_entity(block, &mut bool)` declares ownership once
per non-vanilla block while an immutable, level-owned state table is built.
Declare true for your block; never clear another declaration. Vanilla blocks
and undeclared mod blocks retain API15 and their existing factories.

For a declared block,
`GameLevel::native_block_entity_from_state(block, properties, &mut Option<(String, Compound)>)`
receives the actual block name and property map borrowed from the registry. It
is an associated callback with no Host/world attached and allocates no context. The first supplied entity
owns the response. No response creates no entity, with no fallback to API15 or
the block template. The callback must not mutate the level or its RNG. A null
response must be stable for a state; bulk construction caches nulls per state,
never per block name. Successful construction is still per instance.

Declaring ownership also identifies an EntityBlock class in the level: a null
factory result does not make a state piston-pushable. This is checked through
`LevelAccess::has_native_block_entity`, separately from live instance lookup.

The declaration is compiled before the first level operation, not deferred
until the first block-entity tick. Chunk synchronization and commands can create
entities before that tick. The table belongs to the level's immutable registry.

## Lifetime and order

`LevelAccess::ensure_block_entity` defaults to no additional factory. The block
mutation path invokes it after `onPlace`, checks the actual landed block identity,
and supplies the actual state before returning from `setBlock`. GameLevel uses
one indexed ownership test, preserves an existing instance, and invokes the new
factory inline when a declared block has none. Block hooks, property changes and
neighbor-triggered writes therefore see the newly created entity in the same
level closure. Bulk/raw writes and client synchronization use the same factory
through their explicit adapters.

Creation is separate from validity: an existing entity survives a property
change within the same block, even if a fresh instance of the resulting state
would create none. Replacing the block removes its instance and attachments,
even if the replacement factory returns the same canonical entity id. Placement
still applies supplied item block-entity data to an instance created inline;
falling blocks still merge their carried data over the new defaults.

## Scope

The factory receives the actual block name/property map, not position, neighbors,
player or mutable level/RNG.
A mod requiring those inputs needs a separate extension. The saved mod entity id
is supplied by the mod. Horizon's current network registry still uses a block
name as its mod block-entity type alias; this is not Java protocol compatibility.

## Validation

The frozen source passed two generated-dispatch tests, eight item-data unit tests,
and the explicitly executed prepared-resource Game fixture. Its eight groups cover:

1. Actual generated factories: declared null, no API15 fallback, and unchanged legacy/vanilla defaults.
2. Native state writes: missing-instance creation, same-block fuel retention, and different-block replacement.
3. Both public property helpers, including unchanged/invalid property rejection.
4. Raw behaviour and level-hook property writes, with the entity observable immediately in the same closure.
5. Actual BlockItem placement with supplied block-state components and entity fuel `923` preserved.
6. Command placement and bulk-edit creation/retention, including distinct blocks sharing one canonical entity id.
7. Real powered piston block events: empty and filled declared EntityBlock classes reject moves; ordinary glass moves.
8. Actual in-memory client synchronization, including same-block retention and replacement attachment removal.

The item-data tests also cover opaque nested merge, unchanged identity/coordinates,
unknown payload bytes, mismatched type rejection and pick/place data. Vanilla
canonicalization and op-only gates remain in force. Strict all-target Clippy with
warnings denied passed for the client, world, renderer and generated dispatch crate
with default features disabled. The source digest was unchanged across these checks.

Reproduce the dispatch and item tests with:

```sh
cargo test -p horizon-mod-api-gen-tests state_block_entity_tests -- --nocapture
cargo test -p horizon-client-lib --lib block_entity_item::tests -- --nocapture
cargo clippy -p horizon-client -p horizon-world -p horizon-client-render -p horizon-mod-api-gen-tests --all-targets --no-default-features -- -D warnings
```

The Game test installs process-wide trait mods and must run alone. It requires
the player's complete prepared 26.3 resources, including registry/command reports,
and creates a disposable content root. Use isolated homes and a local-only discovery
endpoint, as this example does after selecting the pinned Rust toolchain:

```python
import os
from tools.integration_env import run_game

env = dict(os.environ,
           HORIZON_TEST_RESOURCES="/path/to/prepared/26.3",
           HORIZON_API_DISCOVERY="http://127.0.0.1:9")
run_game(["cargo", "test", "-p", "horizon-client", "--lib",
          "--no-default-features", "actual_state_factory_lifecycle",
          "--", "--ignored", "--nocapture"], env=env, check=True)
```

The fixture initializes the normal command registry and loads a lit 3×3 chunk
neighborhood before exercising server piston events; it does not bypass the
loaded/ticking gate. Its client uses an in-memory connection, not a real account
or multiplayer server. The ordinary suite intentionally ignores this test; the
pass above is from the explicit ignored-test invocation.

These are focused checks, not a complete repository or Java visual parity claim.
The previously reproduced unchanged-base Darwin installer-signature fixture
failure is outside this change; no aggregate `pr_ready` pass is asserted here.
