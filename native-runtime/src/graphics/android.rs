//! Owner-thread Android EGL pbuffer. No Activity/window or display swap is needed.
use anyhow::{Result,ensure};
use std::{ffi::{CStr,CString,c_void,c_char},ptr::null_mut,collections::BTreeMap,marker::PhantomData,rc::Rc};
type Handle=*mut c_void;
#[link(name="EGL")]
unsafe extern "C"{
    fn eglGetDisplay(native:Handle)->Handle;
    fn eglInitialize(display:Handle,major:*mut i32,minor:*mut i32)->u32;
    fn eglQueryString(display:Handle,name:i32)->*const c_char;
    fn eglChooseConfig(display:Handle,attributes:*const i32,configs:*mut Handle,size:i32,count:*mut i32)->u32;
    fn eglBindAPI(api:u32)->u32;
    fn eglCreateContext(display:Handle,config:Handle,shared:Handle,attributes:*const i32)->Handle;
    fn eglCreatePbufferSurface(display:Handle,config:Handle,attributes:*const i32)->Handle;
    fn eglCreateWindowSurface(display:Handle,config:Handle,window:Handle,attributes:*const i32)->Handle;
    fn eglGetConfigAttrib(display:Handle,config:Handle,name:i32,value:*mut i32)->u32;
    fn eglQuerySurface(display:Handle,surface:Handle,name:i32,value:*mut i32)->u32;
    fn eglSwapBuffers(display:Handle,surface:Handle)->u32;
    fn eglSwapInterval(display:Handle,interval:i32)->u32;
    fn eglMakeCurrent(display:Handle,draw:Handle,read:Handle,context:Handle)->u32;
    fn eglGetCurrentContext()->Handle;
    fn eglGetCurrentDisplay()->Handle;
    fn eglGetCurrentSurface(which:i32)->Handle;
    fn eglGetProcAddress(name:*const c_char)->Option<unsafe extern "C" fn()>;
    fn eglDestroyContext(display:Handle,context:Handle)->u32;
    fn eglDestroySurface(display:Handle,surface:Handle)->u32;
    fn eglGetError()->u32;
}
#[link(name="android")]
unsafe extern "C"{
    fn ANativeWindow_fromSurface(env:*mut jni::sys::JNIEnv,surface:jni::sys::jobject)->Handle;
    fn ANativeWindow_release(window:Handle);
    fn ANativeWindow_setBuffersGeometry(window:Handle,width:i32,height:i32,format:i32)->i32;
}
pub(crate) struct NativeWindow{raw:Handle,_owner:PhantomData<Rc<()>>}
impl NativeWindow{
    pub(crate) fn from_surface(env:&jni::JNIEnv,surface:&jni::objects::JObject)->Result<std::sync::Arc<Self>>{
        let raw=unsafe{ANativeWindow_fromSurface(env.get_native_interface(),surface.as_raw())};
        ensure!(!raw.is_null(),"Android Surface has no live native window");Ok(std::sync::Arc::new(Self{raw,_owner:PhantomData}))
    }
}
impl Drop for NativeWindow{fn drop(&mut self){unsafe{ANativeWindow_release(self.raw);}}}
#[link(name="GLESv2")]
unsafe extern "C"{
    fn glGetString(name:u32)->*const u8;
    fn glGetIntegerv(name:u32,value:*mut i32);
    fn glGetError()->u32;
    fn glViewport(x:i32,y:i32,width:i32,height:i32);
    fn glClear(mask:u32);
    fn glTexParameteri(target:u32,name:u32,value:i32);
    fn glPixelStorei(name:u32,value:i32);
    fn glFinish();
    fn glReadPixels(x:i32,y:i32,width:i32,height:i32,format:u32,kind:u32,pixels:*mut c_void);
}
const GL_VERSION:u32=0x1F02;const GL_VENDOR:u32=0x1F00;const GL_RENDERER:u32=0x1F01;const GL_EXTENSIONS:u32=0x1F03;
const GL_COLOR_BUFFER_BIT:u32=0x4000;const GL_DEPTH_BUFFER_BIT:u32=0x100;const GL_STENCIL_BUFFER_BIT:u32=0x400;
const GL_RGBA:u32=0x1908;const GL_UNSIGNED_BYTE:u32=0x1401;
macro_rules! gl{($context:expr,$name:expr,$signature:ty;$($arg:expr),* $(,)?)=>{{let function:$signature=unsafe{std::mem::transmute($context.procedure($name)?)};unsafe{function($($arg),*)}}};}
pub(super) struct Context{
    display:Handle,config:Handle,context:Handle,surface:Handle,width:i32,height:i32,
    previous_display:Handle,previous_context:Handle,previous_draw:Handle,previous_read:Handle,
    symbols:BTreeMap<String,usize>,_owner:PhantomData<Rc<()>>,
    pub(super) version:String,pub(super) renderer:String,pub(super) vendor:String,
    framebuffer:u32,color:u32,depth:u32,
    window:Option<std::sync::Arc<NativeWindow>>,
    swap_interval:i32,
}
impl Context{
    pub(super) fn swap_interval(&self)->i32{self.swap_interval}
    pub(super) fn new(width:i32,height:i32)->Result<Self>{
        Self::with_window(width,height,None)
    }
    pub(super) fn with_window(width:i32,height:i32,window:Option<std::sync::Arc<NativeWindow>>)->Result<Self>{
        ensure!((1..=8192).contains(&width)&&(1..=8192).contains(&height),"Invalid EGL surface size");
        let mut this=Self{display:null_mut(),config:null_mut(),context:null_mut(),surface:null_mut(),width,height,
            previous_display:unsafe{eglGetCurrentDisplay()},previous_context:unsafe{eglGetCurrentContext()},previous_draw:unsafe{eglGetCurrentSurface(0x3059)},previous_read:unsafe{eglGetCurrentSurface(0x305A)},
            symbols:BTreeMap::new(),_owner:PhantomData,version:String::new(),renderer:String::new(),vendor:String::new(),framebuffer:0,color:0,depth:0,window,swap_interval:if cfg!(feature="android-async-present"){0}else{1}};
        unsafe{
            this.display=eglGetDisplay(null_mut());ensure!(!this.display.is_null(),"Get EGL display: {:#x}",eglGetError());
            let(mut major,mut minor)=(0,0);ensure!(eglInitialize(this.display,&mut major,&mut minor)!=0,"Initialize EGL: {:#x}",eglGetError());
            ensure!(major>1||(major==1&&minor>=4),"EGL 1.4 is required");
            let extensions=eglQueryString(this.display,0x3055);ensure!(!extensions.is_null(),"Query EGL extensions");
            let extensions=CStr::from_ptr(extensions).to_string_lossy();
            let has=|name:&str|extensions.split_ascii_whitespace().any(|value|value==name);
            ensure!((major>1||(major==1&&minor>=5))||has("EGL_KHR_create_context"),"ES3 robust context creation unavailable");
            let attributes=[0x3033,if this.window.is_some(){5}else{1},0x3040,0x40,0x3024,8,0x3023,8,0x3022,8,0x3021,8,0x3025,24,0x3026,8,0x3038];
            let mut count=0;ensure!(eglChooseConfig(this.display,attributes.as_ptr(),&mut this.config,1,&mut count)!=0&&count==1,"Choose EGL ES3 pbuffer config: {:#x}",eglGetError());
            if this.window.is_some(){let mut samples=0;ensure!(eglGetConfigAttrib(this.display,this.config,0x3031,&mut samples)!=0&&samples==0,"Live blit requires a single-sample EGL config");}
            ensure!(eglBindAPI(0x30A0)!=0,"Bind EGL OpenGL ES API");
            // EGL 1.5 core robustness attributes; EXT uses different tokens.
            let context_attributes=if major>1||(major==1&&minor>=5){[0x3098,3,0x31B2,1,0x31BD,0x31BF,0x3038]}
                else{ensure!(has("EGL_EXT_create_context_robustness"),"EGL robustness extension unavailable");[0x3098,3,0x30BF,1,0x3138,0x31BF,0x3038]};
            this.context=eglCreateContext(this.display,this.config,null_mut(),context_attributes.as_ptr());
            ensure!(!this.context.is_null(),"Create robust EGL ES3 context: {:#x}",eglGetError());
            this.surface=if let Some(window)=this.window.as_ref(){
                let mut format=0;ensure!(eglGetConfigAttrib(this.display,this.config,0x302E,&mut format)!=0,"Query EGL native window format");
                ensure!(ANativeWindow_setBuffersGeometry(window.raw,width,height,format)==0,"Set Android surface geometry");
                let attributes=[0x3038];let surface=eglCreateWindowSurface(this.display,this.config,window.raw,attributes.as_ptr());
                ensure!(!surface.is_null(),"Create EGL window surface: {:#x}",eglGetError());surface
            }else{this.create_pbuffer(width,height)?};this.make_current(true)?;
            if this.window.is_some(){
                let(mut minimum,mut maximum)=(0,0);
                ensure!(eglGetConfigAttrib(this.display,this.config,0x303B,&mut minimum)!=0&&eglGetConfigAttrib(this.display,this.config,0x303C,&mut maximum)!=0&&minimum>=0&&maximum>=minimum,"Query EGL swap interval limits");
                this.swap_interval=this.swap_interval.clamp(minimum,maximum);
                ensure!(eglSwapInterval(this.display,this.swap_interval)!=0,"Set EGL presentation interval");
            }
            this.version=this.string(GL_VERSION)?;this.renderer=this.string(GL_RENDERER)?;this.vendor=this.string(GL_VENDOR)?;
            ensure!(this.version.starts_with("OpenGL ES 3"),"EGL did not return ES3: {}",this.version);
            let mut robust=0;glGetIntegerv(0x90F3,&mut robust);
            ensure!(glGetError()==0&&robust!=0,"EGL driver did not enable robust buffer access");
            glViewport(0,0,width,height);
        }
        this.allocate_surface()?;
        gl!(&mut this,"glBindFramebuffer",unsafe extern "system" fn(u32,u32);0x8D40,this.framebuffer);
        unsafe{glClear(GL_COLOR_BUFFER_BIT|GL_DEPTH_BUFFER_BIT|GL_STENCIL_BUFFER_BIT);}Ok(this)
    }
    fn create_pbuffer(&self,width:i32,height:i32)->Result<Handle>{
        let attributes=[0x3057,width,0x3056,height,0x3038];
        let surface=unsafe{eglCreatePbufferSurface(self.display,self.config,attributes.as_ptr())};
        ensure!(!surface.is_null(),"Create EGL pbuffer: {:#x}",unsafe{eglGetError()});Ok(surface)
    }
    pub(super) fn procedure(&mut self,name:&str)->Result<usize>{
        self.make_current(true)?;if let Some(address)=self.symbols.get(name){return Ok(*address);}
        let name_c=CString::new(name)?;
        // Android EGL resolves both GLES core and extension entry points.
        let address=unsafe{eglGetProcAddress(name_c.as_ptr())}.map(|function|function as usize).unwrap_or(0);
        ensure!(address!=0,"Android GLES symbol unavailable: {name}");self.symbols.insert(name.into(),address);Ok(address)
    }
    pub(super) fn string(&mut self,name:u32)->Result<String>{
        self.make_current(true)?;let value=unsafe{glGetString(name)};ensure!(!value.is_null(),"Android GL string query failed");
        Ok(unsafe{CStr::from_ptr(value.cast())}.to_string_lossy().into_owned())
    }
    pub(super) fn dimensions(&self)->(i32,i32){(self.width,self.height)}
    pub(super) fn framebuffer(&self)->u32{self.framebuffer}
    pub(super) fn make_current(&self,active:bool)->Result<()>{
        let success=unsafe{if active{eglGetCurrentContext()==self.context||eglMakeCurrent(self.display,self.surface,self.surface,self.context)!=0}
            else{eglMakeCurrent(self.display,null_mut(),null_mut(),null_mut())!=0}};
        ensure!(success,"Change EGL context binding: {:#x}",unsafe{eglGetError()});Ok(())
    }
    pub(super) fn cursor_visible(&self,_visible:bool)->Result<()>{Ok(())} // Pbuffer has no pointer cursor.
    pub(super) fn resize(&mut self,width:i32,height:i32)->Result<()>{
        ensure!((1..=8192).contains(&width)&&(1..=8192).contains(&height),"Invalid EGL resize");
        // The launcher owns live resolution; guest resize requests cannot reset it.
        // Canvas/screen queries report the selected size back to Unity.
        if self.window.is_some(){return Ok(());}
        let replacement=self.create_pbuffer(width,height)?;
        if unsafe{eglMakeCurrent(self.display,replacement,replacement,self.context)}==0{
            unsafe{eglDestroySurface(self.display,replacement);}anyhow::bail!("Bind resized EGL pbuffer");
        }
        let previous=std::mem::replace(&mut self.surface,replacement);unsafe{eglDestroySurface(self.display,previous);}
        self.width=width;self.height=height;self.allocate_surface()
    }
    pub(super) fn extensions(&mut self)->Result<std::collections::BTreeSet<String>>{
        self.make_current(true)?;let mut count=0;unsafe{glGetIntegerv(0x821D,&mut count);}
        ensure!((0..=4096).contains(&count),"Invalid Android GL extension count");
        let function:unsafe extern "system" fn(u32,u32)->*const u8=unsafe{std::mem::transmute(self.procedure("glGetStringi")?)};
        let mut extensions=std::collections::BTreeSet::new();
        for index in 0..count{let value=unsafe{function(GL_EXTENSIONS,index as u32)};ensure!(!value.is_null(),"Query Android GL extension");extensions.insert(unsafe{CStr::from_ptr(value.cast())}.to_string_lossy().into_owned());}
        Ok(extensions)
    }
    pub(super) fn is_window(&self)->bool{self.window.is_some()}
    pub(super) fn present(&mut self)->Result<Vec<u32>>{
        ensure!(self.window.is_some(),"Presentation requires an Android window surface");self.make_current(true)?;
        let mut width=0;let mut height=0;
        ensure!(unsafe{eglQuerySurface(self.display,self.surface,0x3057,&mut width)}!=0&&unsafe{eglQuerySurface(self.display,self.surface,0x3056,&mut height)}!=0,"Query live EGL size");
        ensure!((1..=8192).contains(&width)&&(1..=8192).contains(&height),"Invalid EGL window dimensions");
        let mut errors=Vec::new();let(mut read,mut draw,mut read_buffer)=(0,0,0);
        unsafe{for _ in 0..16{let error=glGetError();if error==0{break;}errors.push(error);}glGetIntegerv(0x8CAA,&mut read);glGetIntegerv(0x8CA6,&mut draw);}
        let scissor=gl!(self,"glIsEnabled",unsafe extern "system" fn(u32)->u8;0xC11)!=0;
        gl!(self,"glBindFramebuffer",unsafe extern "system" fn(u32,u32);0x8CA8,self.framebuffer);
        unsafe{glGetIntegerv(0xC02,&mut read_buffer);}
        gl!(self,"glReadBuffer",unsafe extern "system" fn(u32);0x8CE0);
        gl!(self,"glBindFramebuffer",unsafe extern "system" fn(u32,u32);0x8CA9,0);
        let back=0x405;gl!(self,"glDrawBuffers",unsafe extern "system" fn(i32,*const u32);1,&back);
        if scissor{gl!(self,"glDisable",unsafe extern "system" fn(u32);0xC11);}
        gl!(self,"glBlitFramebuffer",unsafe extern "system" fn(i32,i32,i32,i32,i32,i32,i32,i32,u32,u32);0,0,self.width,self.height,0,0,width,height,0x4000,0x2600);
        let error=unsafe{glGetError()};
        gl!(self,"glReadBuffer",unsafe extern "system" fn(u32);read_buffer as u32);
        gl!(self,"glBindFramebuffer",unsafe extern "system" fn(u32,u32);0x8CA8,read as u32);
        gl!(self,"glBindFramebuffer",unsafe extern "system" fn(u32,u32);0x8CA9,draw as u32);
        if scissor{gl!(self,"glEnable",unsafe extern "system" fn(u32);0xC11);}
        ensure!(error==0,"Present framebuffer blit failed: {error:#x}");
        ensure!(unsafe{eglSwapBuffers(self.display,self.surface)}!=0,"Swap Android EGL surface: {:#x}",unsafe{eglGetError()});Ok(errors)
    }
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

}
impl Drop for Context{
    fn drop(&mut self){unsafe{
        if !self.display.is_null(){
            if eglGetCurrentContext()==self.context{glFinish();eglMakeCurrent(self.display,null_mut(),null_mut(),null_mut());}
            if !self.surface.is_null(){eglDestroySurface(self.display,self.surface);}
            if !self.context.is_null(){eglDestroyContext(self.display,self.context);}
            if !self.previous_display.is_null()&&!self.previous_context.is_null(){eglMakeCurrent(self.previous_display,self.previous_draw,self.previous_read,self.previous_context);}
            // EGL_DEFAULT_DISPLAY is shared with Android's UI renderer. Never
            // terminate it here: only the context and pbuffer belong to us.
        }
    }}
}

