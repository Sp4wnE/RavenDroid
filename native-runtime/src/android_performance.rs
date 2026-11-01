//! Optional API33+ scheduling hints, resolved dynamically for minSdk29.
//! All handles belong to the VM owner thread; no persistent device settings.
use std::{ffi::c_void,marker::PhantomData,rc::Rc};
type Session=*mut c_void;
type Report=unsafe extern "C" fn(Session,i64)->i32;
type Close=unsafe extern "C" fn(Session);
pub(super) struct Hint{
    library:*mut c_void,session:Session,report:Report,update:Report,close:Close,
    target:i64,reports:u64,error:i32,_owner:PhantomData<Rc<()>>,
}
impl Hint{
    pub fn new()->Option<Self>{unsafe{
        let library=libc::dlopen(c"libandroid.so".as_ptr(),libc::RTLD_NOW|libc::RTLD_LOCAL);
        if library.is_null(){return None;}
        let get=libc::dlsym(library,c"APerformanceHint_getManager".as_ptr());
        let create=libc::dlsym(library,c"APerformanceHint_createSession".as_ptr());
        let report=libc::dlsym(library,c"APerformanceHint_reportActualWorkDuration".as_ptr());
        let update=libc::dlsym(library,c"APerformanceHint_updateTargetWorkDuration".as_ptr());
        let close=libc::dlsym(library,c"APerformanceHint_closeSession".as_ptr());
        if [get,create,report,update,close].iter().any(|p|p.is_null()){libc::dlclose(library);return None;}
        let get:unsafe extern "C" fn()->*mut c_void=std::mem::transmute(get);
        let create:unsafe extern "C" fn(*mut c_void,*const i32,usize,i64)->Session=std::mem::transmute(create);
        let manager=get();if manager.is_null(){libc::dlclose(library);return None;}
        let tid=libc::gettid();let target=16_666_667;let session=create(manager,&tid,1,target);
        if session.is_null(){libc::dlclose(library);return None;}
        Some(Self{library,session,report:std::mem::transmute(report),update:std::mem::transmute(update),close:std::mem::transmute(close),target,reports:0,error:0,_owner:PhantomData})
    }}
    pub fn report(&mut self,work_ns:u128,target_ns:u128){
        if self.error!=0{return;}
        let target=target_ns.clamp(1,i64::MAX as u128) as i64;
        unsafe{
            if target!=self.target{self.error=(self.update)(self.session,target);if self.error!=0{return;}self.target=target;}
            self.error=(self.report)(self.session,work_ns.clamp(1,i64::MAX as u128) as i64);
        }
        if self.error==0{self.reports+=1;}
    }
    pub fn status(&self)->serde_json::Value{serde_json::json!({"session_created":true,"target_ns":self.target,"reports":self.reports,"error":self.error})}
}
impl Drop for Hint{fn drop(&mut self){unsafe{(self.close)(self.session);libc::dlclose(self.library);}}}
