// Apple Vision text recognition (offline, part of macOS): accurate level,
// languages ar-SA / en-US / nl-NL, language correction on.
#import <Foundation/Foundation.h>
#import <Vision/Vision.h>
#import <CoreGraphics/CoreGraphics.h>
#include <limits.h>
#include <stdlib.h>
#include <string.h>

static void kv_zero_mem(void *p, size_t n)
{
	if (!p || n == 0) return;
	volatile unsigned char *v = (volatile unsigned char *)p;
	while (n--) *v++ = 0;
}

// Grey page picture (w*h bytes) -> malloc'd text: one line per recognized line,
// "x\ty\tw\th\ttext" with Vision's normalized box (origin bottom left). NULL on failure.
char *k_vision_ocr(const unsigned char *gray, int w, int h)
{
	if (!gray || w <= 0 || h <= 0) return NULL;
	size_t uw = (size_t)w, uh = (size_t)h;
	if (uw > (size_t)LONG_MAX / uh) return NULL;
	@autoreleasepool {
		CGColorSpaceRef cs = CGColorSpaceCreateDeviceGray();
		CFDataRef data = cs ? CFDataCreate(NULL, gray, (CFIndex)(uw * uh)) : NULL;
		CGDataProviderRef prov = data ? CGDataProviderCreateWithCFData(data) : NULL;
		CGImageRef img = prov ? CGImageCreate(uw, uh, 8, 8, uw, cs, (CGBitmapInfo)kCGImageAlphaNone, prov, NULL, false, kCGRenderingIntentDefault) : NULL;
		if (prov) CGDataProviderRelease(prov);
		if (data) CFRelease(data);
		if (cs) CGColorSpaceRelease(cs);
		if (!img) return NULL;
		VNRecognizeTextRequest *req = [[VNRecognizeTextRequest alloc] init];
		req.recognitionLevel = VNRequestTextRecognitionLevelAccurate;
		req.recognitionLanguages = @[@"ar-SA", @"en-US", @"nl-NL"];
		req.usesLanguageCorrection = YES;
		VNImageRequestHandler *handler = [[VNImageRequestHandler alloc] initWithCGImage:img options:@{}];
		NSError *err = nil;
		BOOL ok = [handler performRequests:@[req] error:&err];
		CGImageRelease(img);
		if (!ok) return NULL;
		NSMutableString *out = [NSMutableString string];
		for (VNRecognizedTextObservation *o in req.results ?: @[]) {
			NSArray<VNRecognizedText *> *top = [o topCandidates:1];
			if (top.count == 0) continue;
			NSString *t = [top[0].string stringByReplacingOccurrencesOfString:@"\n" withString:@" "];
			t = [t stringByReplacingOccurrencesOfString:@"\t" withString:@" "];
			CGRect b = o.boundingBox;
			[out appendFormat:@"%.17g\t%.17g\t%.17g\t%.17g\t%@\n", b.origin.x, b.origin.y, b.size.width, b.size.height, t];
		}
		const char *u = [out UTF8String];
		if (!u) return NULL;
		size_t n = strlen(u);
		char *r = malloc(n + 1);
		if (!r) return NULL;
		memcpy(r, u, n + 1);
		return r;
	}
}

void k_vision_free(char *p)
{
	if (!p) return;
	kv_zero_mem(p, strlen(p));
	free(p);
}
