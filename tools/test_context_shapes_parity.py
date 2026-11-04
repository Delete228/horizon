#!/usr/bin/env python3
"""Compare context-shape dispatch, inherited collision, edges and clipping to Java26.3."""
import argparse
import itertools
import json
import os
from pathlib import Path
import random
import subprocess
try:
    from . import lab, test_rust_parity as base
except ImportError:
    import lab
    import test_rust_parity as base


def corpus():
    bowl = [[.125,0,.125,.875,.125,.875],[0,.125,0,1,1,.125],
            [0,.125,.875,1,1,1],[0,.125,.125,.125,1,.875],[.875,.125,.125,1,1,.875]]
    item = [[.125,0,.125,.875,.125,.875],[0,.125,0,1,.3125,1],
            [0,.3125,0,1,.8125,.125],[0,.3125,.875,1,.8125,1],
            [0,.3125,.125,.125,.8125,.875],[.875,.3125,.125,1,.8125,.875]]
    frame = [[0,0,0,.25,1,1],[.75,0,0,1,1,1],
             [.25,0,0,.75,1,.25],[.25,0,.75,.75,1,1]]
    families = [dict(default=bowl, collision_item=item, interaction=[[0,.125,0,1,1,1],[.125,0,.125,.875,.125,.875]]),
                dict(default=frame, outline_player=[[0,.125,0,1,1,1]]),
                dict(default=[],collision_item=[[0,0,0,1,1.5,1]]),
                dict(default=[[.25,0,.25,.75,.25,.75]],
                     outline_item=[[0,0,0,1,.125,1]],
                     outline_player=[[0,.125,0,1,1,1]],
                     outline_living=[[0,0,0,.25,1,1]], outline_other=[],
                     collision_item=[[0,0,0,1,.3125,1]],
                     collision_player=[[0,0,0,1,.625,1]],
                     collision_living=[[0,0,0,1,1.5,1]], collision_other=[])]
    rays = [([.5,2,.5],[.5,-1,.5]),([.5,.5,.5],[.5,2,.5]),
            ([-1,.7,.5],[2,.7,.5]),([.5,2,-.1],[.5,-1,.8]),
            ([0,2,0],[0,-1,0]),([.125,1,.125],[.875,0,.875])]
    rng=random.Random(516)
    rays += [([rng.uniform(-1,2) for _ in range(3)], [rng.uniform(-1,2) for _ in range(3)]) for _ in range(24)]
    for family,kind,(start,end),x,y in itertools.product(families,
            ['empty','position','item','player','living','other'],rays,[.05,.5,.95],[.1,.3,.8,1.01,1.3,2]):
        # Decimal strings give Double.parseDouble and Rust's correctly rounded
        # f64 parser identical inputs; serde_json's default numeric parser can
        # otherwise introduce a one-ULP difference before the query is run.
        yield {**family,'kind':kind,'from':list(map(repr,start)),'to':list(map(repr,end)),
               'bounds':[list(map(repr,[x-.125,y,.375])),
                         list(map(repr,[x+.125,y+.25,.625]))],'dy':-3.0}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--skip-build',action='store_true')
    args=parser.parse_args()
    directory=lab.LOCAL/'rust-context-shapes-parity';directory.mkdir(parents=True,exist_ok=True)
    (directory/'report.json').unlink(missing_ok=True)
    lock,jars,libraries=base.verify_artifacts()
    oracle=lab.ROOT/'mods/horizon/src/test/java/dev/horizonlab/parity/VanillaContextShapesOracle.java'
    sources=[oracle,oracle.with_name('VanillaCollisionContextOracle.java'),oracle.with_name('VanillaFaceSupportOracle.java')]
    tracked=[*sources,Path(__file__),
             *[lab.ROOT/name for name in [
                 'crates/horizon-world/examples/probe/context_shapes_fixture.rs',
                 'crates/horizon-world/src/collision.rs','crates/horizon-world/src/collision_context.rs',
                 'crates/horizon-world/src/block_collision.rs','crates/horizon-world/src/interaction_shapes.rs',
                 'crates/horizon-core/src/geometry/shape.rs','crates/horizon-core/src/geometry/mod.rs',
                 'crates/horizon-mods/src/content.rs','crates/horizon-client/src/mod_content.rs']]]
    tested_sources=base.source_hashes(tracked)
    classes=directory/'classes';classes.mkdir(exist_ok=True)
    def run(command,name):
        with (directory/(name+'.log')).open('w') as output:
            subprocess.run(list(map(str,command)),cwd=lab.ROOT,stdout=output,stderr=subprocess.STDOUT,check=True)
    cp=os.pathsep.join(map(str,[jars['server'],*libraries]))
    run(['javac','--release',lock['minecraft']['java_major'],'-proc:none','-sourcepath',lab.ROOT/'mods/horizon/src/test/java','-cp',cp,'-d',classes,*sources],'compile-java')
    fixtures=list(corpus());inputs=directory/'inputs.txt'
    inputs.write_text(''.join(json.dumps(f,separators=(',',':'))+'\n' for f in fixtures))
    for side,jar in jars.items():
        run(['java','-Xmx512M','-cp',os.pathsep.join(map(str,[classes,jar,*libraries])),
             'dev.horizonlab.parity.VanillaContextShapesOracle',side,directory],side)
    if not args.skip_build:
        run(['cargo','build','--offline','-p','horizon-world','--example','world_probe'],'compile-rust')
    target=lab.cargo_target()
    run([target/'debug/examples/world_probe','context_shapes_fixture',directory],'rust')
    results={side:base.compare(directory/(side+'.txt'),directory/'rust.txt') for side in jars}
    base.ensure_sources_unchanged(tracked,tested_sources)
    report={'minecraft':lock['minecraft']['version'],'cases':len(fixtures),'matched_output_lines':results,
            'sources_sha256':tested_sources,
            'input_sha256':lab.digest(inputs,'sha256'),'rust_output_sha256':lab.digest(directory/'rust.txt','sha256'),
            'scope':'Real vanilla BlockState/context dispatch for all four classes and empty/position defaults, including a player-only outline with inherited empty-context collision, merged forAllEdges, VoxelShape.collide and clipWithInteractionOverride. Explicit bowl/frame/test geometry and immutable Rust context-table loader. Real ItemEntity movement/overlap and conservative scan integration are covered separately by native Rust tests; this oracle does not establish whole-game presentation or networking parity.'}
    lab.write_json(directory/'report.json',report)
    print(json.dumps(report,indent=2))


if __name__=='__main__':
    main()
