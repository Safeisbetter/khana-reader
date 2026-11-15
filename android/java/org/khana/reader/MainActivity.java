package org.khana.reader;

import android.Manifest;
import android.app.Activity;
import android.content.Intent;
import android.content.pm.PackageInfo;
import android.content.pm.PackageManager;
import android.graphics.Color;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.system.Os;
import android.view.Gravity;
import android.view.ViewGroup;
import android.view.WindowInsets;
import android.webkit.CookieManager;
import android.webkit.PermissionRequest;
import android.webkit.RenderProcessGoneDetail;
import android.webkit.ValueCallback;
import android.webkit.WebChromeClient;
import android.webkit.WebMessage;
import android.webkit.WebMessagePort;
import android.webkit.WebResourceRequest;
import android.webkit.WebResourceResponse;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.window.OnBackInvokedDispatcher;
import android.widget.TextView;
import java.io.File;

/**
 * The window: Android's WebView showing the reader page (web/index.html, app.js, app.css and pdf.js
 * in its sandboxed frame, the same files as the Mac app's), set up as GrapheneOS's PDF Viewer sets
 * up its own: JavaScript only for the app's own page, no file or content access, no network loads
 * (blockNetworkLoads), no cookies or cache, every request answered inside the app (Server) at the
 * app's own address https://khana.invalid, every navigation refused. There is no JavaScript
 * interface: the page reaches the app only through its requests (and the body port, Server).
 * Files are opened with the system's picker (ACTION_OPEN_DOCUMENT) and saved with its «save as»
 * screen (ACTION_CREATE_DOCUMENT, Exporter): the app has no storage permission.
 */
public final class MainActivity extends Activity implements Exporter.Asker {
    private static final String START = Server.ORIGIN + "/";
    /** the pdf.js frame, the one page the reader page may load into a frame */
    private static final String FRAME_PATH = "/pdfjs/frame.html";
    /** (as GrapheneOS's PDF Viewer: an older WebView is not used) */
    private static final int MIN_WEBVIEW_MAJOR = 133;
    private static final int PICK_FILE = 1, ASK_MIC = 2, SAVE = 3;
    private static Server server;
    /** The page (or a startup message), kept clear of system bars. */
    private LinearLayout outer;
    private FrameLayout root;
    private WebView web;
    private WebMessagePort pagePort;
    private ValueCallback<Uri[]> picking;
    private PermissionRequest micRequest;
    private Exporter.Saved saving;
    private boolean pageBarPainted;
    private int barGround, barInk;

    // the page's own dark colours (web/app.css, --paper)
    private static final int DARK_PAPER = Color.rgb(16, 20, 27);

    /** The device is in dark mode (the page follows by itself: prefers-color-scheme). */
    private boolean night() {
        return (getResources().getConfiguration().uiMode & android.content.res.Configuration.UI_MODE_NIGHT_MASK)
                == android.content.res.Configuration.UI_MODE_NIGHT_YES;
    }

    @Override
    public void onConfigurationChanged(android.content.res.Configuration c) {
        super.onConfigurationChanged(c);
        // (the activity is not recreated for a change of mode: the colours around the page change here)
        boolean n = night();
        if (outer != null) outer.setBackgroundColor(n ? DARK_PAPER : Color.WHITE);
        if (root != null) root.setBackgroundColor(n ? DARK_PAPER : Color.WHITE);
        if (web != null) web.setBackgroundColor(n ? DARK_PAPER : Color.WHITE);
        // Keep a chosen page theme until the page supplies its updated colours. Resetting to
        // green here left the bar green when the page's colour had not changed (and was cached).
        if (outer != null) applyBarColors(pageBarPainted ? barGround : defaultBarGround(), pageBarPainted ? barInk : defaultBarInk());
    }

    private int defaultBarGround() { return night() ? Color.rgb(24, 30, 40) : Color.WHITE; }
    private int defaultBarInk() { return night() ? Color.rgb(231, 235, 242) : Color.rgb(26, 33, 48); }

    @Override
    protected void onCreate(Bundle saved) {
        super.onCreate(saved);
        current = this;
        // (secure from the first moment; applyScreenGuard lifts it only if the reader chose so)
        getWindow().addFlags(android.view.WindowManager.LayoutParams.FLAG_SECURE);
        outer = new LinearLayout(this);
        outer.setOrientation(LinearLayout.VERTICAL);
        outer.setBackgroundColor(night() ? DARK_PAPER : Color.WHITE);
        // The page already has settings, export, font and speed controls. A second, empty native
        // toolbar wasted a full row, especially in landscape. Keep just the system-safe insets.
        root = new FrameLayout(this);
        root.setBackgroundColor(night() ? DARK_PAPER : Color.WHITE);
        outer.addView(root, new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f));
        outer.setOnApplyWindowInsetsListener((v, in) -> {
            if (Build.VERSION.SDK_INT >= 30) {
                android.graphics.Insets i = in.getInsets(WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout() | WindowInsets.Type.ime());
                v.setPadding(i.left, i.top, i.right, i.bottom);
            } else {
                v.setPadding(in.getSystemWindowInsetLeft(), in.getSystemWindowInsetTop(), in.getSystemWindowInsetRight(), in.getSystemWindowInsetBottom());
            }
            return in;
        });
        setContentView(outer);
        applyBarColors(defaultBarGround(), defaultBarInk());
        if (Build.VERSION.SDK_INT >= 33) {
            // (the back gesture, Android 13 and newer; for an app made for Android 16 and newer the only way)
            getOnBackInvokedDispatcher().registerOnBackInvokedCallback(OnBackInvokedDispatcher.PRIORITY_DEFAULT, this::goBack);
        }
        String old = webViewTooOld();
        if (old != null) {
            message(old);
            return;
        }
        if (server != null) {
            applyScreenGuard();
            showPage();
            return;
        }
        TextView wait = message("خوانا آماده می‌شود…\nبار اول چند دقیقه طول می‌کشد (صداها و مدل‌ها باز می‌شوند).");
        new Thread(() -> {
            String err = null;
            try {
                start();
            } catch (Exception e) {
                err = e.toString();
            }
            final String fail = err;
            runOnUiThread(() -> {
                if (fail != null) wait.setText("خوانا باز نشد:\n" + fail);
                else showPage();
            });
        }, "khana-start").start();
    }

    private TextView message(String text) {
        TextView t = new TextView(this);
        t.setText(text);
        t.setGravity(Gravity.CENTER);
        t.setTextSize(18);
        t.setPadding(48, 48, 48, 48);
        root.removeAllViews();
        root.addView(t, new FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT));
        return t;
    }

    private String webViewTooOld() {
        PackageInfo p = WebView.getCurrentWebViewPackage();
        if (p == null) return "Android System WebView روی این گوشی نیست.";
        try {
            int major = Integer.parseInt(p.versionName.split("\\.")[0]);
            if (major < MIN_WEBVIEW_MAJOR) return "WebView این گوشی قدیمی است (" + p.versionName + "). لطفاً Android System WebView را به‌روز کنید.";
        } catch (RuntimeException ignored) {
            // (a version name not in the usual form: tried anyway)
        }
        return null;
    }

    /** Unpacks the page and the models (first start and after an update), then starts the core. */
    private void start() throws Exception {
        File files = getFilesDir();
        PackageInfo pi = getPackageManager().getPackageInfo(getPackageName(), 0);
        File app = new File(files, "app");
        Assets.unpack(this, app, (Build.VERSION.SDK_INT >= 28 ? pi.getLongVersionCode() : pi.versionCode) + "-" + pi.lastUpdateTime);
        File data = new File(files, "data"), out = new File(files, "saved");
        //noinspection ResultOfMethodCallIgnored
        data.mkdirs();
        // (the core's temporary files, as std::env::temp_dir: the app's private cache)
        Os.setenv("TMPDIR", getCacheDir().getPath(), true);
        File ort = new File(getApplicationInfo().nativeLibraryDir, "libonnxruntime.so");
        synchronized (MainActivity.class) {
            if (server == null) {
                Native.init(Native.utf8(data.getPath()), Native.utf8(out.getPath()), Native.utf8(new File(app, "web").getPath()),
                        Native.utf8(new File(app, "models").getPath()), Native.utf8(ort.getPath()));
                server = new Server(this);
                Exporter.unsaved(out);
            }
        }
        current = this;
        applyScreenGuard();
        watchHeat();
        maybeBench(getIntent());
    }

    private void showPage() {
        if (web != null) return;
        CookieManager.getInstance().setAcceptCookie(false);
        web = new WebView(this);
        web.setBackgroundColor(night() ? DARK_PAPER : Color.WHITE);
        WebSettings s = web.getSettings();
        s.setJavaScriptEnabled(true);
        s.setBlockNetworkLoads(true);
        s.setAllowFileAccess(false);
        s.setAllowContentAccess(false);
        s.setAllowFileAccessFromFileURLs(false);
        s.setAllowUniversalAccessFromFileURLs(false);
        s.setGeolocationEnabled(false);
        s.setDatabaseEnabled(false);
        s.setDomStorageEnabled(false);
        s.setSupportMultipleWindows(false);
        s.setJavaScriptCanOpenWindowsAutomatically(false);
        s.setSaveFormData(false);
        s.setCacheMode(WebSettings.LOAD_NO_CACHE);
        s.setSafeBrowsingEnabled(false); // (it would ask Google's servers about pages)
        s.setMixedContentMode(WebSettings.MIXED_CONTENT_NEVER_ALLOW);
        s.setMediaPlaybackRequiresUserGesture(false); // (reading aloud plays page after page)
        s.setTextZoom(Math.max(50, Math.min(300, Math.round(getResources().getConfiguration().fontScale * 100))));
        // (nothing typed in the page, the password of a PDF least of all, is offered to the phone's «autofill» service)
        web.setImportantForAutofill(android.view.View.IMPORTANT_FOR_AUTOFILL_NO_EXCLUDE_DESCENDANTS);
        web.setWebViewClient(new Client());
        web.setWebChromeClient(new Chrome());
        web.setDownloadListener((url, ua, cd, mime, len) -> { }); // (nothing is downloaded)
        root.removeAllViews();
        root.addView(web, new FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT));
        web.loadUrl(START);
    }

    /** Gives the reader page (and only it: the main frame, at the app's address) its body port. */
    private void givePort() {
        server.clearBodies();
        if (pagePort != null) pagePort.close();
        WebMessagePort[] ch = web.createWebMessageChannel();
        pagePort = ch[0];
        pagePort.setWebMessageCallback(new WebMessagePort.WebMessageCallback() {
            @Override
            public void onMessage(WebMessagePort port, WebMessage m) {
                server.onPortMessage(port, m);
            }
        });
        web.postWebMessage(new WebMessage("khana-port:" + server.token(), new WebMessagePort[]{ch[1]}), Uri.parse(Server.ORIGIN));
    }

    private final class Client extends WebViewClient {
        @Override
        public WebResourceResponse shouldInterceptRequest(WebView view, WebResourceRequest req) {
            try {
                return server.handle(req);
            } catch (Exception e) {
                return Server.refuse();
            }
        }

        @Override
        public boolean shouldOverrideUrlLoading(WebView view, WebResourceRequest req) {
            // every navigation refused (links are not followed, the page is not left), but the reader
            // page's own pdf.js frame
            Uri u = req.getUrl();
            boolean frame = !req.isForMainFrame() && "https".equals(u.getScheme()) && Server.HOST.equals(u.getHost())
                    && FRAME_PATH.equals(u.getPath());
            return !frame;
        }

        @Override
        public void onPageFinished(WebView view, String url) {
            if (START.equals(url)) givePort();
        }

        @Override
        public boolean onRenderProcessGone(WebView view, RenderProcessGoneDetail detail) {
            server.clearBodies();
            // the page's process ended (out of memory, a crash): a new WebView with the page again
            if (pagePort != null) pagePort.close();
            pagePort = null;
            root.removeView(view);
            view.destroy();
            web = null;
            showPage();
            return true;
        }
    }

    private final class Chrome extends WebChromeClient {
        @Override
        public boolean onConsoleMessage(android.webkit.ConsoleMessage m) {
            // (the page's and pdf.js's console lines are not passed to the system log, which would otherwise keep them, in the public
            // download: true says «handled». A developer's own build lets them through.)
            return !DEV_TOOLS;
        }

        @Override
        public boolean onShowFileChooser(WebView view, ValueCallback<Uri[]> cb, FileChooserParams params) {
            if (picking != null) picking.onReceiveValue(null);
            picking = cb;
            Intent i = new Intent(Intent.ACTION_OPEN_DOCUMENT).addCategory(Intent.CATEGORY_OPENABLE).setType("*/*");
            String[] accept = params.getAcceptTypes();
            if (accept != null && String.join(",", accept).contains("audio/")) i.setType("audio/*");
            // (several pictures at once are the pages of one document)
            else i.putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true);
            try {
                startActivityForResult(i, PICK_FILE);
            } catch (Exception e) {
                picking = null;
                return false;
            }
            return true;
        }

        @Override
        public void onPermissionRequest(PermissionRequest req) {
            // the microphone only, only for the reader page («صدای من»: recording a sample)
            boolean onlyMic = req.getResources().length == 1 && PermissionRequest.RESOURCE_AUDIO_CAPTURE.equals(req.getResources()[0]);
            if (!onlyMic || !Server.ORIGIN.equals(req.getOrigin().toString().replaceAll("/$", ""))) {
                req.deny();
                return;
            }
            if (checkSelfPermission(Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED) {
                req.grant(new String[]{PermissionRequest.RESOURCE_AUDIO_CAPTURE});
            } else {
                micRequest = req;
                requestPermissions(new String[]{Manifest.permission.RECORD_AUDIO}, ASK_MIC);
            }
        }
    }

    /** Exporter.Asker: the «save as» screen for a file the core made. */
    @Override
    public void ask(Exporter.Saved s) {
        saving = s;
        Intent i = new Intent(Intent.ACTION_CREATE_DOCUMENT).addCategory(Intent.CATEGORY_OPENABLE).setType(s.mime())
                .putExtra(Intent.EXTRA_TITLE, s.file.getName());
        try {
            startActivityForResult(i, SAVE);
        } catch (Exception e) {
            saving = null;
            Exporter.chosen(this, s, null);
        }
    }

    @Override
    protected void onActivityResult(int request, int result, Intent data) {
        Uri u = result == RESULT_OK && data != null ? data.getData() : null;
        if (request == PICK_FILE && picking != null) {
            Uri[] chosen = null;
            if (result == RESULT_OK && data != null && data.getClipData() != null) {
                android.content.ClipData cd = data.getClipData();
                chosen = new Uri[cd.getItemCount()];
                for (int k = 0; k < chosen.length; k++) chosen[k] = cd.getItemAt(k).getUri();
            } else if (u != null) {
                chosen = new Uri[]{u};
            }
            picking.onReceiveValue(chosen);
            picking = null;
        } else if (request == SAVE && saving != null) {
            Exporter.Saved s = saving;
            saving = null;
            Exporter.chosen(this, s, u);
        } else {
            super.onActivityResult(request, result, data);
        }
    }

    @Override
    public void onRequestPermissionsResult(int request, String[] perms, int[] results) {
        if (request == ASK_MIC && micRequest != null) {
            if (results.length > 0 && results[0] == PackageManager.PERMISSION_GRANTED) {
                micRequest.grant(new String[]{PermissionRequest.RESOURCE_AUDIO_CAPTURE});
            } else {
                micRequest.deny();
            }
            micRequest = null;
        }
    }

    /**
     * The phone's own word on how hot it is (Android 10 and newer) is passed to the core, which slows its heavy work when the
     * phone is hot and waits when it is very hot (tts::cool_down): a phone that overheats shuts apps down, or itself.
     */
    private void watchHeat() {
        if (android.os.Build.VERSION.SDK_INT < 29) return;
        try {
            android.os.PowerManager pm = getSystemService(android.os.PowerManager.class);
            pm.addThermalStatusListener(getMainExecutor(), status -> {
                // (Android: 0 none, 1 light, 2 moderate, 3 severe, 4 critical and more)
                final int level = status >= 4 ? 3 : status == 3 ? 2 : status == 2 ? 1 : 0;
                new Thread(() -> {
                    try {
                        Native.handle(Native.utf8("POST"), Native.utf8("/api/heat"), Native.utf8("level=" + level), Native.token(), Native.utf8("{}"));
                    } catch (Throwable e) {
                        // (the core is not ready yet: the next change tells it)
                    }
                }, "khana-heat").start();
            });
        } catch (Throwable e) {
            // (no thermal service on this phone: the work goes on as usual)
        }
    }

    private static volatile MainActivity current;

    /**
     * The screen guard: unless the reader allows screenshots in Settings, the window is marked secure. Then no other app can
     * photograph or record this screen (a screen recorder, a «screen sharing» app), and the list of recent apps shows a blank
     * card instead of the page being read. Asked again whenever the settings are saved (Server).
     */
    /** The system-bar margins in the page's own colours, so they match «رنگ پس‌زمینه»; the status bar's icons
     *  dark on a light bar and light on a dark one. */
    static void paintBar(final int ground, final int ink) {
        final MainActivity a = current;
        if (a == null) return;
        android.util.Log.i("KhanaBar", String.format("bar painted #%06x #%06x", ground & 0xFFFFFF, ink & 0xFFFFFF));
        a.runOnUiThread(() -> {
            if (a.outer == null) return;
            a.pageBarPainted = true;
            a.applyBarColors(ground, ink);
        });
    }

    private void applyBarColors(int ground, int ink) {
        barGround = ground;
        barInk = ink;
        outer.setBackgroundColor(ground);
        boolean light = (0.299 * Color.red(ground) + 0.587 * Color.green(ground) + 0.114 * Color.blue(ground)) > 150;
        if (Build.VERSION.SDK_INT >= 30) {
            android.view.WindowInsetsController c = getWindow().getInsetsController();
            if (c != null) {
                int f = android.view.WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS | android.view.WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS;
                c.setSystemBarsAppearance(light ? f : 0, f);
            }
        }
    }

    static void applyScreenGuard() {
        final MainActivity a = current;
        if (a == null) return;
        boolean allow = false;
        try {
            Native.Reply r = new Native.Reply(Native.handle(Native.utf8("GET"), Native.utf8("/api/init"), Native.utf8(""), Native.token(), null));
            allow = Native.text(r.body).replace(" ", "").contains("\"allowScreenshots\":true");
        } catch (Throwable e) {
            allow = false;
        }
        final boolean secure = !allow;
        a.runOnUiThread(() -> {
            if (secure) a.getWindow().addFlags(android.view.WindowManager.LayoutParams.FLAG_SECURE);
            else a.getWindow().clearFlags(android.view.WindowManager.LayoutParams.FLAG_SECURE);
        });
    }

    /** A developer's speed test (Bench), only when it is asked for over USB with USB debugging switched on. */
    /** The developer's tests (Bench) exist only in a developer's own build: the public download is built with this false (android/build.sh). */
    static final boolean DEV_TOOLS = false;

    private void maybeBench(Intent it) {}

    @Override
    protected void onNewIntent(Intent it) {
        super.onNewIntent(it);
        if (server != null) maybeBench(it);
    }

    @Override
    protected void onResume() {
        super.onResume();
        if (server != null) Exporter.setAsker(this);
    }

    @Override
    protected void onPause() {
        if (saving == null) Exporter.setAsker(null); // (the «save as» screen itself pauses this window)
        super.onPause();
    }

    /** (Android 12 and older; newer ones: the callback in onCreate) */
    @Override
    public void onBackPressed() {
        goBack();
    }

    /** Back (the gesture from either edge, or the button): the page decides first (close a window, or go to the page before); only when it has
     *  nothing to go back to does the app go down (the document stays open; a job keeps running). */
    private void goBack() {
        if (web == null) {
            moveTaskToBack(true);
            return;
        }
        web.evaluateJavascript("window.khanaBack ? window.khanaBack() : false", v -> {
            if (!"true".equals(v)) moveTaskToBack(true);
        });
    }

    @Override
    protected void onDestroy() {
        if (server != null) server.clearBodies();
        Exporter.setAsker(null);
        if (isFinishing() && server != null && !ReadService.reading()) {
            // (the window is ending: the open book is let go, its text overwritten, not left for the process's remaining life)
            try {
                Native.handle(Native.utf8("POST"), Native.utf8("/api/close"), Native.utf8(""), Native.utf8(server.token()), null);
            } catch (RuntimeException ignored) {
                // (nothing more to do)
            }
        }
        if (pagePort != null) pagePort.close();
        if (web != null) {
            web.destroy();
            web = null;
        }
        super.onDestroy();
    }
}
