#!/usr/bin/env python3
"""Run the generic fluid capability's real server-Game component fixture.

This is an extension contract test, not a Java/vanilla parity claim. The game
loads private resource copies and synthetic chunks; it never opens a save.
Artifacts are retained under .local/ on both success and failure.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

try:
    from .integration_env import game_environment as game_env
except ImportError:
    from integration_env import game_environment as game_env

ROOT = Path(__file__).resolve().parents[1]


def run(game, resources, scratch=None):
    game, resources = Path(game).resolve(), Path(resources).resolve()
    if not game.is_file() or not resources.is_dir():
        raise ValueError('the compiled game and prepared resource folder are required')
    if scratch is None:
        (ROOT / '.local').mkdir(exist_ok=True)
        scratch = Path(tempfile.mkdtemp(prefix='native-fluids-', dir=ROOT / '.local'))
    else:
        scratch = Path(scratch).resolve()
        scratch.mkdir(parents=True, exist_ok=False)
    private_resources = scratch / 'resources'
    shutil.copytree(resources, private_resources)
    env = game_env(os.environ, scratch)
    env.update(HORIZON_NATIVE_FLUID_FIXTURE=str(scratch / 'fixture-result.json'))
    for key, folder in [('HORIZON_LAUNCHER_TRASH', 'trash'),
                        ('HORIZON_LAUNCHER_CLIPBOARD', 'clipboard'),
                        ('HORIZON_LAUNCHER_DESKTOP', 'desktop')]:
        (scratch / folder).mkdir()
        env[key] = str(scratch / folder)
    command = [str(game), '--headless', '--resources', str(private_resources),
               '--name', 'FluidCapabilityFixture']
    digest = hashlib.sha256()
    with game.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    result = {'passed': False, 'game': str(game), 'command': command,
              'game_sha256': digest.hexdigest(),
              'scratch': str(scratch)}
    try:
        with (scratch / 'stdout.log').open('wb') as out, (scratch / 'stderr.log').open('wb') as err:
            completed = subprocess.run(command, cwd=scratch, env=env, stdout=out, stderr=err,
                                       timeout=180, check=False)
        result['exit_code'] = completed.returncode
        fixture = scratch / 'fixture-result.json'
        if fixture.is_file():
            result['fixture'] = json.loads(fixture.read_text())
        result['passed'] = completed.returncode == 0 and result.get('fixture', {}).get('passed') is True
    except Exception as error:
        result['error'] = str(error)
    (scratch / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({'passed': result['passed'], 'result': str(scratch / 'result.json')}))
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--game', type=Path, required=True)
    parser.add_argument('--resources', type=Path, required=True)
    parser.add_argument('--scratch', type=Path)
    args = parser.parse_args()
    raise SystemExit(0 if run(args.game, args.resources, args.scratch)['passed'] else 1)


if __name__ == '__main__':
    main()
