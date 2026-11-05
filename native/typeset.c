// The translated PDF, laid out by Apple's CoreText (right to left, Farsi letters joined and
// justified as the system does it) on A4 pages with their numbers. Only the app's own text comes
// here (a book's translation), never a document. Wrapped by src/ffi/typeset.rs.
#include <CoreFoundation/CoreFoundation.h>
#include <CoreGraphics/CoreGraphics.h>
#include <CoreText/CoreText.h>
#include <stdlib.h>
#include <string.h>

static CTFontRef font_of(CTFontDescriptorRef desc, CGFloat size)
{
	if (desc) return CTFontCreateWithFontDescriptor(desc, size, NULL);
	return CTFontCreateUIFontForLanguage(kCTFontUIFontSystem, size, CFSTR("fa"));
}

static CTParagraphStyleRef style_of(CTTextAlignment align, CGFloat before, CGFloat after)
{
	CTWritingDirection dir = kCTWritingDirectionRightToLeft;
	CGFloat mult = 1.4;
	CTParagraphStyleSetting s[] = {
		{kCTParagraphStyleSpecifierAlignment, sizeof align, &align},
		{kCTParagraphStyleSpecifierBaseWritingDirection, sizeof dir, &dir},
		{kCTParagraphStyleSpecifierParagraphSpacingBefore, sizeof before, &before},
		{kCTParagraphStyleSpecifierParagraphSpacing, sizeof after, &after},
		{kCTParagraphStyleSpecifierLineHeightMultiple, sizeof mult, &mult},
	};
	return CTParagraphStyleCreate(s, sizeof s / sizeof s[0]);
}

// the page number, in Persian digits, at the foot
static void draw_number(CGContextRef ctx, int page, CTFontRef font, CGColorRef color)
{
	UniChar rev[12], num[12];
	int n = 0, v = page;
	do { rev[n++] = (UniChar)(0x06F0 + v % 10); v /= 10; } while (v && n < 11);
	for (int k = 0; k < n; k++) num[k] = rev[n - 1 - k];
	CFStringRef ns = CFStringCreateWithCharacters(NULL, num, n);
	const void *keys[] = {kCTFontAttributeName, kCTForegroundColorAttributeName};
	const void *vals[] = {font, color};
	CFDictionaryRef attrs = CFDictionaryCreate(NULL, keys, vals, 2, &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks);
	CFAttributedStringRef nas = CFAttributedStringCreate(NULL, ns, attrs);
	CTLineRef line = CTLineCreateWithAttributedString(nas);
	double w = CTLineGetTypographicBounds(line, NULL, NULL, NULL);
	CGContextSetTextPosition(ctx, (595 - w) / 2, 40);
	CTLineDraw(line, ctx);
	CFRelease(line); CFRelease(nas); CFRelease(attrs); CFRelease(ns);
}

// recs: records separated by 0x1E, each a kind byte and UTF-8 text: 'T' the title, 'S' the line
// under it, 'H' a page's heading («صفحه ۵»), 'P' a paragraph. font: the font file (or none: the
// system's). Returns the PDF, malloc'd, or NULL.
unsigned char *kt_farsi_pdf(const char *recs, size_t len, const unsigned char *font, size_t fontlen, size_t *outlen)
{
	*outlen = 0;
	CTFontDescriptorRef desc = NULL;
	if (font && fontlen) {
		CFDataRef fd = CFDataCreate(NULL, font, (CFIndex)fontlen);
		if (fd) { desc = CTFontManagerCreateFontDescriptorFromData(fd); CFRelease(fd); }
	}
	CTFontRef f_title = font_of(desc, 18), f_small = font_of(desc, 9), f_text = font_of(desc, 12);
	CGColorRef ink = CGColorCreateGenericGray(0.0, 1.0), grey = CGColorCreateGenericGray(0.45, 1.0);
	CTParagraphStyleRef s_title = style_of(kCTTextAlignmentCenter, 0, 0), s_sub = style_of(kCTTextAlignmentCenter, 0, 0),
	                    s_head = style_of(kCTTextAlignmentNatural, 0, 0), s_text = style_of(kCTTextAlignmentJustified, 0, 0);
	CFMutableDataRef out = CFDataCreateMutable(NULL, 0);
	CGDataConsumerRef cons = CGDataConsumerCreateWithCFData(out);
	CGRect media = CGRectMake(0, 0, 595, 842);
	CGContextRef ctx = CGPDFContextCreate(cons, &media, NULL);
	// Each block is laid out on its own (one paragraph per framesetter), the pages filled here: in one
	// long text of many paragraphs CoreText took the direction as left to right and laid Farsi out
	// unjoined and backwards; a paragraph alone (even 5000 letters) comes out right.
	const CGFloat left = 56, width = 483, top = 786, bottom = 70;
	CGFloat y = top;
	int page = 0;
	int open = 0;
	size_t i = 0;
	while (ctx && i < len && page < 20000) {
		size_t j = i;
		while (j < len && recs[j] != 0x1E) j++;
		if (j > i + 1) {
			char kind = recs[i];
			CFStringRef body = CFStringCreateWithBytes(NULL, (const UInt8 *)recs + i + 1, (CFIndex)(j - i - 1), kCFStringEncodingUTF8, false);
			if (body) {
				CFMutableAttributedStringRef as = CFAttributedStringCreateMutable(NULL, 0);
				CFAttributedStringReplaceString(as, CFRangeMake(0, 0), body);
				CFRange r = CFRangeMake(0, CFAttributedStringGetLength(as));
				CTFontRef f = kind == 'T' ? f_title : (kind == 'P' ? f_text : f_small);
				CTParagraphStyleRef st = kind == 'T' ? s_title : kind == 'S' ? s_sub : kind == 'H' ? s_head : s_text;
				CGFloat before = kind == 'H' ? 14 : 0, after = kind == 'S' ? 18 : (kind == 'P' ? 8 : 4);
				CFAttributedStringSetAttribute(as, r, kCTFontAttributeName, f);
				CFAttributedStringSetAttribute(as, r, kCTParagraphStyleAttributeName, st);
				CFAttributedStringSetAttribute(as, r, kCTForegroundColorAttributeName, (kind == 'S' || kind == 'H') ? grey : ink);
				CTFramesetterRef fs = CTFramesetterCreateWithAttributedString(as);
				CFIndex pos = 0, total = r.length;
				if (open && y < top) y -= before;
				while (pos < total && page < 20000) {
					if (!open || y - bottom < 20) {
						if (open) { draw_number(ctx, page, f_small, grey); CGPDFContextEndPage(ctx); }
						CGPDFContextBeginPage(ctx, NULL);
						page++;
						open = 1;
						y = top;
					}
					CGMutablePathRef path = CGPathCreateMutable();
					CGPathAddRect(path, NULL, CGRectMake(left, bottom, width, y - bottom));
					CTFrameRef fr = CTFramesetterCreateFrame(fs, CFRangeMake(pos, 0), path, NULL);
					CFRange vis = CTFrameGetVisibleStringRange(fr);
					if (vis.length <= 0) {
						// (not even one line fits: the next page)
						CFRelease(fr); CGPathRelease(path);
						if (y == top) break;
						y = bottom;
						continue;
					}
					CTFrameDraw(fr, ctx);
					CGSize used = CTFramesetterSuggestFrameSizeWithConstraints(fs, vis, NULL, CGSizeMake(width, CGFLOAT_MAX), NULL);
					CFRelease(fr); CGPathRelease(path);
					pos += vis.length;
					y -= used.height;
					if (pos < total) y = bottom;  // (the rest on the next page)
				}
				y -= after;
				CFRelease(fs);
				CFRelease(as);
				CFRelease(body);
			}
		}
		i = j + 1;
	}
	if (open) { draw_number(ctx, page, f_small, grey); CGPDFContextEndPage(ctx); }
	if (ctx) { CGPDFContextClose(ctx); CGContextRelease(ctx); }
	CGDataConsumerRelease(cons);
	CFRelease(s_title); CFRelease(s_sub); CFRelease(s_head); CFRelease(s_text);
	CGColorRelease(ink); CGColorRelease(grey);
	CFRelease(f_title); CFRelease(f_small); CFRelease(f_text);
	if (desc) CFRelease(desc);
	CFIndex n = CFDataGetLength(out);
	unsigned char *res = n > 0 ? malloc((size_t)n) : NULL;
	if (res) {
		memcpy(res, CFDataGetBytePtr(out), (size_t)n);
		*outlen = (size_t)n;
	}
	CFRelease(out);
	return res;
}

void kt_free(void *p) { free(p); }
