#!/usr/bin/env python3
"""Compare the Rust fluid mesh with the player's unchanged Catnip/Ponder 1.0.82 bytecode.

Requires a private Create reference manifest containing classpath, compile_deps and Java home.
No original classes, textures or extracted sources are copied into this repository.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import random
import struct
import subprocess

ROOT = Path(__file__).resolve().parent.parent
PONDER_SHA256 = '0cf4611ad853042b689ac386184c5bbe02950efcffddb49e5f604e82baddb0dc'
SOURCES = ['crates/horizon-client-render/src/fluid_render.rs', 'tools/java/NativeFluidRenderOracle.java',
           'tools/java/native-fluid-fixtures/net/createmod/catnip/platform/CatnipServices.java']


def source_hashes():
    return {s: hashlib.sha256((ROOT / s).read_bytes()).hexdigest() for s in SOURCES}


def cases():
    rng = random.Random(0xF10D)
    boxes = [
        [0, 0, 0, 1, 1, 1], [.125, .125, .125, .875, .625, .875],
        [-1.25, .125, -.25, 1.5, 1.125, .75], [-2, -3, -1, -1, -2, 0],
        [.125, .125, .125, .875, .125, .875], [0, 0, 0, 0, 0, 0],
        [-.001953125, 0, -.001953125, .001953125, .001953125, .001953125],
    ]
    boxes += [[*(lo := [rng.randrange(-128, 128) / 32 for _ in range(3)]),
               *(x + rng.randrange(1, 65) / 32 for x in lo)] for _ in range(1024)]
    for bounds in boxes:
        for bottom in [False, True]:
            argb = rng.getrandbits(32)
            yield dict(bounds=bounds, bottom=bottom, atlas=rng.choice([128, 512, 2048, 4096]),
                       argb=argb if argb < 2**31 else argb - 2**32,
                       light=(rng.randrange(16) << 20) | (rng.randrange(16) << 4),
                       luminosity=rng.choice([0, 0, 7, 15]))


def same(a, b):
    if isinstance(a, list) and isinstance(b, list):
        return len(a) == len(b) and all(same(x, y) for x, y in zip(a, b))
    if isinstance(a, dict) and isinstance(b, dict):
        return a.keys() == b.keys() and all(same(a[k], b[k]) for k in a)
    if isinstance(a, (int, float)) and isinstance(b, (int, float)):
        return struct.pack('>f', a) == struct.pack('>f', b)
    return a == b


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--reference-manifest', type=Path, required=True)
    p.add_argument('--javac', default='javac')
    p.add_argument('--out', type=Path, default=ROOT / '.local/native-fluid-render-parity')
    p.add_argument('--target-dir', type=Path)
    args = p.parse_args()
    manifest = json.loads(args.reference_manifest.read_text())
    ponder = [Path(x) for x in manifest['compile_deps'] if Path(x).name.startswith('ponder-neoforge-1.0.82+')]
    assert len(ponder) == 1 and hashlib.sha256(ponder[0].read_bytes()).hexdigest() == PONDER_SHA256, 'wrong pinned Ponder artifact'
    before = source_hashes()
    cp = os.pathsep.join(manifest['classpath'] + manifest['compile_deps'])
    args.out.mkdir(parents=True, exist_ok=True)
    classes = args.out / 'classes'
    classes.mkdir(exist_ok=True)
    subprocess.run([args.javac, '--release', '21', '-cp', cp, '-d', str(classes),
                    str(ROOT / 'tools/java/NativeFluidRenderOracle.java'),
                    str(ROOT / 'tools/java/native-fluid-fixtures/net/createmod/catnip/platform/CatnipServices.java')], check=True)
    requests = list(cases())
    data = ''.join(json.dumps(case, separators=(',', ':')) + '\n' for case in requests)
    (args.out / 'cases.jsonl').write_text(data)
    java = subprocess.run([str(Path(manifest['java']) / 'bin/java'), '-cp',
                           str(classes) + os.pathsep + cp, 'NativeFluidRenderOracle'],
                          input=data, text=True, capture_output=True, check=True)
    env = dict(os.environ)
    if args.target_dir:
        env['CARGO_TARGET_DIR'] = str(args.target_dir)
    rust = subprocess.run(['cargo', 'run', '--quiet', '-p', 'horizon-client-render',
                           '--example', 'native_fluid_render_probe'], cwd=ROOT, env=env,
                          input=data, text=True, capture_output=True, check=True)
    (args.out / 'java.jsonl').write_text(java.stdout)
    (args.out / 'rust.jsonl').write_text(rust.stdout)
    expected = [json.loads(x) for x in java.stdout.splitlines()]
    actual = [json.loads(x) for x in rust.stdout.splitlines()]
    assert len(expected) == len(actual) == len(requests), (len(expected), len(actual))
    for i, (a, b) in enumerate(zip(expected, actual)):
        if not same(a, b):
            (args.out / 'mismatch.json').write_text(json.dumps(dict(case=requests[i], java=a, rust=b), indent=2))
            raise AssertionError(f'fluid mesh mismatch at case {i}; see {args.out / "mismatch.json"}')
    assert before == source_hashes(), 'source changed during oracle; rerun after freezing it'
    result = dict(passed=len(requests), vertices=sum(map(len, actual)),
                  scope='Actual unchanged Catnip 1.0.82 renderFluidBox/renderStillTiledFace/renderTiledFace/putVertex, '
                        'fixture service supplies sprite/color/luminosity without a mod loader; '
                        'all directions, negative/integer/fractional/zero bounds, bottom policy, '
                        'source UV shrink, raw stack RGBA, luminosity and sky light. No GPU/pixel claim.',
                  ponder_sha256=PONDER_SHA256, sha256=before)
    (args.out / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result))


if __name__ == '__main__':
    main()
