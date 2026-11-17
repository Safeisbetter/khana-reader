package org.khana.reader;
import android.app.*;
import android.content.*;
import android.os.*;

public final class AppLaunchTest {
 public static void main(String[] args) {
  Context ctx = (Context) args[0];

  // Test 1: MainActivity can be instantiated
  try {
   MainActivity activity = new MainActivity();
   if(activity == null) throw new AssertionError("MainActivity instantiation failed");
  } catch(Exception e) {
   throw new AssertionError("MainActivity init: " + e.getMessage());
  }

  // Test 2: Native.init() does not crash
  try {
   String dataDir = ctx.getFilesDir().getAbsolutePath();
   String cacheDir = ctx.getCacheDir().getAbsolutePath();
   Native.init(dataDir, cacheDir);
  } catch(Exception e) {
   throw new AssertionError("Native.init failed: " + e.getMessage());
  }

  // Test 3: Intent can be created for MainActivity
  try {
   Intent intent = new Intent(ctx, MainActivity.class);
   if(intent == null) throw new AssertionError("Intent creation failed");
   if(!intent.getComponent().getClassName().equals("org.khana.reader.MainActivity")) {
    throw new AssertionError("Intent wrong class");
   }
  } catch(Exception e) {
   throw new AssertionError("Intent: " + e.getMessage());
  }

  System.out.println("3 app launch initialization cases passed");
 }
}
