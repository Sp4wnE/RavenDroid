package dev.ravenfield.nativehost;

import android.app.Activity;
import android.app.AlertDialog;
import android.content.ActivityNotFoundException;
import android.content.Intent;
import android.net.Uri;
import android.widget.Toast;

final class PortNotice {
    static final String OFFICIAL_URL="https://github.com/Sp4wnE/RavenDroid";
    static void show(Activity activity) {
        if(activity.isFinishing()||activity.isDestroyed())return;
        new AlertDialog.Builder(activity).setTitle(R.string.ui_port_notice_title).setMessage(R.string.ui_port_notice_message)
            .setPositiveButton(R.string.ui_notice_understood,(dialog,which)->{})
            .setNeutralButton(R.string.ui_notice_github,(dialog,which)->{
                try{activity.startActivity(new Intent(Intent.ACTION_VIEW,Uri.parse(OFFICIAL_URL)));}
                catch(ActivityNotFoundException error){Toast.makeText(activity,R.string.ui_no_browser,Toast.LENGTH_LONG).show();}
            }).show();
    }
}
