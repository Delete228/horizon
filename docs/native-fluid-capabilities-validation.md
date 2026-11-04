# Custom block fluid capability validation

This is an opt-in native extension on the compatibility branch based on
`08821fdbce6b5d081f1e6cc5c4d0da5438861222`. It uses the pending
`custom_block_fluids` hook fragment; it does not allocate a global API number
or claim an official upstream release. API 54's `ModFluidTanks`, tuple return
types and ordinary save schema remain unchanged.

The new owned fluid stack carries its id, millibuckets and complete component
patch, including removals. Tank views, fill and both selected/maximum drain
forms use that value. Custom storage owns its canonical block entity fields;
there is no second `fluids` inventory mirroring it. The declaration is cached
per block state, so ordinary accesses do not dispatch ownership hooks.

## Behavior covered

The real server-Game component fixture creates fresh synthetic loaded chunks
and actual `ModBlockEntity` objects. A scoped provider stores its fluid in a
canonical nested compound while an ordinary tank declaration is also present.
It checks:

- Simulated operations and owned view edits leave serialized NBT, inventory,
  data slots, entity revision, dirty chunks, synchronization log, level events
  and the complete world RNG state unchanged.
- Nonpositive requests do not invoke the provider. Selected drains compare the
  complete patch; removal differs from absence. The legacy tuple reader and
  extractor refuse component-bearing results before any execution.
- Custom ownership prevents ordinary fallback for unanswered hooks, refused
  sides and full tanks. Invalid returned amounts are refused.
- Wrong dimensions, unloaded chunks and missing live block entities are
  guarded before provider access or ordinary fallback.
- Execute preserves fluid components and records the provider's normal
  successful writes. Full-tank no-ops remain byte-for-byte unchanged.
- API 55 refuses a plain matching custom endpoint even when both endpoints
  have fluid and room. Ordinary API 54/55 insertion, movement and extraction
  still conserve their plain fluid.
- Ordinary typed simulation is read-only, and ordinary storage refuses
  nonempty component patches. Vanilla cauldrons retain their whole-third
  filling/draining behavior.

The fixture uses direct ownership tables for its synthetic states; it is not
proof of a third-party content registration, network lifecycle or arbitrary
provider implementation. Separate tests compile the real generated event
fields, response priority and owned component-bearing payloads through trait
mod dispatch. The latter uses a small provider fixture, rather than a Game.

## Reproduction

Use a private prepared resource folder and a compiled game from this branch:

```sh
cargo build -p horizon-client --no-default-features --bin horizon-client
python3 tools/test_native_fluid_capabilities.py --game PATH --resources PATH
```

The driver creates private homes, copies resources, runs headlessly and never
opens a save. It retains its JSON result, exact executable SHA-256 and logs
under `.local/native-fluids-*` on success or failure. It uses the normal
component-harness server thread, without a window or renderer.

Other checks run for this change:

```sh
cargo check -p horizon-client --no-default-features
cargo test -p horizon-mod-api-gen -p horizon-mod-api-gen-tests
cargo test -p horizon-client --no-default-features --test custom_block_fluids
cargo test -p horizon-client --no-default-features --lib game::machines::native_fluids::tests
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo clippy -p horizon-client --all-targets --locked --no-default-features -- -D warnings
cargo fmt --all -- --check
python3 tools/check_public_tree.py
python3 tools/pr_ready.py --quick --base 08821fdbce6b5d081f1e6cc5c4d0da5438861222
```

Validation status: generated tests 15 + 9 passed; component dispatch 1 passed;
storage regressions 10 passed; the final strengthened real-Game fixture
9 groups passed with executable SHA-256
`2b6395c9d4ce4104fd62b53181b6dc5be68d15d960e8a69319507b1d24e85861`;
workspace/server Clippy, formatting and the public-tree check passed.
The complete `pr_ready --quick` report is still running; it is not claimed as
a pass. No visual or Java parity claim applies to this generic extension.

## Limits

Simulation is a provider contract. Native code is trusted; the engine cannot
prevent an arbitrary provider from causing side effects during simulation.
Validation bounds the returned amount and selected payload, not actual storage
consumption. It does not audit or roll back execution, and providers retaining
known source quirks can consume a different amount from what they return.
Consequently a returned stack alone is not a transaction or conservation
guarantee. API 55 deliberately refuses custom endpoints instead of applying
ordinary tank receipt rollback to an arbitrary provider.

Only the loaded current server level is accessible. A block entity temporarily
out of the world while ticking is unavailable. Unsided access is explicit;
providers decide their own side policy. Ordinary storage cannot represent fluid
components and refuses them. This change does not add custom-fluid rendering,
pumps, item-container codecs or a cross-provider atomic move operation.
