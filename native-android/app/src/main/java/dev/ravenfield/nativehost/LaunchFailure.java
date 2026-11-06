package dev.ravenfield.nativehost;

final class LaunchFailure {
    enum Kind { GRAPHICS, MEMORY, STORAGE, RUNTIME, UNKNOWN }
    static Kind classify(String report){
        String s=report.toLowerCase(java.util.Locale.ROOT);
        for(String error:new String[]{"egl robustness","robust egl","egl driver","choose egl","initialize egl","get egl display","egl context","egl window surface:","egl surface size","egl es3","present framebuffer blit failed","robust buffer access","shader compile failed","shader compilation failed","shader link failed","did not return es3"})if(s.contains(error))return Kind.GRAPHICS;
        if(s.contains("outofmemory")||s.contains("out of memory")||s.contains("reason=3 ")||s.contains("cannot allocate memory"))return Kind.MEMORY;
        if(s.contains("not enough storage")||s.contains("no space left"))return Kind.STORAGE;
        if(s.contains("unsatisfiedlinkerror")||s.contains("unable to load arm64")||s.contains("aot")&&s.contains("mismatch"))return Kind.RUNTIME;
        return Kind.UNKNOWN;
    }
}
