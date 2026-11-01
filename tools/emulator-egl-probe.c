// Standalone Android preflight: same ES3 robust pbuffer contract as the host.
// Run only on an explicitly selected test emulator, before compiling game AOT.
#include <EGL/egl.h>
#include <EGL/eglext.h>
#include <GLES3/gl3.h>
#include <stdio.h>
#include <string.h>

static int extension(const char *list, const char *name) {
    if (!list) return 0;
    size_t n = strlen(name);
    for (const char *p = list; (p = strstr(p, name)); p += n)
        if ((p == list || p[-1] == ' ') && (p[n] == ' ' || !p[n])) return 1;
    return 0;
}
int main(void) {
    EGLDisplay display = eglGetDisplay(EGL_DEFAULT_DISPLAY);
    EGLint major = 0, minor = 0, count = 0;
    if (!eglInitialize(display, &major, &minor)) {
        printf("FAIL initialize error=0x%x\n", eglGetError()); return 1;
    }
    const char *ext = eglQueryString(display, EGL_EXTENSIONS);
    int core = major > 1 || (major == 1 && minor >= 5);
    printf("EGL=%d.%d vendor=%s EXT_robustness=%d KHR_create_context=%d\n",
        major, minor, eglQueryString(display, EGL_VENDOR),
        extension(ext, "EGL_EXT_create_context_robustness"), extension(ext, "EGL_KHR_create_context"));
    if (!core && !extension(ext, "EGL_EXT_create_context_robustness") && !extension(ext, "EGL_KHR_create_context")) {
        puts("FAIL robustness unavailable"); eglTerminate(display); return 2;
    }
    EGLint config_attrs[] = {EGL_SURFACE_TYPE,EGL_PBUFFER_BIT,EGL_RENDERABLE_TYPE,EGL_OPENGL_ES3_BIT,
        EGL_RED_SIZE,8,EGL_GREEN_SIZE,8,EGL_BLUE_SIZE,8,EGL_ALPHA_SIZE,8,EGL_DEPTH_SIZE,24,EGL_STENCIL_SIZE,8,EGL_NONE};
    EGLConfig config;
    if (!eglChooseConfig(display,config_attrs,&config,1,&count) || count != 1 || !eglBindAPI(EGL_OPENGL_ES_API)) {
        printf("FAIL config error=0x%x\n",eglGetError()); eglTerminate(display); return 3;
    }
    EGLint core_attrs[] = {EGL_CONTEXT_CLIENT_VERSION,3,0x31B2,1,0x31BD,0x31BF,EGL_NONE};
    EGLint ext_attrs[] = {EGL_CONTEXT_CLIENT_VERSION,3,0x30BF,1,0x3138,0x31BF,EGL_NONE};
    EGLint khr_attrs[] = {EGL_CONTEXT_CLIENT_VERSION,3,EGL_CONTEXT_FLAGS_KHR,EGL_CONTEXT_OPENGL_ROBUST_ACCESS_BIT_KHR,EGL_CONTEXT_OPENGL_RESET_NOTIFICATION_STRATEGY_KHR,EGL_LOSE_CONTEXT_ON_RESET_KHR,EGL_NONE};
    EGLContext context=eglCreateContext(display,config,EGL_NO_CONTEXT,core?core_attrs:extension(ext,"EGL_EXT_create_context_robustness")?ext_attrs:khr_attrs);
    if (context==EGL_NO_CONTEXT) {
        printf("FAIL create context error=0x%x\n",eglGetError()); eglTerminate(display); return 4;
    }
    EGLint surface_attrs[]={EGL_WIDTH,64,EGL_HEIGHT,64,EGL_NONE};
    EGLSurface surface=eglCreatePbufferSurface(display,config,surface_attrs);
    if (context==EGL_NO_CONTEXT || surface==EGL_NO_SURFACE || !eglMakeCurrent(display,surface,surface,context)) {
        printf("FAIL context/surface error=0x%x\n",eglGetError()); return 4;
    }
    GLint robust=0; glGetIntegerv(0x90F3,&robust); GLenum error=glGetError();
    printf("GL=%s renderer=%s robust=%d error=0x%x\n",glGetString(GL_VERSION),glGetString(GL_RENDERER),robust,error);
    glClearColor(0.25f,0.5f,0.75f,1.0f); glClear(GL_COLOR_BUFFER_BIT);
    unsigned char pixel[4]={0}; glReadPixels(0,0,1,1,GL_RGBA,GL_UNSIGNED_BYTE,pixel);
    GLenum read_error=glGetError();
    int ok=robust && !error && !read_error && pixel[0]>=63 && pixel[0]<=65 && pixel[1]>=127 && pixel[1]<=129 && pixel[2]>=190 && pixel[2]<=192;
    printf("%s readback=%u,%u,%u,%u error=0x%x\n",ok?"PASS":"FAIL",pixel[0],pixel[1],pixel[2],pixel[3],read_error);
    eglMakeCurrent(display,EGL_NO_SURFACE,EGL_NO_SURFACE,EGL_NO_CONTEXT);
    eglDestroySurface(display,surface); eglDestroyContext(display,context); eglTerminate(display);
    return ok?0:5;
}
