// Apple Vision text recognition (offline, part of macOS), set up as the Python app's ocr_page:
// accurate level, languages ar-SA / en-US / nl-NL, language correction on.
#import <Foundation/Foundation.h>
#import <Vision/Vision.h>
#import <CoreGraphics/CoreGraphics.h>
#include <stdlib.h>
#include <string.h>

// Grey page picture (w*h bytes) -> malloc'd text: one line per recognized line,
// "x\ty\tw\th\ttext" with Vision's normalized box (origin bottom left). NULL on failure.
char *k_vision_ocr(const unsigned char *gray, int w, int h)
{
	@autoreleasepool {
		CGColorSpaceRef cs = CGColorSpaceCreateDeviceGray();
		CFDataRef data = CFDataCreate(NULL, gray, (CFIndex)w * h);
		CGDataProviderRef prov = CGDataProviderCreateWithCFData(data);
		CGImageRef img = CGImageCreate(w, h, 8, 8, w, cs, (CGBitmapInfo)kCGImageAlphaNone, prov, NULL, false, kCGRenderingIntentDefault);
		CGDataProviderRelease(prov);
		CFRelease(data);
		CGColorSpaceRelease(cs);
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
		char *r = malloc(strlen(u) + 1);
		strcpy(r, u);
		return r;
	}
}
