# R8 (release): the Java shell is shrunk and its names shortened, as GrapheneOS's PDF Viewer is.
# The Rust core finds Native's methods by their JNI names: keep the class and its native methods.
-keep class org.khana.reader.Native { native <methods>; }
-keepclasseswithmembernames,includedescriptorclasses class * { native <methods>; }
# (as PDF Viewer's rule; this app has no JavaScript interface)
-keepclassmembers class * { @android.webkit.JavascriptInterface <methods>; }
