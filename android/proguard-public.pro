# The public download only (android/build.sh with KHANA_PUBLIC=1): every call to Android's log is removed with its arguments, so
# the app writes nothing to the system log, which other apps with log access, a bug report or a USB cable could read.
-assumenosideeffects class android.util.Log {
    public static boolean isLoggable(java.lang.String, int);
    public static int v(...);
    public static int d(...);
    public static int i(...);
    public static int w(...);
    public static int e(...);
    public static int wtf(...);
    public static int println(...);
    public static java.lang.String getStackTraceString(java.lang.Throwable);
}
# (no source file names or line numbers in the shrunk classes)
-renamesourcefileattribute ""
