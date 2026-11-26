#import <Foundation/Foundation.h>
#import <AppKit/AppKit.h>
#import <WebKit/WebKit.h>

// Default-deny WebKit network blocker. Everything is blocked unless it uses Khana's
// local custom schemes (`khana:`, `khanapdf:`) or in-memory `blob:`/`data:` URLs.
// Covers macOS 13 as well, where WKWebsiteDataStore proxy settings are unavailable.
int khana_install_offline_network_filter(void *raw_controller) {
    if (raw_controller == NULL) return 0;

    static WKContentRuleList *cached_list = nil;
    WKContentRuleList *list = cached_list;

    if (list == nil) {
        WKContentRuleListStore *store = [WKContentRuleListStore defaultStore];
        if (store == nil) return 0;

        static NSString * const rules =
            @"["
             "{\"trigger\":{\"url-filter\":\".*\"},\"action\":{\"type\":\"block\"}},"
             "{\"trigger\":{\"url-filter\":\"^khana:\",\"url-filter-is-case-sensitive\":false},\"action\":{\"type\":\"ignore-previous-rules\"}},"
             "{\"trigger\":{\"url-filter\":\"^khanapdf:\",\"url-filter-is-case-sensitive\":false},\"action\":{\"type\":\"ignore-previous-rules\"}},"
             "{\"trigger\":{\"url-filter\":\"^blob:\",\"url-filter-is-case-sensitive\":false},\"action\":{\"type\":\"ignore-previous-rules\"}},"
             "{\"trigger\":{\"url-filter\":\"^data:\",\"url-filter-is-case-sensitive\":false},\"action\":{\"type\":\"ignore-previous-rules\"}}"
             "]";

        NSCondition *condition = [NSCondition new];
        __block WKContentRuleList *compiled = nil;
        __block BOOL finished = NO;
        [store compileContentRuleListForIdentifier:@"org.khana.reader.offline-network-v3"
                             encodedContentRuleList:rules
                                    completionHandler:^(WKContentRuleList *result, NSError *error) {
            [condition lock];
            if (error == nil) compiled = result;
            finished = YES;
            [condition broadcast];
            [condition unlock];
        }];

        NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:20.0];
        while (YES) {
            [condition lock];
            BOOL ready = finished;
            [condition unlock];
            if (ready || [deadline timeIntervalSinceNow] <= 0.0) break;
            [[NSRunLoop currentRunLoop] runMode:NSDefaultRunLoopMode
                                     beforeDate:[NSDate dateWithTimeIntervalSinceNow:0.05]];
        }

        [condition lock];
        list = finished ? compiled : nil;
        [condition unlock];
        if (list == nil) return 0;
        cached_list = list;
    }

    WKUserContentController *controller = (__bridge WKUserContentController *)raw_controller;
    [controller addContentRuleList:list];
    return 1;
}

// Returns the real macOS major version (immune to SYSTEM_VERSION_COMPAT=1).
int khana_macos_major_version(void) {
    NSOperatingSystemVersion ver = [[NSProcessInfo processInfo] operatingSystemVersion];
    return (int)ver.majorVersion;
}

// Excludes Khana's private data directory from Time Machine backups and Spotlight indexing,
// and ensures 0700 permissions.
void khana_protect_data_dir(const char *path) {
    if (path == NULL || path[0] == '\0') return;
    @autoreleasepool {
        NSString *ns = [NSString stringWithUTF8String:path];
        if (ns == nil) return;
        NSFileManager *fm = [NSFileManager defaultManager];
        [fm createDirectoryAtPath:ns
      withIntermediateDirectories:YES
                       attributes:@{ NSFilePosixPermissions: @(0700) }
                            error:nil];
        [fm setAttributes:@{ NSFilePosixPermissions: @(0700) } ofItemAtPath:ns error:nil];
        NSURL *url = [NSURL fileURLWithPath:ns isDirectory:YES];
        [url setResourceValue:@YES forKey:NSURLIsExcludedFromBackupKey error:nil];
        NSString *noIndex = [ns stringByAppendingPathComponent:@".metadata_never_index"];
        if (![fm fileExistsAtPath:noIndex]) {
            [fm createFileAtPath:noIndex contents:[NSData data] attributes:@{ NSFilePosixPermissions: @(0600) }];
        }
    }
}

// Enforces macOS window capture protection (NSWindowSharingNone when allow == 0).
void khana_set_window_sharing(int allow) {
    dispatch_async(dispatch_get_main_queue(), ^{
        @autoreleasepool {
            NSWindowSharingType mode = allow ? NSWindowSharingReadOnly : NSWindowSharingNone;
            for (NSWindow *w in [NSApp windows]) {
                [w setSharingType:mode];
            }
        }
    });
}

static NSInteger khana_last_pb_change = -1;

// Copies text to the macOS pasteboard marked Transient + Concealed (blocks iCloud Universal
// Clipboard and clipboard history managers) and auto-clears after `clear_secs` if unchanged.
void khana_copy_transient(const char *utf8, int clear_secs) {
    if (utf8 == NULL) return;
    @autoreleasepool {
        NSString *text = [NSString stringWithUTF8String:utf8];
        if (text == nil) return;
        dispatch_async(dispatch_get_main_queue(), ^{
            @autoreleasepool {
                NSPasteboard *pb = [NSPasteboard generalPasteboard];
                [pb clearContents];
                NSPasteboardItem *item = [[NSPasteboardItem alloc] init];
                [item setString:text forType:NSPasteboardTypeString];
                [item setData:[NSData data] forType:@"org.nspasteboard.TransientType"];
                [item setData:[NSData data] forType:@"org.nspasteboard.ConcealedType"];
                [item setData:[NSData data] forType:@"org.nspasteboard.AutoGeneratedType"];
                [pb writeObjects:@[item]];
                NSInteger count = [pb changeCount];
                khana_last_pb_change = count;
                if (clear_secs > 0) {
                    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)clear_secs * NSEC_PER_SEC),
                                   dispatch_get_main_queue(), ^{
                        NSPasteboard *pb2 = [NSPasteboard generalPasteboard];
                        if (khana_last_pb_change == count && [pb2 changeCount] == count) {
                            [pb2 clearContents];
                            khana_last_pb_change = -1;
                        }
                    });
                }
            }
        });
    }
}

// Clears the macOS pasteboard immediately if it still holds Khana's copied text.
void khana_clear_transient(void) {
    dispatch_async(dispatch_get_main_queue(), ^{
        @autoreleasepool {
            NSPasteboard *pb = [NSPasteboard generalPasteboard];
            if (khana_last_pb_change >= 0 && [pb changeCount] == khana_last_pb_change) {
                [pb clearContents];
                khana_last_pb_change = -1;
            }
        }
    });
}
