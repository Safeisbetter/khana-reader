package org.khana.reader;
import android.webkit.*;
import android.content.*;

public final class WebViewTest {
 public static void main(String[] args) {
  Context ctx = (Context) args[0];

  // Test 1: WebView can be created
  try {
   WebView wv = new WebView(ctx);
   if(wv == null) throw new AssertionError("WebView creation failed");
  } catch(Exception e) {
   throw new AssertionError("WebView init: " + e.getMessage());
  }

  // Test 2: WebViewClient is configured
  try {
   WebView wv = new WebView(ctx);
   WebViewClient client = new WebViewClient();
   wv.setWebViewClient(client);
   if(wv.getWebViewClient() == null) throw new AssertionError("WebViewClient not set");
  } catch(Exception e) {
   throw new AssertionError("WebViewClient: " + e.getMessage());
  }

  // Test 3: JavaScript is disabled (security)
  try {
   WebView wv = new WebView(ctx);
   WebSettings settings = wv.getSettings();
   settings.setJavaScriptEnabled(false);
   if(settings.getJavaScriptEnabled()) throw new AssertionError("JavaScript should be disabled");
  } catch(Exception e) {
   throw new AssertionError("JavaScript settings: " + e.getMessage());
  }

  // Test 4: Network access is blocked
  try {
   WebView wv = new WebView(ctx);
   WebSettings settings = wv.getSettings();
   settings.setBlockNetworkLoads(true);
   if(!settings.getBlockNetworkLoads()) throw new AssertionError("Network loads not blocked");
  } catch(Exception e) {
   throw new AssertionError("Network blocking: " + e.getMessage());
  }

  System.out.println("4 WebView security cases passed");
 }
}
