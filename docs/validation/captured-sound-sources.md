# Captured positional sound sources

`Audio::play_on` resolves a `SoundSource` before playback. The positional queue
now retains that source, so guest block-use/place/instrument and ordinary world
broadcasts use it in the Sound packet instead of inferring a new category from
the sound-event name. Plain `play` and delayed/seeded playback retain their
existing category inference. Player-only manually queued events remain PLAYERS;
captured events moved to the actor-excluded player queue retain their source.

No registry, protocol, content format or named native hook changes. The transient
capture record gains one `usize`; the existing packet already carries its source.
The sculk/dispenser parity text formats are unchanged.

Validation commands, with a private prepared-resource copy and private launcher
and XDG homes as required by AGENTS.md:

```sh
cargo test --offline -p horizon-client-render --lib audio::tests::
HORIZON_TEST_RESOURCES=/private/prepared/26.3 \
  cargo test --offline -p horizon-client --lib \
  captured_sound_source_reaches -- --ignored --nocapture
cargo clippy --offline -p horizon-client -p horizon-client-render \
  --lib --tests -- -D warnings
cargo fmt --all -- --check
python3 tools/check_public_tree.py
git diff --check
```

The Audio test covers explicit BLOCKS versus ordinary PLAYERS/HOSTILE defaults,
seeded/delayed events, unshared suppression and non-positional exclusion. The
Game test loads prepared resources, uses the normal guest sound route and a real
Hub with an ephemeral localhost-only listener, and decodes actor/observer Sound
packets: the predicting actor receives no server copy, the observer receives one
BLOCKS packet, and general broadcasts retain ordinary/overridden sources in
order. Guest senders are staged in process; this is not a two-client acoustic or
rendering comparison.
