#import <Foundation/Foundation.h>

extern void goNotificationClicked(char *account, char *threadID);
extern void goNotificationCopyCode(char *code);

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
    // The "copy code" button of a verification mail's notification: put the
    // code on the clipboard and leave the window where it is.
    if (notification.activationType == NSUserNotificationActivationTypeActionButtonClicked
        && [info isKindOfClass:[NSDictionary class]]) {
        id codeObj = [info objectForKey:@"code"];
        if ([codeObj isKindOfClass:[NSString class]] && [codeObj length] > 0) {
            goNotificationCopyCode((char *)[codeObj UTF8String]);
            return;
        }
    }
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

void deliverUserNotification(char *title, char *body, char *account, char *threadID, char *identifier, char *code, char *copyLabel) {
    // Owned (+1) copies, not autoreleased: this package is built without ARC, so
    // a dispatch_async block does not retain its captures. Holding the +1 until
    // the block releases them keeps the strings alive until delivery. The char*
    // args are consumed synchronously here, before Go frees them.
    NSString *t = [[NSString alloc] initWithUTF8String:title];
    NSString *b = [[NSString alloc] initWithUTF8String:body];
    NSString *acc = [[NSString alloc] initWithUTF8String:account];
    NSString *tid = [[NSString alloc] initWithUTF8String:threadID];
    NSString *ident = [[NSString alloc] initWithUTF8String:identifier];
    NSString *otp = [[NSString alloc] initWithUTF8String:code];
    NSString *label = [[NSString alloc] initWithUTF8String:copyLabel];
    dispatch_async(dispatch_get_main_queue(), ^{
        NSUserNotification *n = [[NSUserNotification alloc] init];
        n.identifier = ident;
        n.title = t;
        n.informativeText = b;
        n.userInfo = @{@"account": acc, @"threadID": tid, @"code": otp};
        // Without a code there is no button: a click anywhere opens the thread.
        n.hasActionButton = otp.length > 0;
        if (otp.length > 0) {
            n.actionButtonTitle = label;
        }
        [[NSUserNotificationCenter defaultUserNotificationCenter] deliverNotification:n];
        [n release];
        [t release];
        [b release];
        [acc release];
        [tid release];
        [ident release];
        [otp release];
        [label release];
    });
}

// Withdraws a delivered notification by the identifier it was posted with.
// Same ownership rule as above: the +1 copy lives until the block releases it.
void removeUserNotification(char *identifier) {
    NSString *ident = [[NSString alloc] initWithUTF8String:identifier];
    dispatch_async(dispatch_get_main_queue(), ^{
        NSUserNotificationCenter *center = [NSUserNotificationCenter defaultUserNotificationCenter];
        for (NSUserNotification *n in center.deliveredNotifications) {
            if ([n.identifier isEqualToString:ident]) {
                [center removeDeliveredNotification:n];
                break;
            }
        }
        [ident release];
    });
}
