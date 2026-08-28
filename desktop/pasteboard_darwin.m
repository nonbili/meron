#import <AppKit/AppKit.h>
#import <Foundation/Foundation.h>

#include <stdlib.h>
#include <string.h>

// Clipboard image access and "open in the default app" go through AppKit rather
// than shelling out to osascript / open: the App Sandbox denies both execs, so
// the App Store build would silently lose image paste, image copy and attachment
// opening. NSPasteboard and NSWorkspace need no entitlement at all.

// copyBytes returns a malloc'd copy of data for the Go side to take ownership of.
static void *copyBytes(NSData *data, int *outLen) {
    if (data == nil || data.length == 0) {
        *outLen = 0;
        return NULL;
    }
    void *buffer = malloc(data.length);
    if (buffer == NULL) {
        *outLen = 0;
        return NULL;
    }
    memcpy(buffer, data.bytes, data.length);
    *outLen = (int)data.length;
    return buffer;
}

// readPasteboardImage returns the clipboard image as PNG or JPEG bytes, setting
// *outIsPNG to 1 for PNG. The caller frees the returned buffer.
//
// PNG and JPEG are read through verbatim so a pasted screenshot keeps its exact
// bytes; anything else (TIFF from older apps, a promised file) is re-encoded to
// PNG via NSBitmapImageRep.
void *readPasteboardImage(int *outLen, int *outIsPNG) {
    @autoreleasepool {
        *outLen = 0;
        *outIsPNG = 0;
        NSPasteboard *pb = [NSPasteboard generalPasteboard];

        NSData *png = [pb dataForType:NSPasteboardTypePNG];
        if (png != nil && png.length > 0) {
            *outIsPNG = 1;
            return copyBytes(png, outLen);
        }

        NSData *tiff = [pb dataForType:NSPasteboardTypeTIFF];
        if (tiff != nil && tiff.length > 0) {
            NSBitmapImageRep *rep = [NSBitmapImageRep imageRepWithData:tiff];
            NSData *encoded = [rep representationUsingType:NSBitmapImageFileTypePNG properties:@{}];
            if (encoded != nil && encoded.length > 0) {
                *outIsPNG = 1;
                return copyBytes(encoded, outLen);
            }
        }

        // A file dragged or copied in Finder arrives as a file URL rather than
        // image data; read it off disk when it points at an image.
        NSArray<NSURL *> *urls = [pb readObjectsForClasses:@[ [NSURL class] ]
                                                   options:@{NSPasteboardURLReadingFileURLsOnlyKey : @YES}];
        for (NSURL *url in urls) {
            NSString *ext = url.pathExtension.lowercaseString;
            NSData *data = [NSData dataWithContentsOfURL:url];
            if (data == nil || data.length == 0) {
                continue;
            }
            if ([ext isEqualToString:@"jpg"] || [ext isEqualToString:@"jpeg"]) {
                *outIsPNG = 0;
                return copyBytes(data, outLen);
            }
            NSBitmapImageRep *rep = [NSBitmapImageRep imageRepWithData:data];
            if (rep == nil) {
                continue; // not an image file
            }
            NSData *encoded = [rep representationUsingType:NSBitmapImageFileTypePNG properties:@{}];
            if (encoded != nil && encoded.length > 0) {
                *outIsPNG = 1;
                return copyBytes(encoded, outLen);
            }
        }

        return NULL;
    }
}

// writePasteboardImage puts the image file at path on the clipboard. Returns 1
// on success.
//
// The image is written as both its native type and TIFF: some receivers only
// look for TIFF, and NSImage's own TIFF rendering is the reliable way to get it.
int writePasteboardImage(const char *path, int isJPEG) {
    @autoreleasepool {
        NSString *filePath = [NSString stringWithUTF8String:path];
        NSData *data = [NSData dataWithContentsOfFile:filePath];
        if (data == nil || data.length == 0) {
            return 0;
        }
        NSImage *image = [[NSImage alloc] initWithData:data];
        if (image == nil) {
            return 0;
        }

        NSPasteboard *pb = [NSPasteboard generalPasteboard];
        [pb clearContents];

        NSPasteboardType nativeType = isJPEG ? NSPasteboardTypeTIFF : NSPasteboardTypePNG;
        BOOL ok = NO;
        if (!isJPEG) {
            ok = [pb setData:data forType:nativeType];
        }
        NSData *tiff = [image TIFFRepresentation];
        if (tiff != nil && tiff.length > 0) {
            ok = [pb setData:tiff forType:NSPasteboardTypeTIFF] || ok;
        }
        return ok ? 1 : 0;
    }
}

// openPath hands a file to the app registered for it, the sandbox-safe
// equivalent of `open <path>`. Returns 1 on success.
int openPath(const char *path) {
    @autoreleasepool {
        NSString *filePath = [NSString stringWithUTF8String:path];
        NSURL *url = [NSURL fileURLWithPath:filePath];
        return [[NSWorkspace sharedWorkspace] openURL:url] ? 1 : 0;
    }
}
