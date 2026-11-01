//! A hidden, real driver-backed OpenGL ES window. It is never shown or focused.
use anyhow::{Result,ensure};
use std::{ffi::{CStr,CString},ptr::null_mut,collections::BTreeMap,marker::PhantomData,rc::Rc};
use windows_sys::Win32::{Foundation::HWND,Graphics::{Gdi::{HDC,GetDC,ReleaseDC},OpenGL::*},System::LibraryLoader::{GetModuleHandleW,GetProcAddress},UI::WindowsAndMessaging::*};
macro_rules! gl{($context:expr,$name:expr,$signature:ty;$($arg:expr),* $(,)?)=>{{let function:$signature=unsafe{std::mem::transmute($context.procedure($name)?)};unsafe{function($($arg),*)}}};}

pub(super) struct Context {
    window:HWND,dc:HDC,context:HGLRC,width:i32,height:i32,
    symbols:BTreeMap<String,usize>,_owner:PhantomData<Rc<()>>,
    pub(super) version:String,pub(super) renderer:String,pub(super) vendor:String,
    class:Vec<u16>,
    framebuffer:u32,color:u32,depth:u32,
}

fn wide(value:&str)->Vec<u16>{value.encode_utf16().chain(std::iter::once(0)).collect()}
impl Context {
    pub(super) fn post_space(&self,down:bool)->Result<()>{
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
        let scan=unsafe{MapVirtualKeyW(VK_SPACE as u32,MAPVK_VK_TO_VSC)};
        let flags=1u32|(scan<<16)|if down{0}else{(1<<30)|(1<<31)};
        ensure!(unsafe{PostMessageW(self.window,if down{WM_KEYDOWN}else{WM_KEYUP},VK_SPACE as usize,flags as isize)}!=0,"Post test Space to the owned native window");Ok(())
    }
    pub(super) fn next_key(&mut self)->Result<Option<(crate::input::Key,MSG)>>{
        let mut message=MSG::default();
        for _ in 0..256{
            if unsafe{PeekMessageW(&mut message,self.window,0,0,PM_REMOVE)}==0{break;}
            if let Some(key)=crate::input::native_key(&message){return Ok(Some((key,message)));}
            unsafe{TranslateMessage(&message);DispatchMessageW(&message);}
        }Ok(None)
    }
    pub(super) fn finish_key(&self,message:&MSG,prevent_default:bool){
        if !prevent_default{unsafe{TranslateMessage(message);DispatchMessageW(message);}}
    }
    pub(super) fn new(width:i32,height:i32)->Result<Self>{
        ensure!((1..=8192).contains(&width) && (1..=8192).contains(&height),"Invalid native surface size");
        let mut this=Self{window:null_mut(),dc:null_mut(),context:null_mut(),width,height,symbols:BTreeMap::new(),_owner:PhantomData,version:String::new(),renderer:String::new(),vendor:String::new(),class:Vec::new(),framebuffer:0,color:0,depth:0};
        // SAFETY: fixed system window class, owned HWND/DC/context; every handle
        // is checked and released by Drop on this same owner thread.
        unsafe {
            static NEXT:std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(1);
            let class=wide(&format!("RavenfieldNativeSurface-{}",NEXT.fetch_add(1,std::sync::atomic::Ordering::Relaxed)));
            let instance=GetModuleHandleW(std::ptr::null());
            let descriptor=WNDCLASSW{lpfnWndProc:Some(DefWindowProcW),hInstance:instance,lpszClassName:class.as_ptr(),hCursor:LoadCursorW(null_mut(),IDC_ARROW),..Default::default()};
            ensure!(RegisterClassW(&descriptor)!=0,"Register owned native surface class");this.class=class;
            this.window=CreateWindowExW(0,this.class.as_ptr(),wide("Ravenfield native offscreen host").as_ptr(),WS_POPUP,0,0,width,height,null_mut(),null_mut(),instance,null_mut());
            ensure!(!this.window.is_null(),"Create native window: {}",std::io::Error::last_os_error());
            this.dc=GetDC(this.window);ensure!(!this.dc.is_null(),"Get native surface DC");
            let pfd=PIXELFORMATDESCRIPTOR{nSize:std::mem::size_of::<PIXELFORMATDESCRIPTOR>() as u16,nVersion:1,dwFlags:PFD_DRAW_TO_WINDOW|PFD_SUPPORT_OPENGL|PFD_DOUBLEBUFFER,cColorBits:32,cAlphaBits:8,cDepthBits:24,cStencilBits:8,..Default::default()};
            let format=ChoosePixelFormat(this.dc,&pfd);ensure!(format!=0 && SetPixelFormat(this.dc,format,&pfd)!=0,"Set native pixel format");
            this.context=wglCreateContext(this.dc);ensure!(!this.context.is_null() && wglMakeCurrent(this.dc,this.context)!=0,"Bootstrap WGL context");
            let query=this.procedure("wglGetExtensionsStringARB")?;
            let query:unsafe extern "system" fn(HDC)->*const u8=std::mem::transmute(query);
            let extensions=query(this.dc);ensure!(!extensions.is_null(),"Missing WGL extension list");
            let extensions=CStr::from_ptr(extensions.cast()).to_string_lossy();
            ensure!(extensions.contains("WGL_EXT_create_context_es"),"Driver does not support native OpenGL ES contexts");
            ensure!(extensions.contains("WGL_ARB_create_context_robustness"),"Driver lacks robust native buffer access");
            let create=this.procedure("wglCreateContextAttribsARB")?;
            let create:unsafe extern "system" fn(HDC,HGLRC,*const i32)->HGLRC=std::mem::transmute(create);
            let attributes=[0x2091,3,0x2092,0,0x9126,4,0x2094,4,0x8256,0x8252,0];
            let context=create(this.dc,null_mut(),attributes.as_ptr());ensure!(!context.is_null(),"Create OpenGL ES 3 context: {}",std::io::Error::last_os_error());
            let old=this.context;
            if wglMakeCurrent(this.dc,context)==0 {wglDeleteContext(context);anyhow::bail!("Make native GLES context current");}
            this.context=context;wglDeleteContext(old);this.symbols.clear();
            this.version=this.string(GL_VERSION)?;this.renderer=this.string(GL_RENDERER)?;this.vendor=this.string(GL_VENDOR)?;
            ensure!(this.version.starts_with("OpenGL ES 3"),"Driver returned a non-ES3 context: {}",this.version);
            let mut robust=0;glGetIntegerv(0x90F3,&mut robust);
            ensure!(glGetError()==0 && robust!=0,"Driver did not enable robust buffer access");
            glViewport(0,0,width,height);
            glClear(GL_COLOR_BUFFER_BIT|GL_DEPTH_BUFFER_BIT|GL_STENCIL_BUFFER_BIT);
        }
        this.allocate_surface()?;
        gl!(&mut this,"glBindFramebuffer",unsafe extern "system" fn(u32,u32);0x8D40,this.framebuffer);
        unsafe{glClear(GL_COLOR_BUFFER_BIT|GL_DEPTH_BUFFER_BIT|GL_STENCIL_BUFFER_BIT);}
        Ok(this)
    }
    pub(super) fn procedure(&mut self,name:&str)->Result<usize>{
        self.make_current(true)?;
        if let Some(address)=self.symbols.get(name){return Ok(*address);}
        let name_c=CString::new(name)?;
        // SAFETY: current context is owned by this thread; fixed symbol name.
        let address=unsafe {
            let address=wglGetProcAddress(name_c.as_ptr().cast()).map(|function|function as usize).unwrap_or(0);
            if matches!(address,0|1|2|3|usize::MAX){
                let module=GetModuleHandleW(wide("opengl32.dll").as_ptr());
                GetProcAddress(module,name_c.as_ptr().cast()).map(|function|function as usize).unwrap_or(0)
            }else{address}
        };
        ensure!(address!=0,"Native GLES symbol unavailable: {name}");self.symbols.insert(name.into(),address);Ok(address)
    }
    pub(super) fn string(&mut self,name:u32)->Result<String>{
        self.make_current(true)?;
        // Driver-owned, NUL-terminated string; never a guest-controlled pointer.
        let value=unsafe{glGetString(name)};ensure!(!value.is_null(),"Native GL string query failed: {name:#x}");
        Ok(unsafe{CStr::from_ptr(value.cast())}.to_string_lossy().into_owned())
    }
    pub(super) fn dimensions(&self)->(i32,i32){(self.width,self.height)}
    pub(super) fn framebuffer(&self)->u32{self.framebuffer}
    fn allocate_surface(&mut self)->Result<()> {
        self.make_current(true)?;
        let(mut texture,mut renderbuffer,mut draw,mut read)=(0,0,0,0);
        unsafe{glGetIntegerv(0x8069,&mut texture);glGetIntegerv(0x8CA7,&mut renderbuffer);glGetIntegerv(0x8CA6,&mut draw);glGetIntegerv(0x8CAA,&mut read);}
        if self.framebuffer==0{
            let(mut framebuffer,mut color,mut depth)=(0,0,0);
            gl!(self,"glGenFramebuffers",unsafe extern "system" fn(i32,*mut u32);1,&mut framebuffer);
            gl!(self,"glGenTextures",unsafe extern "system" fn(i32,*mut u32);1,&mut color);
            gl!(self,"glGenRenderbuffers",unsafe extern "system" fn(i32,*mut u32);1,&mut depth);
            self.framebuffer=framebuffer;self.color=color;self.depth=depth;
        }
        gl!(self,"glBindTexture",unsafe extern "system" fn(u32,u32);0xDE1,self.color);
        gl!(self,"glTexImage2D",unsafe extern "system" fn(u32,i32,i32,i32,i32,i32,u32,u32,*const u8);0xDE1,0,0x8058,self.width,self.height,0,0x1908,0x1401,std::ptr::null());
        unsafe{glTexParameteri(0xDE1,0x2801,0x2600);glTexParameteri(0xDE1,0x2800,0x2600);}
        gl!(self,"glBindRenderbuffer",unsafe extern "system" fn(u32,u32);0x8D41,self.depth);
        gl!(self,"glRenderbufferStorage",unsafe extern "system" fn(u32,u32,i32,i32);0x8D41,0x88F0,self.width,self.height);
        gl!(self,"glBindFramebuffer",unsafe extern "system" fn(u32,u32);0x8D40,self.framebuffer);
        gl!(self,"glFramebufferTexture2D",unsafe extern "system" fn(u32,u32,u32,u32,i32);0x8D40,0x8CE0,0xDE1,self.color,0);
        gl!(self,"glFramebufferRenderbuffer",unsafe extern "system" fn(u32,u32,u32,u32);0x8D40,0x821A,0x8D41,self.depth);
        let status=gl!(self,"glCheckFramebufferStatus",unsafe extern "system" fn(u32)->u32;0x8D40);
        gl!(self,"glBindTexture",unsafe extern "system" fn(u32,u32);0xDE1,texture as u32);
        gl!(self,"glBindRenderbuffer",unsafe extern "system" fn(u32,u32);0x8D41,renderbuffer as u32);
        gl!(self,"glBindFramebuffer",unsafe extern "system" fn(u32,u32);0x8CA9,draw as u32);
        gl!(self,"glBindFramebuffer",unsafe extern "system" fn(u32,u32);0x8CA8,read as u32);
        ensure!(status==0x8CD5,"Native offscreen surface incomplete: {status:#x}");
        ensure!(unsafe{glGetError()}==0,"Native offscreen surface allocation failed");Ok(())
    }
    pub(super) fn read_surface(&mut self)->Result<(Vec<u8>,Vec<u32>)>{
        self.make_current(true)?;
        let bind_framebuffer:unsafe extern "system" fn(u32,u32)=unsafe{std::mem::transmute(self.procedure("glBindFramebuffer")?)};
        let bind_buffer:unsafe extern "system" fn(u32,u32)=unsafe{std::mem::transmute(self.procedure("glBindBuffer")?)};
        let read_buffer:unsafe extern "system" fn(u32)=unsafe{std::mem::transmute(self.procedure("glReadBuffer")?)};
        let mut old_framebuffer=0;let mut old_buffer=0;let mut old_read=0;let mut old_pack=[0i32;4];let mut errors=Vec::new();
        let mut pixels=vec![0u8;self.width as usize*self.height as usize*4];
        unsafe{
            for _ in 0..16{let error=glGetError();if error==0{break;}errors.push(error);}
            glGetIntegerv(0x8CAA,&mut old_framebuffer);glGetIntegerv(0x88ED,&mut old_buffer);
            for (parameter,value) in [0xD05,0xD02,0xD03,0xD04].into_iter().zip(old_pack.iter_mut()){glGetIntegerv(parameter,value);}
            bind_framebuffer(0x8CA8,self.framebuffer);glGetIntegerv(0xC02,&mut old_read);read_buffer(0x8CE0);bind_buffer(0x88EB,0);
            for (parameter,value) in [(0xD05,1),(0xD02,0),(0xD03,0),(0xD04,0)]{glPixelStorei(parameter,value);}
            glFinish();glReadPixels(0,0,self.width,self.height,GL_RGBA,GL_UNSIGNED_BYTE,pixels.as_mut_ptr().cast());
            let error=glGetError();
            read_buffer(old_read as u32);bind_framebuffer(0x8CA8,old_framebuffer as u32);bind_buffer(0x88EB,old_buffer as u32);
            for (parameter,value) in [0xD05,0xD02,0xD03,0xD04].into_iter().zip(old_pack){glPixelStorei(parameter,value);}
            ensure!(error==0,"Native surface readback failed: {error:#x}");
        }Ok((pixels,errors))
    }
    pub(super) fn cursor_visible(&self,visible:bool)->Result<()> {
        // Only this context's private window class changes. Never modify the
        // desktop cursor counter or the system STATIC class.
        unsafe {
            let cursor=if visible{LoadCursorW(null_mut(),IDC_ARROW)}else{null_mut()};
            ensure!(!visible || !cursor.is_null(),"Load native arrow cursor");
            SetClassLongPtrW(self.window,GCLP_HCURSOR,cursor as isize);
            ensure!(GetClassLongPtrW(self.window,GCLP_HCURSOR)==cursor as usize,"Set native surface cursor");
        }Ok(())
    }
    pub(super) fn extensions(&mut self)->Result<std::collections::BTreeSet<String>>{
        self.make_current(true)?;let mut count=0i32;unsafe{glGetIntegerv(0x821D,&mut count);}
        ensure!((0..=4096).contains(&count),"Invalid native extension count");
        let function:unsafe extern "system" fn(u32,u32)->*const u8=unsafe{std::mem::transmute(self.procedure("glGetStringi")?)};
        let mut extensions=std::collections::BTreeSet::new();
        for index in 0..count {
            let value=unsafe{function(GL_EXTENSIONS,index as u32)};ensure!(!value.is_null(),"Native extension query failed");
            extensions.insert(unsafe{CStr::from_ptr(value.cast())}.to_string_lossy().into_owned());
        }
        Ok(extensions)
    }
    pub(super) fn make_current(&self,active:bool)->Result<()> {
        let success=unsafe{if active {
            wglGetCurrentContext()==self.context || wglMakeCurrent(self.dc,self.context)!=0
        }else{wglMakeCurrent(null_mut(),null_mut())!=0}};
        ensure!(success,"Change native GLES context binding");Ok(())
    }
    pub(super) fn resize(&mut self,width:i32,height:i32)->Result<()> {
        ensure!((1..=8192).contains(&width) && (1..=8192).contains(&height),"Invalid native surface resize");
        ensure!(unsafe{SetWindowPos(self.window,null_mut(),0,0,width,height,SWP_NOACTIVATE|SWP_NOZORDER|SWP_NOMOVE)}!=0,"Resize native surface");
        let mut rect=Default::default();ensure!(unsafe{GetClientRect(self.window,&mut rect)}!=0,"Read native surface size");
        ensure!(rect.right-rect.left==width && rect.bottom-rect.top==height,"Native surface dimensions mismatch");
        self.width=width;self.height=height;self.allocate_surface()
    }
}
impl Drop for Context {
    fn drop(&mut self){unsafe{
        if !self.context.is_null(){if wglGetCurrentContext()==self.context{wglMakeCurrent(null_mut(),null_mut());}wglDeleteContext(self.context);}
        if !self.dc.is_null(){ReleaseDC(self.window,self.dc);}
        if !self.window.is_null(){DestroyWindow(self.window);}
        if !self.class.is_empty(){UnregisterClassW(self.class.as_ptr(),GetModuleHandleW(std::ptr::null()));}
    }}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore="Requires an interactive Windows GPU driver; run explicitly for graphics validation"]
    fn actual_es_context_clears_and_reads_back_pixels(){
        let mut context=Context::new(16,16).unwrap();
        let mut pixel=[0u8;4];unsafe{glClearColor(0.25,0.5,0.75,1.0);glClear(GL_COLOR_BUFFER_BIT);glFinish();glReadPixels(0,0,1,1,GL_RGBA,GL_UNSIGNED_BYTE,pixel.as_mut_ptr().cast());}
        assert_eq!(unsafe{glGetError()},0);
        assert!(pixel.iter().zip([64u8,128,191,255]).all(|(actual,expected)|actual.abs_diff(expected)<=1));
        assert!(context.version.starts_with("OpenGL ES 3"));
        context.post_space(true).unwrap();let (key,message)=context.next_key().unwrap().unwrap();
        assert!(key.event==2&&key.key_code==32&&key.code=="Space");context.finish_key(&message,false);
        let (key,message)=context.next_key().unwrap().unwrap();assert!(key.event==1&&key.char_code==32);context.finish_key(&message,false);
        context.post_space(false).unwrap();
        let (key,message)=context.next_key().unwrap().unwrap();assert!(key.event==3&&key.key_code==32);context.finish_key(&message,false);
        context.post_space(true).unwrap();let (_,message)=context.next_key().unwrap().unwrap();context.finish_key(&message,true);
        assert!(context.next_key().unwrap().is_none()); // preventDefault suppresses translated WM_CHAR.
    }
}
