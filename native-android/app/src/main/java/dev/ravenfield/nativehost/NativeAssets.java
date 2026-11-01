package dev.ravenfield.nativehost;
import android.content.Context;
import java.io.*;
import java.nio.charset.StandardCharsets;
import org.json.JSONObject;

final class NativeAssets {
    static boolean usesFuel(){return !"guest-eh-epoch-v1".equals(BuildConfig.NATIVE_ENGINE_PROFILE);}
    private static JSONObject json(Context context,String name)throws Exception{
        try(InputStream input=context.getAssets().open("native/"+name)){
            ByteArrayOutputStream output=new ByteArrayOutputStream();byte[] buffer=new byte[4096];int count;
            while((count=input.read(buffer))!=-1){if(output.size()+count>65536)throw new IOException("Asset manifest too large");output.write(buffer,0,count);}
            return new JSONObject(output.toString(StandardCharsets.UTF_8.name()));
        }
    }
    static File prepareUnity(Context context)throws Exception{
        JSONObject manifest=json(context,"manifest.json"),data=json(context,"data-profile.json");
        if(!manifest.optString("engineProfile","legacy").equals(BuildConfig.NATIVE_ENGINE_PROFILE))throw new IOException("AOT engine profile mismatch");
        if(manifest.getBoolean("fuel")!=usesFuel()||!(manifest.getString("optLevel").equals("none")||manifest.getString("optLevel").equals("speed")))throw new IOException("Unsupported AOT execution profile");
        if(manifest.getInt("format")!=1||!manifest.getString("runtime").equals("wasmtime-49.0.0")||!manifest.getString("target").equals(BuildConfig.NATIVE_AOT_TARGET)||manifest.getBoolean("nativeUnwindInfo")||!manifest.getString("wasmSha256").equals(BuildProfile.WASM_SHA256))throw new IOException("AOT runtime target/profile mismatch");
        if(data.getInt("format")!=1||!data.getString("wasmSha256").equals(BuildProfile.WASM_SHA256)||data.getInt("partCount")!=15)throw new IOException("Game data profile mismatch");
        GameDataImporter.Profile profile=new GameDataImporter.Profile(data.getLong("dataBytes"),data.getString("dataSha256"),15);
        if(!GameDataImporter.matches(new File(context.getFilesDir(),"game.data"),profile))throw new IOException("Import game data from the launcher first");
        JSONObject entry=manifest.getJSONObject("artifacts").getJSONObject("unity");
        if(!entry.getString("file").equals("unity.cwasm.bin"))throw new IOException("Invalid trusted AOT filename");
        File directory=new File(context.getCodeCacheDir(),"ravenfield-aot");
        if(!directory.isDirectory()&&!directory.mkdirs())throw new IOException("AOT cache unavailable");
        File artifact=new File(directory,"unity.cwasm");long size=entry.getLong("bytes");String hash=entry.getString("sha256");
        if(!VerifiedArtifact.matches(artifact,hash,size)){
            if(directory.getUsableSpace()<size+16L*1024*1024)throw new IOException("Not enough storage for AOT code");
            try(InputStream input=context.getAssets().open("native/unity.cwasm.bin")){VerifiedArtifact.copy(input,artifact,hash,size);}
        }return artifact;
    }
}
