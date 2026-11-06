package dev.ravenfield.nativehost;

import android.app.Activity;
import android.app.ActivityManager;
import android.app.ApplicationExitInfo;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.net.Uri;
import android.os.Bundle;
import android.os.Build;
import android.view.WindowManager;
import java.util.Date;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;
import android.widget.ImageView;
import android.graphics.Bitmap;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileOutputStream;
import java.io.FileInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import org.json.JSONObject;

public final class MainActivity extends Activity {
    private TextView output;
    private LauncherScreen launcher;
    private Button smoke, unity, installed, choose, graphics,live;
    private final android.os.Handler nativePoll=new android.os.Handler(android.os.Looper.getMainLooper());
    private ImageView frame;
    private String report = "";
    private volatile boolean busy;
    private boolean runtimeLoaded;
    private boolean bundledPreparationStarted;
    // GameActivity uses this same explicit name, including in the isolated test applicationId.
    private android.content.SharedPreferences sessionPreferences(){return getSharedPreferences("MainActivity",MODE_PRIVATE);}
    private static native String runProbeNative(String artifactPath, String logPath, String dataPath);
    private static native String runSmokeNative(String artifactPath, String logPath);
    private static native String runGraphicsNative(String artifactPath, String logPath, String dataPath);
    private static native boolean isNativeBusy();

    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        buildLauncher();
        if(state==null)PortNotice.show(this);
        String previous=sessionPreferences().getString("lastReport","");
        boolean pending=sessionPreferences().getBoolean("pending",false);
        setReport(previous.isEmpty()?deviceReport():previous);
        try { System.loadLibrary("ravenfield_native_host"); runtimeLoaded=true; }
        catch(LinkageError error){setReport("Unable to load ARM64 native runtime: "+error+"\n"+deviceReport());nativePoll.post(()->FailureDialog.show(this,report,null));}
        setBusy(pending);
        if(pending&&runtimeLoaded&&isNativeBusy()){waitForNative();}
        else if(pending){
            setReport("Reading native crash details from the previous test…");
            new Thread(()->{
                String recovered="The process ended before the previous native test completed.\n"+deviceReport()+"\n"+exitReport()+"\nLast native stages:\n"+readProgress()+"\n"+previous;
                sessionPreferences().edit().putBoolean("pending",false).putString("lastReport",recovered).putString("lastCrashReport",recovered).commit();
                runOnUiThread(()->{if(isDestroyed())return;setReport(recovered);setBusy(false);FailureDialog.show(this,recovered,null);prepareBundledGame();});
            },"RavenCrashReader").start();
        }
        prepareBundledGame();
    }
    @Override protected void onResume(){super.onResume();if(output!=null&&runtimeLoaded){if(isNativeBusy())waitForNative();else if(!busy)setReport(sessionPreferences().getString("lastReport",report));}}
    @Override protected void onPause(){nativePoll.removeCallbacksAndMessages(null);super.onPause();}
    private void waitForNative(){
        nativePoll.removeCallbacksAndMessages(null);
        if(isNativeBusy()){setBusy(true);setReport("A native session is running or stopping safely…");nativePoll.postDelayed(this::waitForNative,500);}
        else{setBusy(false);setReport(sessionPreferences().getString("lastReport",report));prepareBundledGame();}
    }
    private void prepareBundledGame(){
        if(!BuildConfig.BUNDLED_GAME_DATA||bundledPreparationStarted||busy||!runtimeLoaded)return;
        bundledPreparationStarted=true;
        beginImport(this::openBundledData,true);
    }
    private InputStream openBundledData()throws Exception{
        JSONObject manifest;
        try(InputStream input=getAssets().open("bundled/manifest.json")){
            manifest=new JSONObject(new String(readBounded(input,65536),StandardCharsets.UTF_8));
        }
        GameDataImporter.Profile profile=dataProfile();
        org.json.JSONArray parts=manifest.getJSONArray("parts");
        if(manifest.getInt("format")!=1||manifest.getLong("dataBytes")!=profile.bytes||!manifest.getString("dataSha256").equals(profile.sha256))throw new IOException("Bundled profile mismatch");
        for(int i=0;i<parts.length();i++){
            String name=String.format(java.util.Locale.ROOT,"game.data.%03d.chunk",i);
            if(!parts.getJSONObject(i).getString("file").equals(name))throw new IOException("Invalid bundled data part");
        }
        return new BundledDataStream(parts.length(),index->getAssets().open(String.format(java.util.Locale.ROOT,"bundled/game.data.%03d.chunk",index),android.content.res.AssetManager.ACCESS_STREAMING));
    }
    private File gameDataFile(){return new File(getFilesDir(),"game.data");}
    private File progressFile(){return new File(getFilesDir(),"native-progress.log");}
    private void setReport(String value){report=value;output.setText(value);refreshHome();}
    private void refreshHome(){if(launcher!=null)launcher.status(busy,gameDataFile().isFile(),report);}
    private void buildLauncher(){
        launcher=new LauncherScreen(this,()->{if(!busy&&runtimeLoaded)startActivity(new Intent(this,GameActivity.class));},
            ()->{if(!busy){if(BuildConfig.BUNDLED_GAME_DATA){bundledPreparationStarted=false;prepareBundledGame();}else{Intent intent=new Intent(Intent.ACTION_OPEN_DOCUMENT);intent.setType("*/*");intent.addCategory(Intent.CATEGORY_OPENABLE);startActivityForResult(intent,1);}}},
            ()->{try{String path=getPackageManager().getApplicationInfo("dev.ravenfield.port",0).publicSourceDir;beginImport(()->new FileInputStream(path));}catch(PackageManager.NameNotFoundException|SecurityException error){new android.app.AlertDialog.Builder(this).setTitle(MainActivity.this.getString(R.string.ui_select_game_file_41364)).setMessage(MainActivity.this.getString(R.string.ui_the_prototype_is_not_installed_select_your_d8ce4)).setPositiveButton(MainActivity.this.getString(R.string.ui_ok_2dfaa),null).show();}},
            ()->((ClipboardManager)getSystemService(CLIPBOARD_SERVICE)).setPrimaryClip(ClipData.newPlainText(MainActivity.this.getString(R.string.ui_ravendroid_b77d3),report)),
            ()->startTest(true),()->startTest(false),()->startTest(false,true));
        output=launcher.output;frame=launcher.frame;live=launcher.play;choose=launcher.choose;installed=launcher.installed;smoke=launcher.smoke;unity=launcher.unity;graphics=launcher.graphics;
    }
    private void setBusy(boolean value){
        busy=value;refreshHome();
        smoke.setEnabled(!value&&runtimeLoaded);unity.setEnabled(!value&&runtimeLoaded&&gameDataFile().isFile());
        graphics.setEnabled(!value&&runtimeLoaded&&gameDataFile().isFile());
        live.setEnabled(!value&&runtimeLoaded&&gameDataFile().isFile());
        installed.setEnabled(!value&&runtimeLoaded);choose.setEnabled(!value&&runtimeLoaded);
        if(value)getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        else getWindow().clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
    }
    private void startTest(boolean small){
        startTest(small,false);
    }
    private void startTest(boolean small,boolean render){
        if(busy||!runtimeLoaded)return;
        setBusy(true);if(render)frame.setImageDrawable(null);setReport("Preparing the ARM64 AOT file from the signed APK and verifying SHA-256…");
        new Thread(null,()->{
            String answer;
            try{
                long dataStarted=android.os.SystemClock.elapsedRealtime();
                if(!small&&!GameDataImporter.matches(gameDataFile(),dataProfile()))throw new IOException("Import matching game data first; saved data is missing or from a different build");
                long dataVerifyMs=android.os.SystemClock.elapsedRealtime()-dataStarted;
                long prepareStarted=android.os.SystemClock.elapsedRealtime();
                File artifact=prepareAot(small?"smoke":"unity");
                long prepareMs=android.os.SystemClock.elapsedRealtime()-prepareStarted;
                try(FileOutputStream clear=new FileOutputStream(progressFile())){clear.getFD().sync();}
                String started=deviceReport()+"\nTest="+(small?"small-aot":render?"unity-native-gles":"unity-aot-filesystem")+" artifact_bytes="+artifact.length()+" asset_prepare_ms="+prepareMs+" data_verify_ms="+dataVerifyMs+" started="+new Date();
                sessionPreferences().edit().putBoolean("pending",true).putString("lastReport",started).commit();
                runOnUiThread(()->setReport(started+"\nLoading precompiled code…"));
                String nativeAnswer=small?runSmokeNative(artifact.getAbsolutePath(),progressFile().getAbsolutePath()):render?runGraphicsNative(artifact.getAbsolutePath(),progressFile().getAbsolutePath(),gameDataFile().getAbsolutePath()):runProbeNative(artifact.getAbsolutePath(),progressFile().getAbsolutePath(),gameDataFile().getAbsolutePath());
                answer=started+"\n"+nativeAnswer+"\nNative stages:\n"+readProgress();
                JSONObject nativeReport=new JSONObject(nativeAnswer);
                if(render&&nativeReport.optBoolean("rendered_game_frame")){
                    NativeFrame capture;
                    try(InputStream input=new FileInputStream(new File(getFilesDir(),"native-game-frame.ppm"))){
                        capture=NativeFrame.decode(readBounded(input,16*1024*1024),nativeReport.getJSONObject("graphics").getJSONObject("capture").getString("sha256"));
                    }
                    Bitmap bitmap=Bitmap.createBitmap(capture.pixels,capture.width,capture.height,Bitmap.Config.ARGB_8888);
                    runOnUiThread(()->{if(!isDestroyed())frame.setImageBitmap(bitmap);});
                }
            }catch(Exception|LinkageError error){answer="Native test failed: "+error+"\n"+deviceReport()+"\n"+readProgress();}
            final String finalAnswer=answer;
            sessionPreferences().edit().putBoolean("pending",false).putString("lastReport",finalAnswer).commit();
            runOnUiThread(()->{if(isDestroyed())return;setReport(finalAnswer);setBusy(false);});
        },"RavenNativeProbe",16*1024*1024).start();
    }
    private interface ImportSource {InputStream open()throws Exception;}
    @Override protected void onActivityResult(int request,int result,Intent data){
        super.onActivityResult(request,result,data);
        if(request==1&&result==RESULT_OK&&data!=null&&data.getData()!=null){
            Uri uri=data.getData();beginImport(()->getContentResolver().openInputStream(uri));
        }
    }
    private GameDataImporter.Profile dataProfile()throws Exception{
        JSONObject profile;
        try(InputStream input=getAssets().open("native/data-profile.json")){
            profile=new JSONObject(new String(readBounded(input,65536),StandardCharsets.UTF_8));
        }
        if(profile.getInt("format")!=1||!profile.getString("wasmSha256").equals(BuildProfile.WASM_SHA256)||profile.getInt("partCount")!=15)
            throw new IOException("Game data build profile mismatch");
        return new GameDataImporter.Profile(profile.getLong("dataBytes"),profile.getString("dataSha256"),profile.getInt("partCount"));
    }
    private void beginImport(ImportSource source){
        beginImport(source,false);
    }
    private void beginImport(ImportSource source,boolean bundled){
        if(busy)return;setBusy(true);setReport(bundled?getString(R.string.ui_preparing_bundled_game):"Importing and verifying game data…");
        new Thread(()->{
            String answer;
            try{
                GameDataImporter.Profile profile=dataProfile();
                long started=android.os.SystemClock.elapsedRealtime();
                if(!NativeAssets.verifyGameData(gameDataFile(),profile,true)){
                    long reserve=16L*1024*1024;
                    if(bundled){
                        try(InputStream manifest=getAssets().open("native/manifest.json")){
                            reserve+=new JSONObject(new String(readBounded(manifest,65536),StandardCharsets.UTF_8)).getJSONObject("artifacts").getJSONObject("unity").getLong("bytes");
                        }
                    }
                    if(getFilesDir().getUsableSpace()<profile.bytes+reserve)throw new IOException(bundled?getString(R.string.ui_bundled_game_storage):"About 1.1 GB of free storage is required for game data");
                    try(InputStream input=source.open()){
                        if(input==null)throw new IOException("Could not open APK/ZIP. Try Choose file.");
                        GameDataImporter.importData(input,gameDataFile(),profile,(bytes,total)->runOnUiThread(()->{
                            if(!isDestroyed())setReport(bundled?getString(R.string.ui_bundled_game_progress,(int)(bytes*100/total)):"Importing game data: %"+(bytes*100/total));
                        }));
                    }
                }
                answer=(bundled?getString(R.string.ui_bundled_game_ready)+"\n":"")+"Game data ready: "+profile.bytes+" bytes; SHA-256 verified.\nImport/verification time: "+(android.os.SystemClock.elapsedRealtime()-started)+" ms.\nThe Unity + filesystem test is available under Help.";
            }catch(Exception error){answer="Could not import game data: "+error+"\n"+(bundled?getString(R.string.ui_bundled_game_failed):"If the installed APK cannot be read, use Choose file to select your game content APK or ZIP.");}
            final String result=answer;sessionPreferences().edit().putString("lastReport",result).commit();
            runOnUiThread(()->{if(isDestroyed())return;setReport(result);setBusy(false);});
        },"RavenDataImport").start();
    }
    private File prepareAot(String label)throws Exception{
        JSONObject manifest;
        try(InputStream input=getAssets().open("native/manifest.json")){
            manifest=new JSONObject(new String(readBounded(input,65536),StandardCharsets.UTF_8));
        }
        if(manifest.getInt("format")!=1||!manifest.getString("runtime").equals("wasmtime-49.0.0")
                ||manifest.getBoolean("fuel")!=NativeAssets.usesFuel()||!(manifest.getString("optLevel").equals("none")||manifest.getString("optLevel").equals("speed"))
                ||!manifest.getString("target").equals(BuildConfig.NATIVE_AOT_TARGET)
                ||!manifest.optString("engineProfile","legacy").equals(BuildConfig.NATIVE_ENGINE_PROFILE)
                ||manifest.getBoolean("nativeUnwindInfo")
                ||!manifest.getString("wasmSha256").equals(BuildProfile.WASM_SHA256))
            throw new IOException("AOT build profile mismatch");
        JSONObject entry=manifest.getJSONObject("artifacts").getJSONObject(label);
        String filename=label+".cwasm.bin";
        if(!entry.getString("file").equals(filename))throw new IOException("Invalid AOT asset path");
        String hash=entry.getString("sha256");long size=entry.getLong("bytes");
        File directory=new File(getCodeCacheDir(),"ravenfield-aot");
        if(!directory.isDirectory()&&!directory.mkdirs())throw new IOException("Could not create AOT cache directory");
        File destination=new File(directory,label+".cwasm");
        if(!VerifiedArtifact.matches(destination,hash,size)){
            if(directory.getUsableSpace()<size+16L*1024*1024)throw new IOException("Not enough storage for the AOT file");
            try(InputStream input=getAssets().open("native/"+filename)){
                VerifiedArtifact.copy(input,destination,hash,size);
            }
        }
        return destination;
    }
    private String readProgress(){
        try(java.io.RandomAccessFile input=new java.io.RandomAccessFile(progressFile(),"r")){long size=input.length();int count=(int)Math.min(size,128*1024);byte[] tail=new byte[count];input.seek(size-count);input.readFully(tail);return new String(tail,StandardCharsets.UTF_8);}
        catch(IOException error){return "No native progress log: "+error.getMessage();}
    }
    private static byte[] readBounded(InputStream input,int maximum)throws IOException{
        ByteArrayOutputStream out=new ByteArrayOutputStream();byte[] buffer=new byte[8192];int count;
        while((count=input.read(buffer))!=-1){if(count>maximum-out.size())throw new IOException("Report size limit exceeded");out.write(buffer,0,count);}
        return out.toByteArray();
    }
    private String deviceReport(){
        StringBuilder result=new StringBuilder("RavenDroid "+BuildConfig.VERSION_NAME+"\n");
        result.append(Build.MANUFACTURER).append(' ').append(Build.MODEL).append(" Android ").append(Build.VERSION.RELEASE).append(" API ").append(Build.VERSION.SDK_INT);
        result.append("\nABI=").append(Arrays.toString(Build.SUPPORTED_ABIS));
        result.append(" page_size=").append(android.system.Os.sysconf(android.system.OsConstants._SC_PAGESIZE));
        ActivityManager.MemoryInfo memory=new ActivityManager.MemoryInfo();((ActivityManager)getSystemService(ACTIVITY_SERVICE)).getMemoryInfo(memory);
        result.append("\nRAM total=").append(memory.totalMem).append(" available=").append(memory.availMem).append(" lowMemory=").append(memory.lowMemory).append('\n');
        return result.toString();
    }
    private String exitReport(){
        StringBuilder result=new StringBuilder();
        if(Build.VERSION.SDK_INT>=30){
            try{
                ActivityManager manager=(ActivityManager)getSystemService(ACTIVITY_SERVICE);
                boolean readNative=false;
                for(ApplicationExitInfo exit:manager.getHistoricalProcessExitReasons(getPackageName(),0,5)){
                    result.append(new Date(exit.getTimestamp())).append(" reason=").append(exit.getReason()).append(" status=").append(exit.getStatus())
                        .append(" PSS_KiB=").append(exit.getPss()).append(" ").append(exit.getDescription()).append('\n');
                    if(Build.VERSION.SDK_INT>=31&&exit.getReason()==ApplicationExitInfo.REASON_CRASH_NATIVE&&!readNative){
                        readNative=true;
                        try(InputStream trace=exit.getTraceInputStream()){
                            if(trace==null)result.append("No Android native tombstone available.\n");
                            else result.append(NativeCrashSummary.parse(readBounded(trace,8*1024*1024)));
                        }catch(IOException|RuntimeException error){result.append("Could not read native tombstone: ").append(error).append('\n');}
                    }
                }
            }catch(RuntimeException error){result.append(error);}
        }
        return result.toString();
    }
}
