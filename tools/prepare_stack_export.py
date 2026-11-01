"""Expose the verified pure stackSave global; preserve every other WASM byte.
Only accepts the SHA-pinned Ravenfield source. Not a general WASM optimizer.
"""
import hashlib
from pathlib import Path
import argparse
SOURCE_SHA = "03c6c9516d803086adf7e57c2122b21988421cb1302d1736609cd4e44324cc0a"
EXPORT = b"__raven_stack_pointer_v1"
def leb(value):
    out=bytearray()
    while value>=128: out.append((value&127)|128); value >>= 7
    out.append(value); return bytes(out)
def transform(source):
    if hashlib.sha256(source).hexdigest()!=SOURCE_SHA:
        raise ValueError("Stack export requires the exact reviewed source")
    def uint(p):
        value=0
        for shift in range(0,35,7):
            c=source[p]; p+=1; value|=(c&127)<<shift
            if c<128: return value,p
        raise ValueError("Invalid u32 LEB")
    sections=[]; p=8
    while p<len(source):
        start=p; kind=source[p]; size,p=uint(p+1); end=p+size
        if end>len(source): raise ValueError("Truncated section")
        sections.append((kind,start,p,end));p=end
    exports=next(s for s in sections if s[0]==7)
    count,p=uint(exports[2]); entries_start=p; names={}
    for _ in range(count):
        size,p=uint(p); name=source[p:p+size];p+=size
        kind=source[p];index,p=uint(p+1); names[name]=(kind,index)
    if names[b"stackSave"]!=(0,822) or names[b"stackRestore"]!=(0,823) or EXPORT in names:
        raise ValueError("Unexpected stack exports")
    code=next(s for s in sections if s[0]==10); _,p=uint(code[2]); bodies=[]
    for index in range(4):
        size,p=uint(p);bodies.append(source[p:p+size]);p+=size
    # Exactly: no locals; global.get 7; end. Restore: local.get 0; global.set 7.
    if bodies[2]!=bytes.fromhex("0023070b") or bodies[3]!=bytes.fromhex("00200024070b"):
        raise ValueError("Stack helper is not the verified pure global accessor")
    payload=leb(count+1)+source[entries_start:exports[3]]+leb(len(EXPORT))+EXPORT+bytes([3])+leb(7)
    return source[:exports[1]]+bytes([7])+leb(len(payload))+payload+source[exports[3]:]
if __name__=="__main__":
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument("source",type=Path);parser.add_argument("output",type=Path)
    args=parser.parse_args();data=transform(args.source.read_bytes());args.output.write_bytes(data)
    print(f"Verified stack-global export: {len(data)} bytes SHA256 {hashlib.sha256(data).hexdigest()}")
