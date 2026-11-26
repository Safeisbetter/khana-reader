// The translated PDF, laid out by Apple's CoreText (right to left, Farsi letters joined and
// justified as the system does it) on A4 pages with their numbers. Only the app's own text comes
// here (a book's translation), never a document. Wrapped by src/ffi/typeset.rs.
#include <CoreFoundation/CoreFoundation.h>
#include <CoreGraphics/CoreGraphics.h>
#include <CoreText/CoreText.h>
#include <limits.h>
#include <stdlib.h>
#include <string.h>

static void kt_zero_mem(void *p, size_t n)
{
	if (!p || n == 0) return;
	volatile unsigned char *v = (volatile unsigned char *)p;
	while (n--) *v++ = 0;
}

static CTFontRef font_of(CTFontDescriptorRef desc, CGFloat size)
{
	if (desc) {
		CTFontRef f = CTFontCreateWithFontDescriptor(desc, size, NULL);
		if (f) return f;
	}
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
	if (!ctx || !font || !color) return;
	UniChar rev[12], num[12];
	int n = 0, v = page;
	do { rev[n++] = (UniChar)(0x06F0 + v % 10); v /= 10; } while (v && n < 11);
	for (int k = 0; k < n; k++) num[k] = rev[n - 1 - k];
	CFStringRef ns = CFStringCreateWithCharacters(NULL, num, n);
	if (!ns) return;
	const void *keys[] = {kCTFontAttributeName, kCTForegroundColorAttributeName};
	const void *vals[] = {font, color};
	CFDictionaryRef attrs = CFDictionaryCreate(NULL, keys, vals, 2, &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks);
	CFAttributedStringRef nas = attrs ? CFAttributedStringCreate(NULL, ns, attrs) : NULL;
	CTLineRef line = nas ? CTLineCreateWithAttributedString(nas) : NULL;
	if (line) {
		double w = CTLineGetTypographicBounds(line, NULL, NULL, NULL);
		CGContextSetTextPosition(ctx, (595 - w) / 2, 40);
		CTLineDraw(line, ctx);
		CFRelease(line);
	}
	if (nas) CFRelease(nas);
	if (attrs) CFRelease(attrs);
	CFRelease(ns);
}

// recs: records separated by 0x1E, each a kind byte and UTF-8 text: 'T' the title, 'S' the line
// under it, 'H' a page's heading («صفحه ۵»), 'P' a paragraph. font: the font file (or none: the
// system's). Returns the PDF, malloc'd, or NULL.
unsigned char *kt_farsi_pdf(const char *recs, size_t len, const unsigned char *font, size_t fontlen, size_t *outlen)
{
	if (!outlen) return NULL;
	*outlen = 0;
	if (len > 0 && !recs) return NULL;
	if (fontlen > (size_t)LONG_MAX) return NULL;

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
	CGDataConsumerRef cons = out ? CGDataConsumerCreateWithCFData(out) : NULL;
	const void *aux_keys[] = {kCGPDFContextCreator};
	const void *aux_vals[] = {CFSTR("Khana Reader")};
	CFDictionaryRef aux = CFDictionaryCreate(NULL, aux_keys, aux_vals, 1, &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks);
	CGRect media = CGRectMake(0, 0, 595, 842);
	CGContextRef ctx = (cons && f_title && f_small && f_text && ink && grey && s_title && s_sub && s_head && s_text)
	    ? CGPDFContextCreate(cons, &media, aux)
	    : NULL;

	const CGFloat left = 56, width = 483, top = 786, bottom = 70;
	CGFloat y = top;
	int page = 0;
	int open = 0;
	size_t i = 0;
	while (ctx && i < len && page < 20000) {
		size_t j = i;
		while (j < len && recs[j] != 0x1E) j++;
		if (j > i + 1 && (j - i - 1) <= (size_t)LONG_MAX) {
			char kind = recs[i];
			CFStringRef body = CFStringCreateWithBytes(NULL, (const UInt8 *)recs + i + 1, (CFIndex)(j - i - 1), kCFStringEncodingUTF8, false);
			if (body) {
				CFMutableAttributedStringRef as = CFAttributedStringCreateMutable(NULL, 0);
				if (as) {
					CFAttributedStringReplaceString(as, CFRangeMake(0, 0), body);
					CFRange r = CFRangeMake(0, CFAttributedStringGetLength(as));
					CTFontRef f = kind == 'T' ? f_title : (kind == 'P' ? f_text : f_small);
					CTParagraphStyleRef st = kind == 'T' ? s_title : kind == 'S' ? s_sub : kind == 'H' ? s_head : s_text;
					CGFloat before = kind == 'H' ? 14 : 0, after = kind == 'S' ? 18 : (kind == 'P' ? 8 : 4);
					CFAttributedStringSetAttribute(as, r, kCTFontAttributeName, f);
					CFAttributedStringSetAttribute(as, r, kCTParagraphStyleAttributeName, st);
					CFAttributedStringSetAttribute(as, r, kCTForegroundColorAttributeName, (kind == 'S' || kind == 'H') ? grey : ink);
					CTFramesetterRef fs = CTFramesetterCreateWithAttributedString(as);
					if (fs) {
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
							if (!path) break;
							CGPathAddRect(path, NULL, CGRectMake(left, bottom, width, y - bottom));
							CTFrameRef fr = CTFramesetterCreateFrame(fs, CFRangeMake(pos, 0), path, NULL);
							if (!fr) { CGPathRelease(path); break; }
							CFRange vis = CTFrameGetVisibleStringRange(fr);
							if (vis.length <= 0) {
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
							if (pos < total) y = bottom;
						}
						y -= after;
						CFRelease(fs);
					}
					CFRelease(as);
				}
				CFRelease(body);
			}
		}
		i = j + 1;
	}
	if (ctx && open) { draw_number(ctx, page, f_small, grey); CGPDFContextEndPage(ctx); }
	if (ctx) { CGPDFContextClose(ctx); CGContextRelease(ctx); }
	if (aux) CFRelease(aux);
	if (cons) CGDataConsumerRelease(cons);
	if (s_title) CFRelease(s_title);
	if (s_sub) CFRelease(s_sub);
	if (s_head) CFRelease(s_head);
	if (s_text) CFRelease(s_text);
	if (ink) CGColorRelease(ink);
	if (grey) CGColorRelease(grey);
	if (f_title) CFRelease(f_title);
	if (f_small) CFRelease(f_small);
	if (f_text) CFRelease(f_text);
	if (desc) CFRelease(desc);
	unsigned char *res = NULL;
	if (out) {
		CFIndex n = CFDataGetLength(out);
		if (n > 0) {
			res = malloc((size_t)n);
			if (res) {
				memcpy(res, CFDataGetBytePtr(out), (size_t)n);
				*outlen = (size_t)n;
			}
			kt_zero_mem(CFDataGetMutableBytePtr(out), (size_t)n);
		}
		CFRelease(out);
	}
	return res;
}

void kt_free(void *p) { free(p); }

void kt_free_wipe(void *p, size_t len)
{
	if (!p) return;
	kt_zero_mem(p, len);
	free(p);
}
