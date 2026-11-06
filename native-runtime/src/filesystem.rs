//! Read-only UnityWebData namespace. Guest paths never open host filesystem paths.
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::{collections::BTreeMap, fs::File, io::{Read, Seek}};

// Keep positional reads independent of the handle's shared seek position.
// This is also used by private mmap, which must not advance an open asset fd.
#[cfg(unix)]
fn read_at(file: &File, output: &mut [u8], offset: u64) -> std::io::Result<usize> {
    std::os::unix::fs::FileExt::read_at(file, output, offset)
}

#[cfg(windows)]
fn read_at(file: &File, output: &mut [u8], offset: u64) -> std::io::Result<usize> {
    std::os::windows::fs::FileExt::seek_read(file, output, offset)
}

pub(super) const ENOENT: i32 = 2;
pub(super) const EBADF: i32 = 9;
pub(super) const EACCES: i32 = 13;
pub(super) const EEXIST: i32 = 17;
pub(super) const ENOTDIR: i32 = 20;
pub(super) const EISDIR: i32 = 21;
pub(super) const EINVAL: i32 = 22;
pub(super) const EROFS: i32 = 30;
type FsResult<T> = std::result::Result<T, i32>;

#[derive(Clone)]
struct Node { offset: u64, size: u64, inode: u32, directory: bool }
struct Handle { path: String, position: u64, flags: u32, close_on_exec: bool, directory_entries:Option<Vec<String>> }

#[derive(Clone, Serialize)]
pub struct FsReport {
    pub mounted: bool,
    pub asset_files: usize,
    pub read_bytes: u64,
    pub opened_files: Vec<String>,
    pub saves: Option<super::saves::SaveReport>,
}

pub(super) struct AssetFs {
    file: File,
    nodes: BTreeMap<String, Node>,
    handles: BTreeMap<i32, Handle>,
    report: FsReport,
    saves: Option<super::saves::SaveStore>,
}

fn u32_at(file: &mut File) -> Result<u32> {
    let mut bytes = [0; 4]; file.read_exact(&mut bytes)?; Ok(u32::from_le_bytes(bytes))
}

impl AssetFs {
    pub(super) fn mount(path: &str) -> Result<Self> {
        let mut file = File::open(path).context("Open local UnityWebData container")?;
        let file_size = file.metadata()?.len();
        let mut magic = [0; 16]; file.read_exact(&mut magic)?;
        ensure!(&magic == b"UnityWebData1.0\0", "Not a UnityWebData1.0 container");
        let header_end = u32_at(&mut file)? as u64;
        ensure!((20..=1024*1024).contains(&header_end) && header_end <= file_size, "Invalid UnityWebData header size");
        let mut nodes = BTreeMap::new();
        nodes.insert("/".into(), Node { offset: 0, size: 4096, inode: 1, directory: true });
        let mut ranges = Vec::new();
        let mut assets = 0;
        while file.stream_position()? < header_end {
            ensure!(header_end - file.stream_position()? >= 12 && assets < 4096, "Invalid UnityWebData directory");
            let offset = u32_at(&mut file)? as u64;
            let size = u32_at(&mut file)? as u64;
            let length = u32_at(&mut file)? as u64;
            ensure!(length > 0 && length <= 4096 && file.stream_position()? + length <= header_end, "Invalid UnityWebData path length");
            let mut name = vec![0; length as usize]; file.read_exact(&mut name)?;
            let name = String::from_utf8(name).context("Non UTF-8 UnityWebData name")?;
            ensure!(!name.starts_with('/') && !name.contains(['\\', '\0'])
                && name.split('/').all(|part| !part.is_empty() && part != "." && part != ".."), "Unsafe UnityWebData name");
            ensure!(offset >= header_end && offset.checked_add(size).is_some_and(|end| end <= file_size), "UnityWebData range outside container");
            let mut directory = String::new();
            let parts: Vec<_> = name.split('/').collect();
            for part in &parts[..parts.len()-1] {
                directory.push('/'); directory.push_str(part);
                if let Some(existing) = nodes.get(&directory) { ensure!(existing.directory, "Asset path crosses a file"); }
                else { nodes.insert(directory.clone(), Node {offset:0, size:4096, inode:nodes.len() as u32+1, directory:true}); }
            }
            let key = format!("/{name}");
            ensure!(!nodes.contains_key(&key), "Duplicate/conflicting UnityWebData path");
            nodes.insert(key, Node { offset, size, inode: nodes.len() as u32 + 1, directory: false });
            if size != 0 { ranges.push((offset, offset + size)); }
            assets += 1;
        }
        ranges.sort_unstable();
        ensure!(ranges.windows(2).all(|pair| pair[0].1 <= pair[1].0), "Overlapping UnityWebData files");
        Ok(Self { file, nodes, handles:BTreeMap::new(), saves:None, report:FsReport {mounted:true, asset_files:assets, read_bytes:0, opened_files:Vec::new(),saves:None} })
    }

    pub(super) fn mount_saves(&mut self,path:&std::path::Path)->Result<()> {
        ensure!(self.saves.is_none() && !self.nodes.keys().any(|path|path=="/idbfs" || path.starts_with("/idbfs/")),"Save mount conflicts with an existing namespace");
        let saves=super::saves::SaveStore::load(path)?;
        for (path,directory,size) in saves.nodes(){
            self.nodes.insert(path.into(),Node{offset:0,size:if directory{4096}else{size},inode:self.nodes.len() as u32+1,directory});
        }
        self.saves=Some(saves);Ok(())
    }
    fn save_path(&self,key:&str)->bool {self.saves.is_some() && (key=="/idbfs" || key.starts_with("/idbfs/"))}
    fn refresh_save(&mut self,key:&str){
        let (_,directory,size)=self.saves.as_ref().unwrap().nodes().find(|(path,_,_)|*path==key).unwrap();
        let inode=self.nodes.get(key).map(|node|node.inode).unwrap_or(self.nodes.len() as u32+1);
        self.nodes.insert(key.into(),Node{offset:0,size:if directory{4096}else{size},inode,directory});
    }
    pub(super) fn saves_are_dirty(&self) -> bool {
        self.saves.as_ref().is_some_and(|saves| saves.is_dirty())
    }

    pub(super) fn sync_saves(&mut self)->Result<()> {
        self.saves.as_mut().context("Writable save storage is not mounted")?.sync()
    }

    fn normalize(&self, path: &str) -> FsResult<String> {
        if path.is_empty() { return Err(ENOENT); }
        if path.contains(['\0', '\\']) { return Err(EINVAL); }
        let mut parts: Vec<&str> = Vec::new();
        for part in path.split('/') {
            if !parts.is_empty() {
                let current = format!("/{}", parts.join("/"));
                if self.nodes.get(&current).is_some_and(|node| !node.directory) { return Err(ENOTDIR); }
                if !self.nodes.contains_key(&current) { return Err(ENOENT); }
            }
            match part { ""|"." => {}, ".." => {parts.pop();}, _ => parts.push(part) }
        }
        Ok(format!("/{}", parts.join("/")))
    }

    pub(super) fn open(&mut self, path: &str, flags: u32) -> FsResult<i32> {
        let key = self.normalize(path)?;
        if flags & 3 == 3 {return Err(EINVAL);}
        if self.save_path(&key){
            if flags & 512!=0 && flags & 3==0{return Err(EACCES);}
            let fd=(3..1024).find(|fd|!self.handles.contains_key(fd)).ok_or(24)?;
            let node=self.nodes.get(&key);
            if node.is_some() && flags & (64|128)==(64|128){return Err(EEXIST);}
            if let Some(node)=node {
                if node.directory && flags & (3|512)!=0{return Err(EISDIR);}
                if !node.directory && flags & 65536!=0{return Err(ENOTDIR);}
            }else{
                if flags & 64==0{return Err(ENOENT);}
                if flags & 65536!=0{return Err(ENOTDIR);}
                self.saves.as_mut().unwrap().create(&key,false)?;
            }
            if flags & 512!=0 {
                self.saves.as_mut().unwrap().truncate(&key)?;
            }
            self.refresh_save(&key);
            self.handles.insert(fd,Handle{path:key.clone(),position:0,flags,close_on_exec:flags & 524288!=0,directory_entries:None});
            if self.report.opened_files.len()<64{self.report.opened_files.push(key);}
            return Ok(fd);
        }
        let Some(node) = self.nodes.get(&key) else { return Err(if flags & 64 != 0 {EROFS} else {ENOENT}); };
        if flags & 3 == 3 { return Err(EINVAL); }
        if flags & (3 | 64 | 512) != 0 { return Err(EROFS); }
        if flags & 65536 != 0 && !node.directory { return Err(ENOTDIR); }
        let fd = (3..1024).find(|fd| !self.handles.contains_key(fd)).ok_or(24)?;
        self.handles.insert(fd, Handle {path:key.clone(), position:0, flags, close_on_exec:flags & 524288 != 0,directory_entries:None});
        if self.report.opened_files.len() < 64 { self.report.opened_files.push(key); }
        Ok(fd)
    }

    pub(super) fn close(&mut self, fd:i32) -> FsResult<i32> {
        self.handles.remove(&fd).map(|_|0).ok_or(EBADF)
    }
    pub(super) fn getdents(&mut self,fd:i32,output:&mut[u8])->FsResult<usize>{
        // Legacy pinned JS ABI, not Linux's modern 64-bit dirent layout:
        // ino:u32, offset:u32, reclen:u16, type:u8, name[256], padding.
        const RECORD:usize=268;
        let handle=self.handles.get(&fd).ok_or(EBADF)?;
        let node=self.nodes.get(&handle.path).ok_or(ENOENT)?;
        if !node.directory{return Err(ENOTDIR);}
        if handle.directory_entries.is_none(){
            let prefix=if handle.path=="/"{"/".to_string()}else{format!("{}/",handle.path)};
            let mut entries=vec![".".to_string(),"..".to_string()];
            entries.extend(self.nodes.keys().filter_map(|path|path.strip_prefix(&prefix)).filter(|name|!name.is_empty()&&!name.contains('/')).map(str::to_string));
            self.handles.get_mut(&fd).unwrap().directory_entries=Some(entries);
        }
        let mut position=0;
        while position+RECORD<=output.len(){
            let handle=self.handles.get_mut(&fd).unwrap();
            let Some(name)=handle.directory_entries.as_mut().unwrap().pop()else{break;};
            let (inode,kind)=if name.starts_with('.') {(1,4u8)}else{
                let child=if handle.path=="/"{format!("/{name}")}else{format!("{}/{name}",handle.path)};
                let node=self.nodes.get(&child).ok_or(ENOENT)?;(node.inode,if node.directory{4}else{8})
            };
            let record=&mut output[position..position+RECORD];record.fill(0);
            record[..4].copy_from_slice(&inode.to_le_bytes());record[4..8].copy_from_slice(&(handle.position as u32).to_le_bytes());
            record[8..10].copy_from_slice(&(RECORD as u16).to_le_bytes());record[10]=kind;
            let mut length=name.len().min(255);while !name.is_char_boundary(length){length-=1;}record[11..11+length].copy_from_slice(&name.as_bytes()[..length]);
            position+=RECORD;
        }Ok(position)
    }

    pub(super) fn read(&mut self, fd:i32, output:&mut [u8]) -> FsResult<usize> {
        let position = self.handles.get(&fd).ok_or(EBADF)?.position;
        let count = self.pread(fd, position, output)?;
        self.handles.get_mut(&fd).unwrap().position += count as u64;
        Ok(count)
    }

    pub(super) fn pread(&mut self, fd:i32, position:u64, output:&mut [u8]) -> FsResult<usize> {
        let handle = self.handles.get(&fd).ok_or(EBADF)?;
        if handle.flags & 3==1{return Err(EBADF);}
        let node = self.nodes.get(&handle.path).ok_or(ENOENT)?;
        if node.directory { return Err(EISDIR); }
        if self.save_path(&handle.path){
            let count=self.saves.as_ref().unwrap().read(&handle.path,position,output)?;
            self.report.read_bytes+=count as u64;return Ok(count);
        }
        let length = output.len().min(node.size.saturating_sub(position) as usize);
        if length == 0 { return Ok(0); }
        let count = read_at(&self.file, &mut output[..length], node.offset + position).map_err(|_|5)?;
        self.report.read_bytes += count as u64;
        Ok(count)
    }

    pub(super) fn seek(&mut self, fd:i32, offset:i64, whence:u32) -> FsResult<u64> {
        let handle = self.handles.get_mut(&fd).ok_or(EBADF)?;
        let node = self.nodes.get(&handle.path).ok_or(ENOENT)?;
        let base = match whence {0=>0, 1=>handle.position, 2=>if node.directory{0}else{node.size}, _=>return Err(EINVAL)};
        let position = (base as i128) + (offset as i128);
        if position < 0 || position > i64::MAX as i128 { return Err(EINVAL); }
        handle.position = position as u64;
        Ok(handle.position)
    }

    pub(super) fn write(&mut self,fd:i32,input:&[u8])->FsResult<usize>{
        let handle=self.handles.get(&fd).ok_or(EBADF)?;
        if handle.flags & 3==0{return Err(EBADF);}
        if !self.save_path(&handle.path){return Err(EROFS);}
        let path=handle.path.clone();
        let synchronous=handle.flags & 4096!=0;
        let position=if handle.flags & 1024!=0 {self.nodes.get(&path).unwrap().size}else{handle.position};
        let count=self.saves.as_mut().unwrap().write(&path,position,input)?;
        self.handles.get_mut(&fd).unwrap().position=position+count as u64;
        self.refresh_save(&path);
        if synchronous{self.sync_saves().map_err(|_|5)?;}
        Ok(count)
    }

    pub(super) fn stat(&self, path:&str) -> FsResult<[u8;76]> {
        let key = self.normalize(path)?;
        let node = self.nodes.get(&key).ok_or(ENOENT)?;
        let mut result = [0;76];
        // Exact 32-bit struct stat layout used by the reviewed Emscripten JS.
        let mode=if self.save_path(&key) {if node.directory {0o40700}else{0o100600}}
            else if node.directory {0o40555}else{0o100444};
        for (offset,value) in [(0,1),(8,node.inode),(12,mode),
            (16,1),(36,node.size as u32),(40,4096),(44,node.size.div_ceil(512) as u32),(72,node.inode)] {
            result[offset..offset+4].copy_from_slice(&value.to_le_bytes());
        }
        Ok(result)
    }

    pub(super) fn fstat(&self, fd:i32) -> FsResult<[u8;76]> {
        self.stat(&self.handles.get(&fd).ok_or(EBADF)?.path)
    }

    pub(super) fn check_mapping(&self, fd:i32, offset:u64, length:usize, prot:u32, flags:u32)->FsResult<()> {
        let handle=self.handles.get(&fd).ok_or(EBADF)?;
        let node=self.nodes.get(&handle.path).ok_or(ENOENT)?;
        if node.directory {return Err(EISDIR);}
        if length==0 || flags & 16 != 0 || !matches!(flags & 3,1|2) {return Err(EINVAL);}
        if prot & 2 != 0 && flags & 3 == 1 {return Err(EACCES);}
        // The last partial page is zero-filled. Larger beyond-EOF mappings
        // would require SIGBUS semantics that a wasm32 heap cannot provide.
        let limit=node.size.div_ceil(4096)*4096;
        if offset>=limit || offset.checked_add(length as u64).is_none_or(|end|end>limit){return Err(EINVAL);}
        Ok(())
    }

    pub(super) fn access(&self, path:&str, mode:u32) -> FsResult<i32> {
        if mode & !7 != 0 {return Err(EINVAL);}
        let key=self.normalize(path)?; let node=self.nodes.get(&key).ok_or(ENOENT)?;
        if (mode & 2 != 0 && !self.save_path(&key)) || (mode & 1 != 0 && !node.directory) {return Err(EACCES);}
        Ok(0)
    }

    pub(super) fn mkdir(&mut self,path:&str)->FsResult<i32>{
        // A trailing slash requires a directory, but the final component is
        // precisely the directory mkdir is about to create. normalize would
        // otherwise require that nonexistent component to exist as a parent.
        let trimmed=path.trim_end_matches('/');
        let key=self.normalize(if trimmed.is_empty()&&!path.is_empty(){"/"}else{trimmed})?;
        if self.nodes.contains_key(&key){return Err(EEXIST);}
        if self.save_path(&key){
            self.saves.as_mut().unwrap().create(&key,true)?;self.refresh_save(&key);return Ok(0);
        }
        // normalize has already validated every parent component. This mount
        // is read-only, so an otherwise valid creation fails with EROFS.
        Err(EROFS)
    }

    pub(super) fn fcntl(&mut self, fd:i32, cmd:u32, argument:u32) -> FsResult<i32> {
        let handle=self.handles.get_mut(&fd).ok_or(EBADF)?;
        match cmd {
            1=>Ok(handle.close_on_exec as i32),
            2=>{handle.close_on_exec=argument & 1 != 0;Ok(0)},
            3=>Ok(handle.flags as i32),
            4=>{handle.flags=(handle.flags & !(1024|2048)) | (argument & (1024|2048));Ok(0)},
            _=>Err(EINVAL),
        }
    }

    pub(super) fn report(&self) -> FsReport {
        let mut report=self.report.clone();report.saves=self.saves.as_ref().map(|saves|saves.report());report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn container(name:&str, contents:&[u8]) -> std::path::PathBuf {
        let header_end=20+12+name.len() as u32;
        let path=std::env::temp_dir().join(format!("raven-fs-{}-{}.data",std::process::id(),name.replace('/',"_")));
        let mut file=File::create(&path).unwrap();file.write_all(b"UnityWebData1.0\0").unwrap();
        for value in [header_end,header_end,contents.len() as u32,name.len() as u32] {file.write_all(&value.to_le_bytes()).unwrap();}
        file.write_all(name.as_bytes()).unwrap();file.write_all(contents).unwrap();path
    }
    #[test]fn legacy_directory_records_are_paged_per_descriptor_and_include_real_children(){
        let path=container("Assets/nested/file",b"data");let state=path.with_extension("dents-save");
        let mut fs=AssetFs::mount(path.to_str().unwrap()).unwrap();fs.mount_saves(&state).unwrap();
        fs.mkdir("/idbfs/maps").unwrap();fs.open("/idbfs/maps/user.map",64|2).unwrap();
        let fd=fs.open("/idbfs/maps",65536).unwrap();let second=fs.open("/idbfs/maps",65536).unwrap();let mut page=[0xCC;269];
        assert_eq!(fs.getdents(fd,&mut page[..267]),Ok(0));assert_eq!(fs.getdents(fd,&mut page),Ok(268));
        assert_eq!(&page[8..11],&[12,1,8]);assert_eq!(&page[11..20],b"user.map\0");assert_eq!(page[268],0xCC);
        assert_eq!(fs.getdents(second,&mut page),Ok(268));assert_eq!(&page[11..20],b"user.map\0");
        assert_eq!(fs.getdents(fd,&mut page),Ok(268));assert_eq!(&page[11..14],b"..\0");
        assert_eq!(fs.getdents(fd,&mut page),Ok(268));assert_eq!(&page[11..13],b".\0");assert_eq!(fs.getdents(fd,&mut page),Ok(0));
        let asset=fs.open("/Assets",65536).unwrap();assert_eq!(fs.getdents(asset,&mut page),Ok(268));assert_eq!(page[10],4);assert_eq!(&page[11..18],b"nested\0");
        let file=fs.open("/Assets/nested/file",0).unwrap();assert_eq!(fs.getdents(file,&mut page),Err(ENOTDIR));assert_eq!(fs.getdents(999,&mut page),Err(EBADF));
        fs.close(fd).unwrap();let reopened=fs.open("/idbfs/maps",65536).unwrap();assert_eq!(fs.getdents(reopened,&mut page),Ok(268));assert_eq!(&page[11..20],b"user.map\0");
        drop(fs);std::fs::remove_file(path).unwrap();
    }
    #[test] fn file_reads_seek_and_eof_stay_inside_the_asset_range(){
        let path=container("boot.config",b"hello");let mut fs=AssetFs::mount(path.to_str().unwrap()).unwrap();
        let fd=fs.open("./boot.config",32768).unwrap();let mut out=[255;16];
        assert_eq!(fs.read(fd,&mut out).unwrap(),5);assert_eq!(&out[..5],b"hello");assert_eq!(out[5],255);
        assert_eq!(fs.read(fd,&mut out).unwrap(),0);assert_eq!(fs.seek(fd,-2,2).unwrap(),3);
        assert_eq!(fs.read(fd,&mut out).unwrap(),2);assert_eq!(&out[..2],b"lo");
        assert_eq!(fs.seek(fd,-99,0),Err(EINVAL));fs.close(fd).unwrap();assert_eq!(fs.read(fd,&mut out),Err(EBADF));
        assert_eq!(fs.report().read_bytes,7);std::fs::remove_file(path).unwrap();
    }
    #[test] fn guest_paths_cannot_escape_to_host_files_and_assets_are_read_only(){
        let path=container("Il2CppData/Metadata/test",b"data");let mut fs=AssetFs::mount(path.to_str().unwrap()).unwrap();
        assert_eq!(fs.open("/etc/passwd",0),Err(ENOENT));
        assert_eq!(fs.open("../../etc/passwd",0),Err(ENOENT));
        assert_eq!(fs.open("Il2CppData/Metadata/test/../test",0),Err(ENOTDIR));
        assert_eq!(fs.open("Il2CppData/Metadata/test",2),Err(EROFS));
        assert_eq!(fs.open("new-save",64|1),Err(EROFS));
        assert!(fs.open("Il2CppData/./Metadata/../Metadata/test",0).is_ok());
        assert_eq!(fs.access("Il2CppData/Metadata/test",2),Err(EACCES));
        assert_eq!(fs.mkdir("/"),Err(EEXIST));
        assert_eq!(fs.mkdir("Il2CppData/Metadata"),Err(EEXIST));
        assert_eq!(fs.mkdir("Il2CppData/Metadata/test"),Err(EEXIST));
        assert_eq!(fs.mkdir("new-directory"),Err(EROFS));
        assert_eq!(fs.mkdir("missing/child"),Err(ENOENT));
        assert_eq!(fs.mkdir("Il2CppData/Metadata/test/child"),Err(ENOTDIR));
        assert_eq!(fs.open("new-directory",0),Err(ENOENT));
        let stat=fs.stat("Il2CppData/Metadata/test").unwrap();assert_eq!(u32::from_le_bytes(stat[36..40].try_into().unwrap()),4);
        std::fs::remove_file(path).unwrap();
    }
    #[test] fn positional_reads_preserve_each_guest_handles_offset(){
        let path=container("pread.config",b"abcdef");
        let mut fs=AssetFs::mount(path.to_str().unwrap()).unwrap();
        let first=fs.open("pread.config",0).unwrap();
        let second=fs.open("pread.config",0).unwrap();
        let mut out=[0;2];
        assert_eq!(fs.read(first,&mut out).unwrap(),2);assert_eq!(&out,b"ab");
        assert_eq!(fs.pread(first,4,&mut out).unwrap(),2);assert_eq!(&out,b"ef");
        assert_eq!(fs.read(second,&mut out).unwrap(),2);assert_eq!(&out,b"ab");
        assert_eq!(fs.read(first,&mut out).unwrap(),2);assert_eq!(&out,b"cd");
        assert_eq!(fs.pread(first,u64::MAX,&mut out).unwrap(),0);
        fs.close(first).unwrap();fs.close(second).unwrap();drop(fs);
        std::fs::remove_file(path).unwrap();
    }
    #[test] fn directory_entries_and_bad_containers_are_rejected(){
        let path=container("Resources/test",b"data");let mut fs=AssetFs::mount(path.to_str().unwrap()).unwrap();
        let fd=fs.open("Resources",0).unwrap();assert_eq!(fs.read(fd,&mut[0;4]),Err(EISDIR));
        assert_eq!(fs.open("Resources/test",65536),Err(ENOTDIR));std::fs::remove_file(path).unwrap();
        let path=container("../escape",b"data");assert!(AssetFs::mount(path.to_str().unwrap()).is_err());std::fs::remove_file(path).unwrap();
        let path=container("truncated",b"data");File::options().write(true).open(&path).unwrap().set_len(20).unwrap();
        assert!(AssetFs::mount(path.to_str().unwrap()).is_err());std::fs::remove_file(path).unwrap();
    }
    #[test] fn save_descriptors_obey_permissions_append_seek_and_truncate(){
        let path=container("save-test.config",b"asset");
        let state=path.with_extension("saves");assert!(!state.exists());
        let mut fs=AssetFs::mount(path.to_str().unwrap()).unwrap();fs.mount_saves(&state).unwrap();
        assert_eq!(fs.mkdir("/idbfs/profile/"),Ok(0));
        assert_eq!(fs.mkdir("/idbfs/profile/SaveData/"),Ok(0));
        assert_eq!(fs.mkdir("/idbfs/profile/SaveData/Mods///"),Ok(0));
        assert_eq!(fs.mkdir("/idbfs/profile/SaveData/Mods/"),Err(EEXIST));
        assert_eq!(fs.mkdir("////"),Err(EEXIST));
        assert_eq!(fs.mkdir("/idbfs/missing/child/"),Err(ENOENT));
        assert_eq!(fs.mkdir("/new-directory/"),Err(EROFS));
        assert_eq!(fs.open("/idbfs/fail",64|512),Err(EACCES));assert_eq!(fs.open("/idbfs/fail",0),Err(ENOENT));
        let fd=fs.open("/idbfs/profile/test",64|2).unwrap();assert_eq!(fs.write(fd,b"abc").unwrap(),3);
        assert_eq!(fs.open("/idbfs/profile/test",64|128|2),Err(EEXIST));
        let append=fs.open("/idbfs/profile/test",1|1024).unwrap();fs.seek(append,0,0).unwrap();fs.write(append,b"d").unwrap();
        assert_eq!(fs.read(append,&mut[0;4]),Err(EBADF));
        fs.seek(fd,6,0).unwrap();fs.write(fd,b"z").unwrap();fs.seek(fd,0,0).unwrap();let mut out=[0;7];
        assert_eq!(fs.read(fd,&mut out).unwrap(),7);assert_eq!(&out,b"abcd\0\0z");
        let readonly=fs.open("/idbfs/profile/test",0).unwrap();assert_eq!(fs.write(readonly,b"x"),Err(EBADF));
        assert_eq!(fs.open("/save-test.config",2),Err(EROFS));
        assert_eq!(fs.open("/idbfs/../../escape",64|2),Err(EROFS));
        assert_eq!(fs.access("/idbfs/profile/test",2),Ok(0));fs.sync_saves().unwrap();drop(fs);
        let mut fs=AssetFs::mount(path.to_str().unwrap()).unwrap();fs.mount_saves(&state).unwrap();
        assert_eq!(fs.access("/idbfs/profile/SaveData/Mods/",0),Ok(0));
        let fd=fs.open("/idbfs/profile/test",0).unwrap();assert_eq!(fs.read(fd,&mut out).unwrap(),7);assert_eq!(&out,b"abcd\0\0z");
        let fd=fs.open("/idbfs/profile/test",2|512).unwrap();assert_eq!(fs.read(fd,&mut out).unwrap(),0);
        fs.sync_saves().unwrap();drop(fs);std::fs::remove_file(state).unwrap();std::fs::remove_file(path).unwrap();
    }
}
