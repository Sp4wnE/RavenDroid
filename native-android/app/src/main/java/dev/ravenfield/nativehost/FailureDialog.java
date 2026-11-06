package dev.ravenfield.nativehost;

import android.app.AlertDialog;
import android.content.*;

final class FailureDialog {
    static void show(Context context,String report,Runnable closed){
        int hint;
        switch(LaunchFailure.classify(report)){
            case GRAPHICS:hint=R.string.ui_failure_graphics;break;
            case MEMORY:hint=R.string.ui_failure_memory;break;
            case STORAGE:hint=R.string.ui_failure_storage;break;
            case RUNTIME:hint=R.string.ui_failure_runtime;break;
            default:hint=R.string.ui_failure_unknown;
        }
        AlertDialog dialog=new AlertDialog.Builder(context).setTitle(R.string.ui_failure_title).setMessage(hint)
            .setPositiveButton(android.R.string.ok,(d,w)->{})
            .setNeutralButton(R.string.ui_copy_last_session_report_6e7df,(d,w)->((ClipboardManager)context.getSystemService(Context.CLIPBOARD_SERVICE)).setPrimaryClip(ClipData.newPlainText("RavenDroid",report))).create();
        dialog.setOnDismissListener(d->{if(closed!=null)closed.run();});dialog.show();
    }
}
