#!/usr/bin/env python3
"""Exercise the owned item callback through the official raw-Mixin weaver.

Uses only an owned source snapshot and a minimal test-only bundle, with no
registered trait mods. Requires prepared vanilla resources and an owned Cargo
cache. No client window, server entry or installed player data is used.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
METHOD = 'actual_owned_item_contact_stack_discard_world_and_callback_spawns'
CONTACT = 'crates/horizon-client/src/game/item_contacts.rs'
PARAMETERS = [
    ('game', "&mut crate::game::Game<'_>", 'self'),
    ('dimension', '&str', 'dimension'), ('client', 'bool', 'client'),
    ('entity', 'u32', 'entity'),
    ('state', 'horizon_world::registry::BlockStateId', 'state'),
    ('block_pos', '[i32; 3]', 'block_pos'), ('item_pos', '[f64; 3]', 'item_pos'),
    ('stack', '&mut crate::items::ItemStack', 'stack'),
    ('discard', '&mut bool', 'discard'),
]


def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--mixins', required=True, type=Path)
    parser.add_argument('--resources', required=True, type=Path)
    parser.add_argument('--target-dir', required=True, type=Path)
    parser.add_argument('--baseline', help='optional older revision whose trait-only fast path must fail')
    args = parser.parse_args()
    work = Path(tempfile.mkdtemp(prefix='item-contact-mixin-', dir=ROOT / '.local'))
    source, mods, stage = (work / name for name in ('source', 'mods', 'stage'))
    source.mkdir()
    revision = git('rev-parse', 'HEAD').decode().strip()
    paths = git('ls-files', '--cached', '--others', '--exclude-standard', '-z').decode().split('\0')
    hashes = {}
    for name in filter(None, paths):
        origin = ROOT / name
        if not origin.exists():
            continue
        resolved = origin.resolve()
        if not resolved.is_relative_to(ROOT) or not resolved.is_file():
            raise ValueError('Expected an owned regular source file: ' + name)
        output = source / name
        output.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(resolved, output)
        hashes[name] = hashlib.sha256(output.read_bytes()).hexdigest()
    (source / '.horizon-revision').write_text(revision + '\n')
    bundle = mods / 'contact-probe'
    bundle.mkdir(parents=True)
    bindings = ',\n'.join(' {name=' + json.dumps(name) + ',ty=' + json.dumps(ty)
                            + ',value=' + json.dumps(value) + '}'
                            for name, ty, value in PARAMETERS)
    (bundle / 'horizon-mixin.toml').write_text('''[mod]
id = "contact_probe"
version = "0.1.0"
[compatibility]
horizon_revision = "''' + revision + '''"
[[module]]
id = "contact-probe"
file = "crates/horizon-client/src/game.rs"
name = "item_contact_raw_probe"
source = "probe.rs"
[[injection]]
id = "owned-contact-probe"
target = "horizon_client::game::native_hooks::Game<'_>::native_item_after_vertical_collision"
kind = "HEAD"
helper = "contact.rs"
bindings = [
''' + bindings + '\n]\n')
    signature = ', '.join(name + ': ' + ty for name, ty, _ in PARAMETERS)
    arguments = ', '.join(name for name, _, _ in PARAMETERS)
    # The woven helper itself contains no cfg: its ordinary module has a test
    # implementation and a no-op non-test implementation. It never registers
    # trait mods or calls the callback directly from the test entry point.
    (bundle / 'contact.rs').write_text('fn raw_item_contact(' + signature + ') {\n'
        + ' crate::game::item_contact_raw_probe::contact(' + arguments + ');\n}\n')
    (bundle / 'probe.rs').write_text('#[cfg(test)]\npub(super) fn contact(' + signature + ') {\n'
        + ' super::item_contact_tests::raw_contact(' + arguments + ');\n}\n'
        + '#[cfg(not(test))]\npub(super) fn contact(' + signature + ') {\n'
        + ' let _ = (' + arguments + ');\n}\n')
    target = args.target_dir.resolve()
    if not target.is_dir() or target.is_symlink():
        raise ValueError('Use an existing owned Cargo target')
    env = os.environ.copy()
    env.update(CARGO_TARGET_DIR=str(target), CARGO_BUILD_JOBS='2',
               HORIZON_TEST_RESOURCES=str(args.resources.resolve()),
               HORIZON_ITEM_CONTACT_RAW_PROBE='1', HORIZON_NO_AUDIO='1',
               HORIZON_LAUNCHER_NO_SELF_UPDATE='1', HORIZON_API_DISCOVERY='http://127.0.0.1:9')
    for key in ('HORIZON_HOME', 'HORIZON_LAUNCHER_HOME', 'XDG_DATA_HOME', 'XDG_CONFIG_HOME'):
        (work / key).mkdir()
        env[key] = str(work / key)
    report = dict(passed=False, revision=revision, source_sha256=hashes,
                  diff_sha256=hashlib.sha256(git('diff', 'HEAD', '--binary')).hexdigest(),
                  mixins_sha256=hashlib.sha256(args.mixins.read_bytes()).hexdigest(),
                  scope='Official raw weaving, real Game/Remote item tick and empty trait MODS; no Create or rendered-client acceptance',
                  checks=[])

    def run(name, command, cwd, expected=0):
        start = time.monotonic()
        with (work / (name + '.log')).open('w') as log:
            result = subprocess.run(list(map(str, command)), cwd=cwd, env=env,
                                    stdout=log, stderr=subprocess.STDOUT)
        report['checks'].append(dict(check=name, exit=result.returncode,
                                     seconds=time.monotonic() - start, expected=expected))
        (work / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
        print(name, result.returncode, work, flush=True)
        if result.returncode != expected:
            raise RuntimeError(name + ' had an unexpected result; inspect ' + str(work))

    run('official-apply', [args.mixins.resolve(), 'apply', '--workspace', source,
                          '--mods', mods, '--out', stage], ROOT)
    woven = stage / 'crates/horizon-client/src/game/native_hooks.rs'
    assert 'fn raw_item_contact(' in woven.read_text()
    command = ['cargo', 'test', '--offline', '--locked', '-p', 'horizon-client', '--lib', METHOD,
               '--', '--ignored', '--nocapture']
    candidate = (stage / CONTACT).read_bytes()
    if args.baseline:
        baseline = git('show', args.baseline + ':' + CONTACT)
        assert b'mod_api::mods().is_empty()' in baseline
        (stage / CONTACT).write_bytes(baseline)
        try:
            run('baseline-raw-callback-missing', command, stage, expected=101)
            log = (work / 'baseline-raw-callback-missing.log').read_text()
            if ('assertion' not in log and 'index out of bounds' not in log) or 'test result: FAILED' not in log:
                raise RuntimeError('The baseline must fail at a runtime assertion, not compilation')
        finally:
            (stage / CONTACT).write_bytes(candidate)
    run('actual-raw-game', command, stage)
    report['passed'] = True
    (work / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'passed': True, 'result': str(work / 'result.json')}))


if __name__ == '__main__':
    main()
