#!/usr/bin/env python3
"""Execute the player's unchanged 1.21.1 FaceBakery; compare native model geometry/UV/normals.

No game is started and no game classes or assets are bundled. The manifest must name the
player's existing reference classpath. This is a method/vertex oracle, not a pixel comparison.
"""
import argparse
import json
import math
import os
from pathlib import Path
import random
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def pose(rng):
    angle = rng.uniform(-math.pi, math.pi)
    c, s = math.cos(angle), math.sin(angle)
    x, y, z = [rng.choice([.25, .5, 1, 2, 3]) for _ in range(3)]
    if rng.randrange(3) == 0:
        x = -x
    return [c*x, 0, -s*x, 0, 0, y, 0, 0, s*z, 0, c*z, 0,
            rng.uniform(-8, 8), rng.uniform(-8, 8), rng.uniform(-8, 8), 1]


def cases():
    rng = random.Random(0x507032)
    identity = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]
    result = []
    for direction in ['down', 'up', 'north', 'south', 'west', 'east']:
        for rotation in [0, 90, 180, 270]:
            for atlas in [512, 1024, 2048, 8192]:
                result.append(dict(direction=direction, rotation=rotation, atlas=atlas,
                                   **{'from': [0, 0, 0]}, to=[16, 16, 16],
                                   uv=[0, 0, 16, 16], pose=identity))
    for _ in range(416):
        low = [rng.randrange(-64, 96)/8 for _ in range(3)]
        high = [v + rng.randrange(1, 128)/8 for v in low]
        uv = [rng.randrange(-32, 128)/8 for _ in range(4)]
        result.append(dict(direction=rng.choice(['down', 'up', 'north', 'south', 'west', 'east']),
                           rotation=rng.randrange(4)*90, atlas=rng.choice([512, 1024, 2048, 8192]),
                           **{'from': low}, to=high, uv=uv, pose=pose(rng)))
    return result


def run(args):
    manifest = json.loads(args.reference_manifest.read_text())
    if manifest['minecraft'] != '1.21.1':
        raise ValueError('this oracle requires the pinned Minecraft 1.21.1 reference')
    args.output.mkdir(parents=True, exist_ok=True)
    classes = args.output/'classes'
    classes.mkdir(exist_ok=True)
    classpath = os.pathsep.join(manifest['compile_deps']+manifest['classpath'])
    commands = [
        [args.javac, '--release', '21', '-cp', classpath, '-d', str(classes),
         str(ROOT/'tools/java/NativeTerrainModelOracle.java')],
        ['cargo', 'build', '-p', 'horizon-client-render', '--example', 'native_terrain_model_probe'],
    ]
    for index, command in enumerate(commands):
        with (args.output/f'compile-{index}.log').open('w') as log:
            subprocess.run(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT, check=True)
    data = ''.join(json.dumps(case)+'\n' for case in cases())
    (args.output/'cases.jsonl').write_text(data)
    target = Path(os.environ.get('CARGO_TARGET_DIR', ROOT/'target'))
    commands = [
        [str(Path(manifest['java'])/'bin/java'), '-Djoml.useMathFma=false', '-cp',
         str(classes)+os.pathsep+classpath, 'NativeTerrainModelOracle'],
        [str(target/'debug/examples/native_terrain_model_probe')],
    ]
    values = []
    for name, command in zip(['java', 'rust'], commands):
        process = subprocess.run(command, cwd=ROOT, input=data, text=True, capture_output=True)
        (args.output/f'{name}.jsonl').write_text(process.stdout)
        (args.output/f'{name}.stderr').write_text(process.stderr)
        if process.returncode:
            raise RuntimeError(f'{name} failed; see {args.output}')
        values.append([json.loads(line) for line in process.stdout.splitlines()])
    expected, actual = values
    assert len(expected) == len(actual) == len(cases())
    floats = 0
    maximum = 0.0
    for index, (left, right) in enumerate(zip(expected, actual)):
        assert len(left) == len(right) == 4
        for vertex, (a, b) in enumerate(zip(left, right)):
            for field in ['pos', 'uv', 'normal']:
                for component, (x, y) in enumerate(zip(a[field], b[field])):
                    floats += 1
                    maximum = max(maximum, abs(x-y))
                    if not math.isclose(x, y, rel_tol=2e-6, abs_tol=2e-5):
                        raise AssertionError((index, vertex, field, component, x, y, cases()[index]))
    result = dict(passed=True, cases=len(expected), vertices=len(expected)*4,
                  floats=floats, maximum_absolute_difference=maximum,
                  absolute_tolerance=2e-5, relative_tolerance=2e-6,
                  source='unchanged Minecraft 1.21.1 FaceBakery plus JOML instance normal transform')
    (args.output/'result.json').write_text(json.dumps(result, indent=2)+'\n')
    print(json.dumps(result))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reference-manifest', type=Path, required=True)
    parser.add_argument('--javac', required=True)
    parser.add_argument('--output', type=Path, required=True)
    run(parser.parse_args())
