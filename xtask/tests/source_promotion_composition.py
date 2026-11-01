import os, subprocess, tempfile, json, hashlib, pathlib
script=str(pathlib.Path(__file__).resolve().parents[2] / '.github/scripts/source-promotion-composition.sh')
def run(*args,env=None,ok=True):
 p=subprocess.run(args,cwd=root,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
 if (p.returncode==0)!=ok: raise Exception(f'{args} status {p.returncode} {p.stderr}')
 return p.stdout.strip()
with tempfile.TemporaryDirectory() as root:
 root=pathlib.Path(root); run('git','init','-q'); run('git','config','user.name','Tester'); run('git','config','user.email','test@example.com')
 (root/'a').write_text('p');run('git','add','a');run('git','commit','-qm','P'); p=run('git','rev-parse','HEAD')
 (root/'a').write_text('s');run('git','commit','-qam','S');s=run('git','rev-parse','HEAD'); t=run('git','rev-parse','HEAD^{tree}')
 k=run('git','commit-tree',t,'-p',p,'-p',s,'-m','K');run('git','update-ref','refs/pull/42/merge',k)
 (root/'docs/release/source-promotion').mkdir(parents=True)
 pf={'mode':'fast_forward','source_parent':p,'swarm_parent':s,'dry_merge':{'preview_tree':t}}
 data=json.dumps(pf).encode();(root/'docs/release/source-promotion/preflight.json').write_bytes(data)
 packet={'schema':'ripr.source_promotion_ci_inputs.v2','source_main':p,'join_head':k,'preflight':'docs/release/source-promotion/preflight.json','preflight_sha256':hashlib.sha256(data).hexdigest()}
 (root/'docs/release/source-promotion/contract-inputs.json').write_text(json.dumps(packet));run('git','add','docs');run('git','commit','-qm','control'); c=run('git','rev-parse','HEAD');run('git','remote','add','origin',str(root))
 env=os.environ|{'PR_BODY':f'<!-- source-promotion: true -->\n<!-- source-promotion-control: {c} -->','HEAD_REF':'promote/0.11.0-test','BASE_SHA':p,'HEAD_SHA':s,'MERGE_SHA':k,'PR_NUMBER':'42'}
 run('bash',script,env=env);run('bash',script,env=env|{'BASE_SHA':s},ok=False);run('bash',script,env=env|{'HEAD_SHA':p},ok=False);run('bash',script,env=env|{'MERGE_SHA':p},ok=False);run('bash',script,env=env|{'PR_BODY':'broken'},ok=False);run('bash',script,env=env|{'PR_BODY':'ordinary','HEAD_REF':'feature'},ok=True)
 # Existing direct-join controls remain valid when the PR head is J.
 j=run('git','commit-tree',t,'-p',p,'-p',s,'-m','J'); k2=run('git','commit-tree',t,'-p',p,'-p',j,'-m','preview-join')
 run('git','update-ref','refs/pull/42/merge',k2)
 pf={'mode':'two_parent_join','source_parent':p,'swarm_parent':s,'dry_merge':{'reviewed_resolved_tree':t}}
 data=json.dumps(pf).encode();(root/'docs/release/source-promotion/preflight.json').write_bytes(data)
 packet['join_head']=j; packet['preflight_sha256']=hashlib.sha256(data).hexdigest(); (root/'docs/release/source-promotion/contract-inputs.json').write_text(json.dumps(packet))
 run('git','add','docs');run('git','commit','-qm','join control'); c2=run('git','rev-parse','HEAD')
 env2=env|{'PR_BODY':f'<!-- source-promotion: true -->\n<!-- source-promotion-control: {c2} -->','HEAD_SHA':j,'MERGE_SHA':k2}
 run('bash',script,env=env2)
 print('ordinary, staged, legacy, missing marker, wrong base/head/preview: pass')
