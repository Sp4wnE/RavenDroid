//! Confined /idbfs namespace, persisted as an atomic, checksummed snapshot.
//! Guest names are index keys, never host paths; no symlinks are represented.
use anyhow::{Context,Result,ensure};
use serde::{Deserialize,Serialize};
use sha2::{Digest,Sha256};
use std::{collections::BTreeMap,fs::{File,OpenOptions},io::{Read,Write},path::{Path,PathBuf},sync::atomic::{AtomicU64,Ordering}};
use super::filesystem::{ENOENT,EACCES,EEXIST,ENOTDIR,EISDIR,EINVAL};

const MAGIC:&[u8;8]=b"RAVSAVE1";
const MAX_INDEX:usize=1024*1024;
pub(super) const MAX_FILE:usize=8*1024*1024;
const MAX_TOTAL:usize=32*1024*1024;
const MAX_NODES:usize=1024;
type FsResult<T>=std::result::Result<T,i32>;

struct Entry { directory:bool, data:Vec<u8> }
#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct IndexEntry { path:String,directory:bool,bytes:usize }

#[derive(Clone,Serialize)]
pub struct SaveReport { pub files:usize,pub bytes:usize,pub dirty:bool,pub syncs:u64,pub written_bytes:u64 }

pub(super) struct SaveStore {
    path:PathBuf,entries:BTreeMap<String,Entry>,dirty:bool,syncs:u64,written_bytes:u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn path(tag:&str)->PathBuf {
        std::env::temp_dir().join(format!("raven-save-{}-{tag}-{}.bin",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()))
    }
    #[test]
    fn snapshots_replace_previous_state_and_detect_corruption(){
        let path=path("roundtrip");let mut store=SaveStore::load(&path).unwrap();
        store.create("/idbfs/profile",true).unwrap();store.create("/idbfs/profile/save",false).unwrap();
        store.write("/idbfs/profile/save",2,b"one").unwrap();store.sync().unwrap();
        assert!(!store.report().dirty);
        let mut reopened=SaveStore::load(&path).unwrap();let mut out=[99;8];
        assert_eq!(reopened.read("/idbfs/profile/save",0,&mut out).unwrap(),5);assert_eq!(&out[..5],b"\0\0one");
        reopened.write("/idbfs/profile/save",2,b"two").unwrap();reopened.sync().unwrap();
        let reopened=SaveStore::load(&path).unwrap();
        reopened.read("/idbfs/profile/save",0,&mut out).unwrap();assert_eq!(&out[..5],b"\0\0two");
        let mut bytes=std::fs::read(&path).unwrap();let last=bytes.len()-1;bytes[last]^=1;std::fs::write(&path,&bytes).unwrap();
        assert!(SaveStore::load(&path).is_err());assert_eq!(std::fs::read(&path).unwrap(),bytes);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn save_names_and_quotas_cannot_escape_or_partially_mutate_state(){
        let path=path("quota");let mut store=SaveStore::load(&path).unwrap();
        for name in ["/etc/passwd","/idbfs/../outside","/idbfs/a\\b","/idbfs//double","/idbfs/./dot"]{
            assert_eq!(store.create(name,false),Err(EACCES));
        }
        store.create("/idbfs/test",false).unwrap();store.write("/idbfs/test",0,b"old").unwrap();store.sync().unwrap();
        let before=std::fs::read(&path).unwrap();
        assert_eq!(store.write("/idbfs/test",MAX_FILE as u64,b"x"),Err(28));
        assert_eq!(store.write("/idbfs/test",u64::MAX,b"x"),Err(28));
        assert!(!store.report().dirty);assert_eq!(std::fs::read(&path).unwrap(),before);
        let mut out=[0;3];store.read("/idbfs/test",0,&mut out).unwrap();assert_eq!(&out,b"old");
        assert_eq!(store.create("/idbfs/test/child",false),Err(ENOTDIR));
        assert_eq!(store.create("/idbfs/missing/child",false),Err(ENOENT));
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn failed_commit_retains_dirty_state_for_retry(){
        let path=path("retry");let backup=path.with_extension("backup");
        let mut store=SaveStore::load(&path).unwrap();store.create("/idbfs/test",false).unwrap();
        store.write("/idbfs/test",0,b"old").unwrap();store.sync().unwrap();
        let old=std::fs::read(&path).unwrap();std::fs::rename(&path,&backup).unwrap();std::fs::create_dir(&path).unwrap();
        store.write("/idbfs/test",0,b"new").unwrap();assert!(store.sync().is_err());
        assert!(store.report().dirty);assert_eq!(std::fs::read(&backup).unwrap(),old);assert!(path.is_dir());
        std::fs::remove_dir(&path).unwrap();std::fs::rename(&backup,&path).unwrap();store.sync().unwrap();
        let reopened=SaveStore::load(&path).unwrap();let mut out=[0;3];reopened.read("/idbfs/test",0,&mut out).unwrap();assert_eq!(&out,b"new");
        std::fs::remove_file(path).unwrap();
    }
}

fn valid(path:&str)->bool {
    (path=="/idbfs" || path.starts_with("/idbfs/")) && path.len()<=4096
        && !path.contains(['\\','\0']) && path[1..].split('/').all(|part|!part.is_empty() && part!="." && part!="..")
}

impl SaveStore {
    pub(super) fn load(path:&Path)->Result<Self>{
        let mut entries=BTreeMap::new();
        match File::open(path) {
            Err(error) if error.kind()==std::io::ErrorKind::NotFound=>{
                entries.insert("/idbfs".into(),Entry{directory:true,data:Vec::new()});
                Ok(Self{path:path.into(),entries,dirty:true,syncs:0,written_bytes:0})
            }
            Err(error)=>Err(error).context("Open native save snapshot"),
            Ok(mut file)=>{
                let length=file.metadata()?.len();
                ensure!((44..=(MAX_TOTAL+MAX_INDEX+44) as u64).contains(&length),"Invalid save snapshot size");
                let mut header=[0u8;12];file.read_exact(&mut header)?;
                ensure!(&header[..8]==MAGIC,"Invalid save snapshot magic");
                let index_size=u32::from_le_bytes(header[8..12].try_into().unwrap()) as usize;
                ensure!(index_size<=MAX_INDEX && index_size as u64+44<=length,"Invalid save index size");
                let mut index=vec![0;index_size];file.read_exact(&mut index)?;
                let metadata:Vec<IndexEntry>=serde_json::from_slice(&index).context("Parse save index")?;
                ensure!(!metadata.is_empty() && metadata.len()<=MAX_NODES,"Invalid save node count");
                let mut total=0usize;
                for entry in &metadata {
                    ensure!(valid(&entry.path) && entry.bytes<=MAX_FILE && (!entry.directory || entry.bytes==0),"Invalid save entry");
                    total=total.checked_add(entry.bytes).context("Save size overflow")?;
                    ensure!(total<=MAX_TOTAL,"Save quota exceeded");
                }
                ensure!(length==12+index_size as u64+total as u64+32,"Save payload length mismatch");
                let mut hash=Sha256::new();hash.update(header);hash.update(&index);
                for entry in metadata {
                    let mut data=vec![0;entry.bytes];file.read_exact(&mut data)?;hash.update(&data);
                    ensure!(entries.insert(entry.path,Entry{directory:entry.directory,data}).is_none(),"Duplicate save path");
                }
                let mut expected=[0;32];file.read_exact(&mut expected)?;
                let actual:[u8;32]=hash.finalize().into();
                ensure!(actual==expected,"Save snapshot checksum mismatch");
                ensure!(entries.get("/idbfs").is_some_and(|entry|entry.directory),"Save root is missing");
                for path in entries.keys().filter(|path|path.as_str()!="/idbfs") {
                    let parent=path.rsplit_once('/').unwrap().0;
                    ensure!(entries.get(parent).is_some_and(|entry|entry.directory),"Invalid save parent");
                }
                Ok(Self{path:path.into(),entries,dirty:false,syncs:0,written_bytes:0})
            }
        }
    }

    pub(super) fn nodes(&self)->impl Iterator<Item=(&str,bool,u64)> {
        self.entries.iter().map(|(path,entry)|(path.as_str(),entry.directory,entry.data.len() as u64))
    }
    fn total(&self)->usize {self.entries.values().map(|entry|entry.data.len()).sum()}

    pub(super) fn create(&mut self,path:&str,directory:bool)->FsResult<()> {
        if !valid(path){return Err(EACCES);}
        if self.entries.contains_key(path){return Err(EEXIST);}
        let parent=path.rsplit_once('/').ok_or(EINVAL)?.0;
        let entry=self.entries.get(parent).ok_or(ENOENT)?;
        if !entry.directory{return Err(ENOTDIR);}
        if self.entries.len()>=MAX_NODES{return Err(28);}
        self.entries.insert(path.into(),Entry{directory,data:Vec::new()});self.dirty=true;Ok(())
    }
    pub(super) fn truncate(&mut self,path:&str)->FsResult<()> {
        let entry=self.entries.get_mut(path).ok_or(ENOENT)?;
        if entry.directory{return Err(EISDIR);}
        entry.data.clear();self.dirty=true;Ok(())
    }
    pub(super) fn read(&self,path:&str,position:u64,output:&mut[u8])->FsResult<usize>{
        let entry=self.entries.get(path).ok_or(ENOENT)?;
        if entry.directory{return Err(EISDIR);}
        if position>=entry.data.len() as u64{return Ok(0);}
        let position=position as usize;let count=output.len().min(entry.data.len()-position);
        output[..count].copy_from_slice(&entry.data[position..position+count]);Ok(count)
    }
    pub(super) fn write(&mut self,path:&str,position:u64,input:&[u8])->FsResult<usize>{
        let entry=self.entries.get(path).ok_or(ENOENT)?;
        if entry.directory{return Err(EISDIR);}
        if input.is_empty(){return Ok(0);}
        let end=position.checked_add(input.len() as u64).filter(|end|*end<=MAX_FILE as u64).ok_or(28)? as usize;
        let size=entry.data.len().max(end);
        if self.total()-entry.data.len()+size>MAX_TOTAL{return Err(28);}
        let entry=self.entries.get_mut(path).unwrap();entry.data.resize(size,0);
        entry.data[position as usize..end].copy_from_slice(input);
        self.dirty=true;self.written_bytes+=input.len() as u64;Ok(input.len())
    }

    pub(super) fn sync(&mut self)->Result<()> {
        if !self.dirty {self.syncs+=1;return Ok(());}
        static SERIAL:AtomicU64=AtomicU64::new(0);
        let temporary=self.path.with_file_name(format!(".raven-save-{}-{}.partial",std::process::id(),SERIAL.fetch_add(1,Ordering::Relaxed)));
        let mut output=OpenOptions::new().write(true).create_new(true).open(&temporary).context("Create save transaction")?;
        let result=(||->Result<()>{
            let index:Vec<_>=self.nodes().map(|(path,directory,bytes)|IndexEntry{path:path.into(),directory,bytes:bytes as usize}).collect();
            let index=serde_json::to_vec(&index)?;ensure!(index.len()<=MAX_INDEX,"Save index exceeds quota");
            let mut hash=Sha256::new();
            let mut emit=|bytes:&[u8]|->Result<()>{output.write_all(bytes)?;hash.update(bytes);Ok(())};
            emit(MAGIC)?;emit(&(index.len() as u32).to_le_bytes())?;emit(&index)?;
            for entry in self.entries.values(){emit(&entry.data)?;}
            output.write_all(&hash.finalize())?;output.sync_all()?;drop(output);
            std::fs::rename(&temporary,&self.path).context("Commit native save snapshot")?;
            #[cfg(unix)]
            if let Some(parent)=self.path.parent(){File::open(parent)?.sync_all()?;}
            Ok(())
        })();
        if result.is_err(){let _=std::fs::remove_file(&temporary);}
        result?;self.dirty=false;self.syncs+=1;Ok(())
    }
    pub(super) fn is_dirty(&self) -> bool { self.dirty }

    pub(super) fn report(&self)->SaveReport {
        SaveReport{files:self.entries.values().filter(|entry|!entry.directory).count(),bytes:self.total(),dirty:self.dirty,syncs:self.syncs,written_bytes:self.written_bytes}
    }
}
