#import <Foundation/Foundation.h>

extern void goNotificationClicked(char *account, char *threadID);

// NSUserNotification is deprecated in favour of UserNotifications.framework, but
// the modern API requires a code-signed bundle with notification entitlements
// and async authorization. NSUserNotification still works for a bundled Wails app
// and reports clicks via the center delegate, which is what we need here.
@interface MeronNotificationDelegate : NSObject <NSUserNotificationCenterDelegate>
@end

@implementation MeronNotificationDelegate
// Show the banner even when Meron is the frontmost app.
- (BOOL)userNotificationCenter:(NSUserNotificationCenter *)center
     shouldPresentNotification:(NSUserNotification *)notification {
    return YES;
}

- (void)userNotificationCenter:(NSUserNotificationCenter *)center
        didActivateNotification:(NSUserNotification *)notification {
    NSDictionary *info = notification.userInfo;
    NSString *accountStr = @"";
    NSString *threadIDStr = @"";
    if ([info isKindOfClass:[NSDictionary class]]) {
        id accountObj = [info objectForKey:@"account"];
        if ([accountObj isKindOfClass:[NSString class]]) {
            accountStr = accountObj;
        }
        id threadIDObj = [info objectForKey:@"threadID"];
        if ([threadIDObj isKindOfClass:[NSString class]]) {
            threadIDStr = threadIDObj;
        }
    }
    const char *account = [accountStr UTF8String];
    const char *threadID = [threadIDStr UTF8String];
    // Runs on the main thread inside the active autorelease pool, and Go copies
    // the strings synchronously, so the UTF8String buffers stay valid.
    goNotificationClicked((char *)account, (char *)threadID);
}
@end

static MeronNotificationDelegate *meronNotifyDelegate = nil;

void setupNotificationDelegate() {
    dispatch_async(dispatch_get_main_queue(), ^{
        if (meronNotifyDelegate == nil) {
            meronNotifyDelegate = [[MeronNotificationDelegate alloc] init];
        }
        [NSUserNotificationCenter defaultUserNotificationCenter].delegate = meronNotifyDelegate;
    });
}

void deliverUserNotification(char *title, char *body, char *account, char *threadID) {
    // Owned (+1) copies, not autoreleased: this package is built without ARC, so
    // a dispatch_async block does not retain its captures. Holding the +1 until
    // the block releases them keeps the strings alive until delivery. The char*
    // args are consumed synchronously here, before Go frees them.
    NSString *t = [[NSString alloc] initWithUTF8String:title];
    NSString *b = [[NSString alloc] initWithUTF8String:body];
    NSString *acc = [[NSString alloc] initWithUTF8String:account];
    NSString *tid = [[NSString alloc] initWithUTF8String:threadID];
    dispatch_async(dispatch_get_main_queue(), ^{
        NSUserNotification *n = [[NSUserNotification alloc] init];
        n.title = t;
        n.informativeText = b;
        n.userInfo = @{@"account": acc, @"threadID": tid};
        [[NSUserNotificationCenter defaultUserNotificationCenter] deliverNotification:n];
        [n release];
        [t release];
        [b release];
        [acc release];
        [tid release];
    });
}
