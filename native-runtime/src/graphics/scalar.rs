// Scalar-only GL calls generated from pinned import types and NDK r28c GLES3 prototypes.
// No guest pointer is ever forwarded by this module.
use super::{Host,host_error,value_i32,procedure};
use wasmtime::{Caller,Val};
pub(super) fn dispatch(caller:&mut Caller<'_,Host>,name:&str,args:&[Val],results:&mut[Val])->wasmtime::Result<bool>{
    match name {
        "env._glActiveTexture"=>{
            let function:unsafe extern "system" fn(u32)->()=unsafe{std::mem::transmute(procedure(caller,"glActiveTexture")?)};
            unsafe{function(value_i32(args,0)? as u32)};
        }
        "env._glAttachShader"=>{
            let function:unsafe extern "system" fn(u32,u32)->()=unsafe{std::mem::transmute(procedure(caller,"glAttachShader")?)};
            unsafe{function(value_i32(args,0)? as u32,value_i32(args,1)? as u32)};
        }
        "env._glBeginQuery"=>{
            let function:unsafe extern "system" fn(u32,u32)->()=unsafe{std::mem::transmute(procedure(caller,"glBeginQuery")?)};
            unsafe{function(value_i32(args,0)? as u32,value_i32(args,1)? as u32)};
        }
        "env._glBeginTransformFeedback"=>{
            let function:unsafe extern "system" fn(u32)->()=unsafe{std::mem::transmute(procedure(caller,"glBeginTransformFeedback")?)};
            unsafe{function(value_i32(args,0)? as u32)};
        }
        "env._glBindBuffer"=>{
            let function:unsafe extern "system" fn(u32,u32)->()=unsafe{std::mem::transmute(procedure(caller,"glBindBuffer")?)};
            unsafe{function(value_i32(args,0)? as u32,value_i32(args,1)? as u32)};
        }
        "env._glBindBufferBase"=>{
            let function:unsafe extern "system" fn(u32,u32,u32)->()=unsafe{std::mem::transmute(procedure(caller,"glBindBufferBase")?)};
            unsafe{function(value_i32(args,0)? as u32,value_i32(args,1)? as u32,value_i32(args,2)? as u32)};
        }
        "env._glBindBufferRange"=>{
            let function:unsafe extern "system" fn(u32,u32,u32,isize,isize)->()=unsafe{std::mem::transmute(procedure(caller,"glBindBufferRange")?)};
            unsafe{function(value_i32(args,0)? as u32,value_i32(args,1)? as u32,value_i32(args,2)? as u32,value_i32(args,3)? as isize,value_i32(args,4)? as isize)};
        }
        "env._glBindFramebuffer"=>{
            let function:unsafe extern "system" fn(u32,u32)->()=unsafe{std::mem::transmute(procedure(caller,"glBindFramebuffer")?)};
            unsafe{function(value_i32(args,0)? as u32,value_i32(args,1)? as u32)};
        }
        "env._glBindRenderbuffer"=>{
            let function:unsafe extern "system" fn(u32,u32)->()=unsafe{std::mem::transmute(procedure(caller,"glBindRenderbuffer")?)};
            unsafe{function(value_i32(args,0)? as u32,value_i32(args,1)? as u32)};
        }
        "env._glBindSampler"=>{
            let function:unsafe extern "system" fn(u32,u32)->()=unsafe{std::mem::transmute(procedure(caller,"glBindSampler")?)};
            unsafe{function(value_i32(args,0)? as u32,value_i32(args,1)? as u32)};
        }
        "env._glBindTexture"=>{
            let function:unsafe extern "system" fn(u32,u32)->()=unsafe{std::mem::transmute(procedure(caller,"glBindTexture")?)};
            unsafe{function(value_i32(args,0)? as u32,value_i32(args,1)? as u32)};
        }
        "env._glBindTransformFeedback"=>{
            let function:unsafe extern "system" fn(u32,u32)->()=unsafe{std::mem::transmute(procedure(caller,"glBindTransformFeedback")?)};
            unsafe{function(value_i32(args,0)? as u32,value_i32(args,1)? as u32)};
        }
        "env._glBindVertexArray"=>{
            let function:unsafe extern "system" fn(u32)->()=unsafe{std::mem::transmute(procedure(caller,"glBindVertexArray")?)};
            unsafe{function(value_i32(args,0)? as u32)};
        }
        "env._glBlendEquation"=>{
            let function:unsafe extern "system" fn(u32)->()=unsafe{std::mem::transmute(procedure(caller,"glBlendEquation")?)};
            unsafe{function(value_i32(args,0)? as u32)};
        }
        "env._glBlendEquationSeparate"=>{
            let function:unsafe extern "system" fn(u32,u32)->()=unsafe{std::mem::transmute(procedure(caller,"glBlendEquationSeparate")?)};
            unsafe{function(value_i32(args,0)? as u32,value_i32(args,1)? as u32)};
        }
        "env._glBlendFuncSeparate"=>{
            let function:unsafe extern "system" fn(u32,u32,u32,u32)->()=unsafe{std::mem::transmute(procedure(caller,"glBlendFuncSeparate")?)};
            unsafe{function(value_i32(args,0)? as u32,value_i32(args,1)? as u32,value_i32(args,2)? as u32,value_i32(args,3)? as u32)};
        }
        "env._glBlitFramebuffer"=>{
            let function:unsafe extern "system" fn(i32,i32,i32,i32,i32,i32,i32,i32,u32,u32)->()=unsafe{std::mem::transmute(procedure(caller,"glBlitFramebuffer")?)};
            unsafe{function(value_i32(args,0)? as i32,value_i32(args,1)? as i32,value_i32(args,2)? as i32,value_i32(args,3)? as i32,value_i32(args,4)? as i32,value_i32(args,5)? as i32,value_i32(args,6)? as i32,value_i32(args,7)? as i32,value_i32(args,8)? as u32,value_i32(args,9)? as u32)};
        }
        "env._glCheckFramebufferStatus"=>{
            let function:unsafe extern "system" fn(u32)->u32=unsafe{std::mem::transmute(procedure(caller,"glCheckFramebufferStatus")?)};
            results[0]=Val::I32(unsafe{function(value_i32(args,0)? as u32)} as i32);
        }
        "env._glClear"=>{
            let function:unsafe extern "system" fn(u32)->()=unsafe{std::mem::transmute(procedure(caller,"glClear")?)};
            unsafe{function(value_i32(args,0)? as u32)};
        }
        "env._glClearBufferfi"=>{
            let function:unsafe extern "system" fn(u32,i32,f32,i32)->()=unsafe{std::mem::transmute(procedure(caller,"glClearBufferfi")?)};
            unsafe{function(value_i32(args,0)? as u32,value_i32(args,1)? as i32,args[2].f64().ok_or_else(||host_error("Expected GL float"))? as f32,value_i32(args,3)? as i32)};
        }
        "env._glClearColor"=>{
            let function:unsafe extern "system" fn(f32,f32,f32,f32)->()=unsafe{std::mem::transmute(procedure(caller,"glClearColor")?)};
            unsafe{function(args[0].f64().ok_or_else(||host_error("Expected GL float"))? as f32,args[1].f64().ok_or_else(||host_error("Expected GL float"))? as f32,args[2].f64().ok_or_else(||host_error("Expected GL float"))? as f32,args[3].f64().ok_or_else(||host_error("Expected GL float"))? as f32)};
        }
        "env._glClearDepthf"=>{
            let function:unsafe extern "system" fn(f32)->()=unsafe{std::mem::transmute(procedure(caller,"glClearDepthf")?)};
            unsafe{function(args[0].f64().ok_or_else(||host_error("Expected GL float"))? as f32)};
        }
        "env._glClearStencil"=>{
            let function:unsafe extern "system" fn(i32)->()=unsafe{std::mem::transmute(procedure(caller,"glClearStencil")?)};
            unsafe{function(value_i32(args,0)? as i32)};
        }
        "env._glColorMask"=>{
            let function:unsafe extern "system" fn(u8,u8,u8,u8)->()=unsafe{std::mem::transmute(procedure(caller,"glColorMask")?)};
            unsafe{function(value_i32(args,0)? as u8,value_i32(args,1)? as u8,value_i32(args,2)? as u8,value_i32(args,3)? as u8)};
        }
        "env._glCompileShader"=>{
            let function:unsafe extern "system" fn(u32)->()=unsafe{std::mem::transmute(procedure(caller,"glCompileShader")?)};
            unsafe{function(value_i32(args,0)? as u32)};
        }
        "env._glCopyBufferSubData"=>{
            let function:unsafe extern "system" fn(u32,u32,isize,isize,isize)->()=unsafe{std::mem::transmute(procedure(caller,"glCopyBufferSubData")?)};
            unsafe{function(value_i32(args,0)? as u32,value_i32(args,1)? as u32,value_i32(args,2)? as isize,value_i32(args,3)? as isize,value_i32(args,4)? as isize)};
        }
        "env._glCopyTexImage2D"=>{
            let function:unsafe extern "system" fn(u32,i32,u32,i32,i32,i32,i32,i32)->()=unsafe{std::mem::transmute(procedure(caller,"glCopyTexImage2D")?)};
            unsafe{function(value_i32(args,0)? as u32,value_i32(args,1)? as i32,value_i32(args,2)? as u32,value_i32(args,3)? as i32,value_i32(args,4)? as i32,value_i32(args,5)? as i32,value_i32(args,6)? as i32,value_i32(args,7)? as i32)};
        }
        "env._glCopyTexSubImage2D"=>{
            let function:unsafe extern "system" fn(u32,i32,i32,i32,i32,i32,i32,i32)->()=unsafe{std::mem::transmute(procedure(caller,"glCopyTexSubImage2D")?)};
            unsafe{function(value_i32(args,0)? as u32,value_i32(args,1)? as i32,value_i32(args,2)? as i32,value_i32(args,3)? as i32,value_i32(args,4)? as i32,value_i32(args,5)? as i32,value_i32(args,6)? as i32,value_i32(args,7)? as i32)};
        }
        "env._glCreateProgram"=>{
            let function:unsafe extern "system" fn()->u32=unsafe{std::mem::transmute(procedure(caller,"glCreateProgram")?)};
            results[0]=Val::I32(unsafe{function()} as i32);
        }
        "env._glCreateShader"=>{
            let function:unsafe extern "system" fn(u32)->u32=unsafe{std::mem::transmute(procedure(caller,"glCreateShader")?)};
            results[0]=Val::I32(unsafe{function(value_i32(args,0)? as u32)} as i32);
        }
        "env._glCullFace"=>{
            let function:unsafe extern "system" fn(u32)->()=unsafe{std::mem::transmute(procedure(caller,"glCullFace")?)};
            unsafe{function(value_i32(args,0)? as u32)};
        }
        "env._glDeleteProgram"=>{
            let function:unsafe extern "system" fn(u32)->()=unsafe{std::mem::transmute(procedure(caller,"glDeleteProgram")?)};
            unsafe{function(value_i32(args,0)? as u32)};
        }
        "env._glDeleteShader"=>{
            let function:unsafe extern "system" fn(u32)->()=unsafe{std::mem::transmute(procedure(caller,"glDeleteShader")?)};
            unsafe{function(value_i32(args,0)? as u32)};
        }
        "env._glDepthFunc"=>{
            let function:unsafe extern "system" fn(u32)->()=unsafe{std::mem::transmute(procedure(caller,"glDepthFunc")?)};
            unsafe{function(value_i32(args,0)? as u32)};
        }
        "env._glDepthMask"=>{
            let function:unsafe extern "system" fn(u8)->()=unsafe{std::mem::transmute(procedure(caller,"glDepthMask")?)};
            unsafe{function(value_i32(args,0)? as u8)};
        }
        "env._glDetachShader"=>{
            let function:unsafe extern "system" fn(u32,u32)->()=unsafe{std::mem::transmute(procedure(caller,"glDetachShader")?)};
            unsafe{function(value_i32(args,0)? as u32,value_i32(args,1)? as u32)};
        }
        "env._glDisable"=>{
            let function:unsafe extern "system" fn(u32)->()=unsafe{std::mem::transmute(procedure(caller,"glDisable")?)};
            unsafe{function(value_i32(args,0)? as u32)};
        }
        "env._glDisableVertexAttribArray"=>{
            let function:unsafe extern "system" fn(u32)->()=unsafe{std::mem::transmute(procedure(caller,"glDisableVertexAttribArray")?)};
            unsafe{function(value_i32(args,0)? as u32)};
        }
        "env._glDrawArrays"=>{
            let function:unsafe extern "system" fn(u32,i32,i32)->()=unsafe{std::mem::transmute(procedure(caller,"glDrawArrays")?)};
            unsafe{function(value_i32(args,0)? as u32,value_i32(args,1)? as i32,value_i32(args,2)? as i32)};
            caller.data_mut().graphics.as_mut().unwrap().draw_calls+=1;
        }
        "env._glDrawArraysInstanced"=>{
            let function:unsafe extern "system" fn(u32,i32,i32,i32)->()=unsafe{std::mem::transmute(procedure(caller,"glDrawArraysInstanced")?)};
            unsafe{function(value_i32(args,0)? as u32,value_i32(args,1)? as i32,value_i32(args,2)? as i32,value_i32(args,3)? as i32)};
            caller.data_mut().graphics.as_mut().unwrap().draw_calls+=1;
        }
        "env._glEnable"=>{
            let function:unsafe extern "system" fn(u32)->()=unsafe{std::mem::transmute(procedure(caller,"glEnable")?)};
            unsafe{function(value_i32(args,0)? as u32)};
        }
        "env._glEnableVertexAttribArray"=>{
            let function:unsafe extern "system" fn(u32)->()=unsafe{std::mem::transmute(procedure(caller,"glEnableVertexAttribArray")?)};
            unsafe{function(value_i32(args,0)? as u32)};
        }
        "env._glEndQuery"=>{
            let function:unsafe extern "system" fn(u32)->()=unsafe{std::mem::transmute(procedure(caller,"glEndQuery")?)};
            unsafe{function(value_i32(args,0)? as u32)};
        }
        "env._glEndTransformFeedback"=>{
            let function:unsafe extern "system" fn()->()=unsafe{std::mem::transmute(procedure(caller,"glEndTransformFeedback")?)};
            unsafe{function()};
        }
        "env._glFinish"=>{
            let function:unsafe extern "system" fn()->()=unsafe{std::mem::transmute(procedure(caller,"glFinish")?)};
            unsafe{function()};
        }
        "env._glFlush"=>{
            let function:unsafe extern "system" fn()->()=unsafe{std::mem::transmute(procedure(caller,"glFlush")?)};
            unsafe{function()};
        }
        "env._glFlushMappedBufferRange"=>{
            let function:unsafe extern "system" fn(u32,isize,isize)->()=unsafe{std::mem::transmute(procedure(caller,"glFlushMappedBufferRange")?)};
            unsafe{function(value_i32(args,0)? as u32,value_i32(args,1)? as isize,value_i32(args,2)? as isize)};
        }
        "env._glFramebufferRenderbuffer"=>{
            let function:unsafe extern "system" fn(u32,u32,u32,u32)->()=unsafe{std::mem::transmute(procedure(caller,"glFramebufferRenderbuffer")?)};
            unsafe{function(value_i32(args,0)? as u32,value_i32(args,1)? as u32,value_i32(args,2)? as u32,value_i32(args,3)? as u32)};
        }
        "env._glFramebufferTexture2D"=>{
            let function:unsafe extern "system" fn(u32,u32,u32,u32,i32)->()=unsafe{std::mem::transmute(procedure(caller,"glFramebufferTexture2D")?)};
            unsafe{function(value_i32(args,0)? as u32,value_i32(args,1)? as u32,value_i32(args,2)? as u32,value_i32(args,3)? as u32,value_i32(args,4)? as i32)};
        }
        "env._glFramebufferTextureLayer"=>{
            let function:unsafe extern "system" fn(u32,u32,u32,i32,i32)->()=unsafe{std::mem::transmute(procedure(caller,"glFramebufferTextureLayer")?)};
            unsafe{function(value_i32(args,0)? as u32,value_i32(args,1)? as u32,value_i32(args,2)? as u32,value_i32(args,3)? as i32,value_i32(args,4)? as i32)};
        }
        "env._glFrontFace"=>{
            let function:unsafe extern "system" fn(u32)->()=unsafe{std::mem::transmute(procedure(caller,"glFrontFace")?)};
            unsafe{function(value_i32(args,0)? as u32)};
        }
        "env._glGenerateMipmap"=>{
            let function:unsafe extern "system" fn(u32)->()=unsafe{std::mem::transmute(procedure(caller,"glGenerateMipmap")?)};
            unsafe{function(value_i32(args,0)? as u32)};
        }
        "env._glGetError"=>{
            let function:unsafe extern "system" fn()->u32=unsafe{std::mem::transmute(procedure(caller,"glGetError")?)};
            results[0]=Val::I32(unsafe{function()} as i32);
        }
        "env._glIsEnabled"=>{
            let function:unsafe extern "system" fn(u32)->u8=unsafe{std::mem::transmute(procedure(caller,"glIsEnabled")?)};
            results[0]=Val::I32(unsafe{function(value_i32(args,0)? as u32)} as i32);
        }
        "env._glIsVertexArray"=>{
            let function:unsafe extern "system" fn(u32)->u8=unsafe{std::mem::transmute(procedure(caller,"glIsVertexArray")?)};
            results[0]=Val::I32(unsafe{function(value_i32(args,0)? as u32)} as i32);
        }
        "env._glLinkProgram"=>{
            let function:unsafe extern "system" fn(u32)->()=unsafe{std::mem::transmute(procedure(caller,"glLinkProgram")?)};
            unsafe{function(value_i32(args,0)? as u32)};
        }
        "env._glPixelStorei"=>{
            let function:unsafe extern "system" fn(u32,i32)->()=unsafe{std::mem::transmute(procedure(caller,"glPixelStorei")?)};
            unsafe{function(value_i32(args,0)? as u32,value_i32(args,1)? as i32)};
        }
        "env._glPolygonOffset"=>{
            let function:unsafe extern "system" fn(f32,f32)->()=unsafe{std::mem::transmute(procedure(caller,"glPolygonOffset")?)};
            unsafe{function(args[0].f64().ok_or_else(||host_error("Expected GL float"))? as f32,args[1].f64().ok_or_else(||host_error("Expected GL float"))? as f32)};
        }
        "env._glProgramParameteri"=>{
            let function:unsafe extern "system" fn(u32,u32,i32)->()=unsafe{std::mem::transmute(procedure(caller,"glProgramParameteri")?)};
            unsafe{function(value_i32(args,0)? as u32,value_i32(args,1)? as u32,value_i32(args,2)? as i32)};
        }
        "env._glReadBuffer"=>{
            let function:unsafe extern "system" fn(u32)->()=unsafe{std::mem::transmute(procedure(caller,"glReadBuffer")?)};
            unsafe{function(value_i32(args,0)? as u32)};
        }
        "env._glRenderbufferStorage"=>{
            let function:unsafe extern "system" fn(u32,u32,i32,i32)->()=unsafe{std::mem::transmute(procedure(caller,"glRenderbufferStorage")?)};
            unsafe{function(value_i32(args,0)? as u32,value_i32(args,1)? as u32,value_i32(args,2)? as i32,value_i32(args,3)? as i32)};
        }
        "env._glRenderbufferStorageMultisample"=>{
            let function:unsafe extern "system" fn(u32,i32,u32,i32,i32)->()=unsafe{std::mem::transmute(procedure(caller,"glRenderbufferStorageMultisample")?)};
            unsafe{function(value_i32(args,0)? as u32,value_i32(args,1)? as i32,value_i32(args,2)? as u32,value_i32(args,3)? as i32,value_i32(args,4)? as i32)};
        }
        "env._glSamplerParameteri"=>{
            let function:unsafe extern "system" fn(u32,u32,i32)->()=unsafe{std::mem::transmute(procedure(caller,"glSamplerParameteri")?)};
            unsafe{function(value_i32(args,0)? as u32,value_i32(args,1)? as u32,value_i32(args,2)? as i32)};
        }
        "env._glScissor"=>{
            let function:unsafe extern "system" fn(i32,i32,i32,i32)->()=unsafe{std::mem::transmute(procedure(caller,"glScissor")?)};
            unsafe{function(value_i32(args,0)? as i32,value_i32(args,1)? as i32,value_i32(args,2)? as i32,value_i32(args,3)? as i32)};
        }
        "env._glStencilFuncSeparate"=>{
            let function:unsafe extern "system" fn(u32,u32,i32,u32)->()=unsafe{std::mem::transmute(procedure(caller,"glStencilFuncSeparate")?)};
            unsafe{function(value_i32(args,0)? as u32,value_i32(args,1)? as u32,value_i32(args,2)? as i32,value_i32(args,3)? as u32)};
        }
        "env._glStencilMask"=>{
            let function:unsafe extern "system" fn(u32)->()=unsafe{std::mem::transmute(procedure(caller,"glStencilMask")?)};
            unsafe{function(value_i32(args,0)? as u32)};
        }
        "env._glStencilOpSeparate"=>{
            let function:unsafe extern "system" fn(u32,u32,u32,u32)->()=unsafe{std::mem::transmute(procedure(caller,"glStencilOpSeparate")?)};
            unsafe{function(value_i32(args,0)? as u32,value_i32(args,1)? as u32,value_i32(args,2)? as u32,value_i32(args,3)? as u32)};
        }
        "env._glTexParameterf"=>{
            let function:unsafe extern "system" fn(u32,u32,f32)->()=unsafe{std::mem::transmute(procedure(caller,"glTexParameterf")?)};
            unsafe{function(value_i32(args,0)? as u32,value_i32(args,1)? as u32,args[2].f64().ok_or_else(||host_error("Expected GL float"))? as f32)};
        }
        "env._glTexParameteri"=>{
            let function:unsafe extern "system" fn(u32,u32,i32)->()=unsafe{std::mem::transmute(procedure(caller,"glTexParameteri")?)};
            unsafe{function(value_i32(args,0)? as u32,value_i32(args,1)? as u32,value_i32(args,2)? as i32)};
        }
        "env._glTexStorage2D"=>{
            let function:unsafe extern "system" fn(u32,i32,u32,i32,i32)->()=unsafe{std::mem::transmute(procedure(caller,"glTexStorage2D")?)};
            unsafe{function(value_i32(args,0)? as u32,value_i32(args,1)? as i32,value_i32(args,2)? as u32,value_i32(args,3)? as i32,value_i32(args,4)? as i32)};
        }
        "env._glTexStorage3D"=>{
            let function:unsafe extern "system" fn(u32,i32,u32,i32,i32,i32)->()=unsafe{std::mem::transmute(procedure(caller,"glTexStorage3D")?)};
            unsafe{function(value_i32(args,0)? as u32,value_i32(args,1)? as i32,value_i32(args,2)? as u32,value_i32(args,3)? as i32,value_i32(args,4)? as i32,value_i32(args,5)? as i32)};
        }
        "env._glUniform1i"=>{
            let function:unsafe extern "system" fn(i32,i32)->()=unsafe{std::mem::transmute(procedure(caller,"glUniform1i")?)};
            unsafe{function(value_i32(args,0)? as i32,value_i32(args,1)? as i32)};
        }
        "env._glUniformBlockBinding"=>{
            let function:unsafe extern "system" fn(u32,u32,u32)->()=unsafe{std::mem::transmute(procedure(caller,"glUniformBlockBinding")?)};
            unsafe{function(value_i32(args,0)? as u32,value_i32(args,1)? as u32,value_i32(args,2)? as u32)};
        }
        "env._glUnmapBuffer"=>{
            let function:unsafe extern "system" fn(u32)->u8=unsafe{std::mem::transmute(procedure(caller,"glUnmapBuffer")?)};
            results[0]=Val::I32(unsafe{function(value_i32(args,0)? as u32)} as i32);
        }
        "env._glUseProgram"=>{
            let function:unsafe extern "system" fn(u32)->()=unsafe{std::mem::transmute(procedure(caller,"glUseProgram")?)};
            unsafe{function(value_i32(args,0)? as u32)};
        }
        "env._glValidateProgram"=>{
            let function:unsafe extern "system" fn(u32)->()=unsafe{std::mem::transmute(procedure(caller,"glValidateProgram")?)};
            unsafe{function(value_i32(args,0)? as u32)};
        }
        "env._glVertexAttrib4f"=>{
            let function:unsafe extern "system" fn(u32,f32,f32,f32,f32)->()=unsafe{std::mem::transmute(procedure(caller,"glVertexAttrib4f")?)};
            unsafe{function(value_i32(args,0)? as u32,args[1].f64().ok_or_else(||host_error("Expected GL float"))? as f32,args[2].f64().ok_or_else(||host_error("Expected GL float"))? as f32,args[3].f64().ok_or_else(||host_error("Expected GL float"))? as f32,args[4].f64().ok_or_else(||host_error("Expected GL float"))? as f32)};
        }
        "env._glViewport"=>{
            let function:unsafe extern "system" fn(i32,i32,i32,i32)->()=unsafe{std::mem::transmute(procedure(caller,"glViewport")?)};
            unsafe{function(value_i32(args,0)? as i32,value_i32(args,1)? as i32,value_i32(args,2)? as i32,value_i32(args,3)? as i32)};
        }
        _=>return Ok(false),
    }
    Ok(true)
}
