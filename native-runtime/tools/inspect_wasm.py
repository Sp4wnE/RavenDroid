#!/usr/bin/env python3
"""Inspect the actual wasm32 ABI without compiling or executing the module."""
import argparse, json
from pathlib import Path

class Reader:
    def __init__(self, data): self.data, self.pos = data, 0
    def byte(self):
        value=self.data[self.pos]; self.pos+=1; return value
    def take(self,n):
        value=self.data[self.pos:self.pos+n]; self.pos+=n; return value
    def uint(self):
        value=shift=0
        while True:
            byte=self.byte();value|=(byte&127)<<shift
            if not byte&128:return value
            shift+=7
            if shift>64:raise ValueError('Invalid LEB128')
    def string(self):return self.take(self.uint()).decode('utf-8')
    def limits(self):
        flags=self.uint();minimum=self.uint();maximum=self.uint() if flags&1 else None
        return dict(minimum=minimum,maximum=maximum,shared=bool(flags&2),memory64=bool(flags&4))

TYPES={0x7f:'i32',0x7e:'i64',0x7d:'f32',0x7c:'f64',0x70:'funcref',0x6f:'externref'}
def inspect(data):
    r=Reader(data)
    if r.take(8)!=b'\0asm\1\0\0\0':raise ValueError('Expected wasm v1')
    types=[];imports=[];exports=[]
    while r.pos<len(data):
        kind=r.byte();length=r.uint();s=Reader(r.take(length))
        if kind==1:
            for _ in range(s.uint()):
                if s.byte()!=0x60:raise ValueError('Unsupported type')
                params=[TYPES[s.byte()] for _ in range(s.uint())];results=[TYPES[s.byte()] for _ in range(s.uint())]
                types.append(dict(params=params,results=results))
        elif kind==2:
            for _ in range(s.uint()):
                item=dict(module=s.string(),name=s.string());tag=s.byte()
                if tag==0:item.update(kind='function',**types[s.uint()])
                elif tag==1:item.update(kind='table',element=TYPES[s.byte()],**s.limits())
                elif tag==2:item.update(kind='memory',**s.limits())
                elif tag==3:item.update(kind='global',valueType=TYPES[s.byte()],mutable=bool(s.byte()))
                else:raise ValueError('Unsupported import')
                imports.append(item)
        elif kind==7:
            for _ in range(s.uint()):exports.append(dict(name=s.string(),kind=s.byte(),index=s.uint()))
    return dict(imports=imports,exports=exports)

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('wasm',type=Path);p.add_argument('--output',type=Path);a=p.parse_args()
    result=inspect(a.wasm.read_bytes());text=json.dumps(result,indent=2)
    if a.output:a.output.write_text(text+'\n')
    else:print(text)
