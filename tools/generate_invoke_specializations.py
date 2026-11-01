"""Generate audited typed invoke cases from pinned ABI types, never letter guesses."""
import argparse,json,re
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def generate(check=False):
    abi={x['name']:x for x in json.loads((ROOT/'native-runtime/abi-inventory.json').read_text())['imports'] if x.get('name','').startswith('invoke_')}
    names=json.loads((ROOT/'native-runtime/invoke-specializations.json').read_text())['signatures']
    if len(names)!=len(set(names)):raise ValueError('Duplicate specialization')
    enum=[];cache=[];bind=[]
    for n,name in enumerate(names):
        item=abi['invoke_'+name];params=item['params'];results=item['results']
        if not params or len(params)>16 or any(p not in ('i32','f64') for p in params) or results not in ([],['i32'],['f64']):raise ValueError('Unsupported typed ABI')
        ty=params[0] if len(params)==1 else '('+','.join(params)+')';ret=results[0] if results else '()';zero={'i32':'0','f64':'f64::NAN','()':'()'}[ret]
        enum.append(f'    Extra{n}(TypedFunc<{ty},{ret}>),')
        cache.append(f'            "dynCall_{name}"=>InvokeCall::Extra{n}(function.typed(&*caller)?),')
        args=','.join(f'a{i}:{ty}' for i,ty in enumerate(params))
        bind.append(f'        "invoke_{name}"=>bind!(Extra{n},{ret},{zero};{args}),')
    target=ROOT/'native-runtime/src/exceptions.rs';source=target.read_text(encoding='utf-8')
    for tag,lines in [('ENUM',enum),('CACHE',cache),('BIND',bind)]:
        begin=f'// BEGIN GENERATED {tag}';end=f'// END GENERATED {tag}'
        body=begin+'\n'+'\n'.join(lines)+'\n'+end
        source,count=re.subn(re.escape(begin)+r'.*?'+re.escape(end),lambda _:body,source,flags=re.S)
        if count!=1:raise ValueError('Missing or duplicate generated block: '+tag)
    if check:
        if source!=target.read_text(encoding='utf-8'):raise ValueError('Generated invoke bindings are stale')
    else:target.write_text(source,encoding='utf-8')
if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--check',action='store_true');generate(parser.parse_args().check)
