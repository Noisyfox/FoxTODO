#import <Foundation/Foundation.h>
#import <UserNotifications/UserNotifications.h>

// Called on a worker thread. UNUserNotificationCenter keeps requests after app exit.
int fox_schedule(const char *json) {
    @autoreleasepool {
        if (@available(macOS 10.14, *)) {} else { return 3; }
        if (![NSBundle mainBundle].bundleIdentifier) return 3;
        NSData *data = [[NSString stringWithUTF8String:json] dataUsingEncoding:NSUTF8StringEncoding];
        NSArray *rows = [NSJSONSerialization JSONObjectWithData:data options:0 error:nil];
        UNUserNotificationCenter *center = UNUserNotificationCenter.currentNotificationCenter;
        dispatch_semaphore_t ready = dispatch_semaphore_create(0);
        __block BOOL allowed = NO;
        if (rows.count) {
            [center requestAuthorizationWithOptions:(UNAuthorizationOptionAlert|UNAuthorizationOptionSound)
                                  completionHandler:^(BOOL ok, NSError *error){ allowed=ok && !error; dispatch_semaphore_signal(ready); }];
            if (dispatch_semaphore_wait(ready,dispatch_time(DISPATCH_TIME_NOW,120*NSEC_PER_SEC)) || !allowed) return 1;
        }
        __block NSArray *pending;
        [center getPendingNotificationRequestsWithCompletionHandler:^(NSArray *requests){ pending=requests;dispatch_semaphore_signal(ready); }];
        if(dispatch_semaphore_wait(ready,dispatch_time(DISPATCH_TIME_NOW,10*NSEC_PER_SEC))) return 2;
        NSMutableSet *wanted=[NSMutableSet set], *existing=[NSMutableSet set];
        for(NSDictionary *row in rows) [wanted addObject:row[@"id"]];
        for(UNNotificationRequest *r in pending){
            if([wanted containsObject:r.identifier]) [existing addObject:r.identifier];
            else [center removePendingNotificationRequestsWithIdentifiers:@[r.identifier]];
        }
        for(NSDictionary *row in rows){
            if([existing containsObject:row[@"id"]]) continue;
            NSTimeInterval delay=[row[@"at"] doubleValue]/1000 - NSDate.date.timeIntervalSince1970;
            if(delay<=0) continue;
            UNMutableNotificationContent *content=[UNMutableNotificationContent new];
            content.title=@"FoxTODO"; content.body=row[@"title"]; content.sound=UNNotificationSound.defaultSound;
            UNNotificationRequest *request=[UNNotificationRequest requestWithIdentifier:row[@"id"] content:content trigger:[UNTimeIntervalNotificationTrigger triggerWithTimeInterval:MAX(1,delay) repeats:NO]];
            __block BOOL ok=NO;
            [center addNotificationRequest:request withCompletionHandler:^(NSError *error){ok=error==nil;dispatch_semaphore_signal(ready);}];
            if(dispatch_semaphore_wait(ready,dispatch_time(DISPATCH_TIME_NOW,10*NSEC_PER_SEC)) || !ok) return 2;
        }
        return 0;
    }
}
