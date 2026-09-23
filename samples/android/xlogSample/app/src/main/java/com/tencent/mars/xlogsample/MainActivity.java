package com.tencent.mars.xlogsample;

import android.os.Bundle;
import android.widget.TextView;

import androidx.appcompat.app.AppCompatActivity;

import com.tencent.mars.xlog.Log;
import com.tencent.mars.xlog.Xlog;

public class MainActivity extends AppCompatActivity {

    // libmarsxlog.so is built by cargo (see mars/gradle/mars-cargo.gradle.kts)
    // and packaged in the mars-xlog AAR, so there is no C++ runtime to load.
    static {
        System.loadLibrary("marsxlog");
    }

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        setContentView(R.layout.activity_main);

        String logPath = getExternalFilesDir(null).getPath() + "/xlog";
        Log.setLogImp(new Xlog());
        Log.setConsoleLogOpen(true);
        Log.appenderOpen(Xlog.LEVEL_DEBUG, Xlog.AppednerModeAsync, "", logPath, "LOGSAMPLE", 0);

        Log.d("xlogsample", "appenderOpen, logPath=%s", logPath);
        Log.i("xlogsample", "hello from the Rust port of xlog");
        Log.w("xlogsample", "this warning goes through the same pipeline");
        Log.appenderFlush();

        TextView tv = findViewById(R.id.sample_text);
        tv.setText("Hello from Rust xlog");
    }
}
