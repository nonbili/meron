#import <Foundation/Foundation.h>
#import <AppKit/AppKit.h>

extern void goSystemResumed(void);

// Observes NSWorkspaceDidWakeNotification, which NSWorkspace's own notification
// center posts when the Mac wakes from sleep. On wake we tell Go so the sidecar
// reconnects its IDLE watchers.
@interface MeronResumeObserver : NSObject
@end

@implementation MeronResumeObserver
- (void)didWake:(NSNotification *)note {
    goSystemResumed();
}
@end

static MeronResumeObserver *meronResumeObserver = nil;

void setupResumeObserver() {
    dispatch_async(dispatch_get_main_queue(), ^{
        if (meronResumeObserver == nil) {
            meronResumeObserver = [[MeronResumeObserver alloc] init];
        }
        [[[NSWorkspace sharedWorkspace] notificationCenter]
            addObserver:meronResumeObserver
               selector:@selector(didWake:)
                   name:NSWorkspaceDidWakeNotification
                 object:nil];
    });
}

void teardownResumeObserver() {
    dispatch_async(dispatch_get_main_queue(), ^{
        if (meronResumeObserver != nil) {
            [[[NSWorkspace sharedWorkspace] notificationCenter]
                removeObserver:meronResumeObserver];
        }
    });
}
