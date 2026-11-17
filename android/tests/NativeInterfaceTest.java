package org.khana.reader;
import java.nio.charset.StandardCharsets;

public final class NativeInterfaceTest {
 public static void main(String[] args) {
  // Test 1: Native.handle() accepts valid UTF-8
  try {
   byte[] testData = "سلام".getBytes(StandardCharsets.UTF_8);
   byte[] result = Native.handle(testData, testData.length);
   if(result == null) throw new AssertionError("handle() returned null");
   if(result.length == 0) throw new AssertionError("handle() returned empty");
  } catch(Exception e) {
   throw new AssertionError("Native.handle() UTF-8: " + e.getMessage());
  }

  // Test 2: Native.handle() with empty input
  try {
   byte[] testData = new byte[0];
   byte[] result = Native.handle(testData, 0);
   if(result == null) throw new AssertionError("handle() empty returned null");
  } catch(Exception e) {
   throw new AssertionError("Native.handle() empty: " + e.getMessage());
  }

  // Test 3: Native.token() generates consistent tokens
  try {
   byte[] token1 = Native.token();
   byte[] token2 = Native.token();
   if(token1 == null || token2 == null) throw new AssertionError("token() returned null");
   if(token1.length != token2.length) throw new AssertionError("token() length inconsistent");
  } catch(Exception e) {
   throw new AssertionError("Native.token(): " + e.getMessage());
  }

  // Test 4: Bytes-only boundary (no object pointers)
  try {
   byte[] shortData = "test".getBytes(StandardCharsets.UTF_8);
   byte[] longData = "word ".repeat(1000).getBytes(StandardCharsets.UTF_8);

   byte[] result1 = Native.handle(shortData, shortData.length);
   byte[] result2 = Native.handle(longData, longData.length);

   if(result1 == null || result2 == null) throw new AssertionError("handle() on varying sizes failed");
  } catch(Exception e) {
   throw new AssertionError("Native.handle() size limits: " + e.getMessage());
  }

  System.out.println("4 native interface boundary cases passed");
 }
}
