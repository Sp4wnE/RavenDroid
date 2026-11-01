package dev.ravenfield.port;

import android.app.Application;

public final class PortApplication extends Application {
    @Override public void onCreate() {
        super.onCreate();
        StartupDiagnostics.initialize(this);
    }
}
