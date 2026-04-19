#import <Cocoa/Cocoa.h>
#import <IOSurface/IOSurface.h>
#import <MetalKit/MetalKit.h>
#import <fcntl.h>
#import <stdatomic.h>
#import <stdint.h>
#import <string.h>
#import <sys/mman.h>
#import <unistd.h>
#import <QuartzCore/QuartzCore.h>

extern void sys_ipc_push_command(uint32_t cmd, uint64_t arg1, uint32_t arg2);
extern void sys_ipc_send_r2m_command_with_payload(uint32_t cmd_type,
                                                  uint64_t arg1,
                                                  uint64_t payload_ptr,
                                                  uint32_t payload_len);

// Epic 69: Forward declarations for full R2M read
typedef struct {
  uint32_t type;
  uint64_t arg1;
  uint32_t arg2;
  uint64_t payload_ptr;
  uint32_t payload_len;
} SaltIPCCommand;

typedef struct __attribute__((packed)) {
  uint32_t type;
  uint64_t arg1;
  uint32_t arg2;
  uint32_t payload_len;
  uint8_t payload[1024];
} ChildIPCCmd;

typedef struct {
  _Atomic uint32_t head;
  _Atomic uint32_t tail;
} RingHeader;
extern SaltIPCCommand *sys_ipc_read_r2m_command_full(void);

// Epic 69: OOPIF child tracker (max 16 iframes)
#define MAX_OOPIF_CHILDREN 16
static int oopif_child_shm_fds[MAX_OOPIF_CHILDREN] = {0};
static pid_t oopif_child_pids[MAX_OOPIF_CHILDREN] = {0};
static uint32_t oopif_child_node_ids[MAX_OOPIF_CHILDREN] = {0};
static uint8_t *oopif_child_shm_ptrs[MAX_OOPIF_CHILDREN] = {NULL};
static int oopif_child_count = 0;

// Epic 76: Service Worker Tracker
#define CMD_WORKER_REGISTERED 7
#define CMD_FETCH_INTERCEPT 8
#define CMD_FETCH_RESPONSE 9

static int sw_shm_fds[MAX_OOPIF_CHILDREN] = {0};
static uint8_t *sw_shm_ptrs[MAX_OOPIF_CHILDREN] = {NULL};
static pid_t sw_pids[MAX_OOPIF_CHILDREN] = {0};
static int sw_count = 0;

// Epic 75: CDM Sandbox Tracker
#define CMD_CDM_REGISTERED 10
#define CMD_CDM_INIT 11
static int cdm_shm_fds[MAX_OOPIF_CHILDREN] = {0};
static uint8_t *cdm_shm_ptrs[MAX_OOPIF_CHILDREN] = {NULL};
static pid_t cdm_pids[MAX_OOPIF_CHILDREN] = {0};
static int cdm_count = 0;

@interface FlippedView : NSView
@end
@implementation FlippedView
- (BOOL)isFlipped { return YES; }
- (BOOL)wantsUpdateLayer { return YES; }
- (void)updateLayer {
  // Purposefully left blank. AppKit will only call this instead of drawRect:,
  // which leaves layer.contents safely persistent without AppKit stomping it.
}
@end

@interface BrowserChrome : NSWindowController <NSTextFieldDelegate>
@property(nonatomic, strong) NSTextField *omnibox;
@property(nonatomic, strong) NSButton *backButton;
@property(nonatomic, strong) NSButton *forwardButton;
@property(nonatomic, strong) NSView *tabContentView;
@property(nonatomic, strong) NSMutableArray *tabPIDs;
@end

static BrowserChrome *globalChrome = NULL;

@implementation BrowserChrome

- (instancetype)initWithWindow:(NSWindow *)window {
  self = [super initWithWindow:window];
  if (self) {
    NSView *contentView = window.contentView;
    self.tabPIDs = [NSMutableArray array];

    // Setup Omnibox
    self.omnibox = [[NSTextField alloc]
        initWithFrame:NSMakeRect(100, contentView.frame.size.height - 40,
                                 contentView.frame.size.width - 120, 30)];
    self.omnibox.autoresizingMask = NSViewWidthSizable | NSViewMinYMargin;
    self.omnibox.delegate = self;
    [contentView addSubview:self.omnibox];

    // Setup Back Button
    self.backButton = [NSButton buttonWithTitle:@"Back"
                                         target:self
                                         action:@selector(onBackButtonClicked)];
    self.backButton.frame =
        NSMakeRect(10, contentView.frame.size.height - 40, 40, 30);
    self.backButton.autoresizingMask = NSViewMaxXMargin | NSViewMinYMargin;
    [contentView addSubview:self.backButton];

    // Setup Tab Content View (CoreAnimation Layer container)
    self.tabContentView = [[FlippedView alloc]
        initWithFrame:NSMakeRect(0, 0, contentView.frame.size.width,
                                 contentView.frame.size.height - 40)];
    self.tabContentView.autoresizingMask =
        NSViewWidthSizable | NSViewHeightSizable;
    [self.tabContentView setWantsLayer:YES];
    self.tabContentView.layer.contentsGravity = kCAGravityTopLeft;
    self.tabContentView.layer.masksToBounds = YES;
    [contentView addSubview:self.tabContentView];

    globalChrome = self;

    // Poll for IPC frames from child renderer
    [NSTimer scheduledTimerWithTimeInterval:1.0 / 60.0
                                     target:self
                                   selector:@selector(pollIPCForActiveTab)
                                   userInfo:nil
                                    repeats:YES];

    [self createNewTabWithURL:@"https://google.com"];
  }
  return self;
}

- (void)createNewTabWithURL:(NSString *)url {
  // 1. Create a POSIX shared memory object
  shm_unlink("/keuos_tab");
  int shm_fd = shm_open("/keuos_tab", O_CREAT | O_RDWR, 0666);
  if (shm_fd < 0) {
    perror("mac_app shm_open tab failed");
  }
  ftruncate(shm_fd, 2097152);

  // Clear FD_CLOEXEC so the fd survives execl() into the child renderer
  int flags = fcntl(shm_fd, F_GETFD);
  fcntl(shm_fd, F_SETFD, flags & ~FD_CLOEXEC);

  // We map it locally as well for Main Process -> Renderer comms
  extern int32_t ext_ipc_init_shared_memory(int32_t fd);
  ext_ipc_init_shared_memory(shm_fd);

  // 2. Fork the Renderer Process
  pid_t pid = fork();
  if (pid == 0) {
    // We are the child. Exec the Prisimi engine with the shared FD.
    char fd_str[16];
    sprintf(fd_str, "%d", shm_fd);
    execl("/tmp/salt_build/prisimi_renderer", "prisimi_renderer", "--ipc-fd",
          fd_str, "--url", [url UTF8String], NULL);
    exit(1); // Failsafe
  } else {
    // We are the parent. Track the Tab PID
    [self.tabPIDs addObject:@(pid)];

    // Epic 75: Spawn CDM sandbox for this tab
    [self spawnCDMSandboxForTab:1];
  }
}

// Epic 69: Spawn a child renderer for an OOPIF iframe
- (void)spawnOOPIFForNode:(uint32_t)nodeIdx
                  withURL:(const char *)url
                   urlLen:(uint32_t)urlLen {
  if (oopif_child_count >= MAX_OOPIF_CHILDREN) {
    NSLog(@"[OOPIF] Max iframe children reached (%d)", MAX_OOPIF_CHILDREN);
    return;
  }

  // 1. Create per-iframe shared memory region
  char shm_name[64];
  snprintf(shm_name, sizeof(shm_name), "/prisimi_iframe_%u_%d", nodeIdx,
           oopif_child_count);
  shm_unlink(shm_name);
  int iframe_shm_fd = shm_open(shm_name, O_CREAT | O_RDWR, 0666);
  if (iframe_shm_fd < 0) {
    perror("[OOPIF] shm_open failed");
    return;
  }
  ftruncate(iframe_shm_fd, 2097152);

  // Map it in the main process so we can poll the child's R2M ring
  void *iframe_shm_ptr =
      mmap(NULL, 2097152, PROT_READ | PROT_WRITE, MAP_SHARED, iframe_shm_fd, 0);
  if (iframe_shm_ptr == MAP_FAILED) {
    perror("[OOPIF] mmap failed");
    close(iframe_shm_fd);
    return;
  }
  memset(iframe_shm_ptr, 0, 2097152); // Zero-init ring headers

  // 2. Build null-terminated URL string for execl
  char url_buf[1024];
  uint32_t copy_len = urlLen < 1023 ? urlLen : 1023;
  memcpy(url_buf, url, copy_len);
  url_buf[copy_len] = '\0';

  // 3. Fork & Exec the iframe renderer
  pid_t iframe_pid = fork();
  if (iframe_pid == 0) {
    // Child: exec renderer with iframe's private IPC FD
    char fd_str[16];
    sprintf(fd_str, "%d", iframe_shm_fd);
    execl("/tmp/salt_build/prisimi_renderer", "prisimi_renderer", "--ipc-fd",
          fd_str, "--url", url_buf, NULL);
    _exit(1);
  }

  // 4. Track the child in our OOPIF table
  int slot = oopif_child_count;
  oopif_child_shm_fds[slot] = iframe_shm_fd;
  oopif_child_pids[slot] = iframe_pid;
  oopif_child_node_ids[slot] = nodeIdx;
  oopif_child_shm_ptrs[slot] = (uint8_t *)iframe_shm_ptr;
  oopif_child_count++;

  NSLog(@"[OOPIF] Spawned iframe renderer PID=%d for node=%u URL=%s",
        iframe_pid, nodeIdx, url_buf);
}

- (void)pollIPCForActiveTab {
  // ═══════════════════════════════════════════════════════════
  // Phase A: Read parent renderer's R2M ring (tab-level)
  // ═══════════════════════════════════════════════════════════
  for (int drain = 0; drain < 10; drain++) {
    SaltIPCCommand *cmd = sys_ipc_read_r2m_command_full();
    if (!cmd)
      break;

    NSLog(@"[MainProcess] IPC cmd type=%u arg1=%llu arg2=%u", cmd->type,
          (unsigned long long)cmd->arg1, cmd->arg2);

    if (cmd->type == 1) {
      // COMMAND_NEW_FRAME: Tab surface update
      uint32_t surfaceID = (uint32_t)cmd->arg1;
      NSLog(@"[MainProcess] CMD_NEW_FRAME surfaceID=%u", surfaceID);
      if (surfaceID != 0) {
        IOSurfaceRef childSurface = IOSurfaceLookup(surfaceID);
        NSLog(@"[MainProcess] IOSurfaceLookup result=%p", childSurface);
        if (childSurface) {
          self.tabContentView.layer.contents = (__bridge id)childSurface;
          [self.tabContentView.layer setNeedsDisplay];
          CFRelease(childSurface);
        }
      }
    } else if (cmd->type == 4) {
      // CMD_SPAWN_IFRAME: Parent renderer requests an OOPIF child
      uint32_t nodeIdx = (uint32_t)cmd->arg1;
      // The iframe src URL was stored in the DOM's SoA by the lexer.
      // For now, read it from the payload if available, otherwise use a
      // default.
      const char *iframe_url = "about:blank";
      uint32_t iframe_url_len = 11;
      if (cmd->payload_len > 0) {
        uint64_t full_ptr = cmd->payload_ptr;
        iframe_url = (const char *)full_ptr;
        iframe_url_len = cmd->payload_len;
      }
      [self spawnOOPIFForNode:nodeIdx withURL:iframe_url urlLen:iframe_url_len];
    } else if (cmd->type == 6) {
      // CMD_POST_MESSAGE: Route postMessage between processes
      // arg1 = target node index, payload = serialized message
      uint32_t target_node = (uint32_t)cmd->arg1;
      uint64_t payload_ptr = cmd->payload_ptr;

      for (int i = 0; i < oopif_child_count; i++) {
        if (oopif_child_node_ids[i] == target_node) {
          // Push EVENT_POST_MESSAGE(5) into the child's M2R ring (offset 0)
          uint8_t *child_shm = oopif_child_shm_ptrs[i];
          RingHeader *m2r = (RingHeader *)child_shm;
          uint32_t head = atomic_load(&m2r->head);
          uint32_t tail = atomic_load(&m2r->tail);
          uint32_t capacity = (65536 - 8) / sizeof(ChildIPCCmd);

          if (((head + 1) % capacity) != tail) {
            ChildIPCCmd *child_m2r_cmds = (ChildIPCCmd *)(child_shm + 8);
            ChildIPCCmd *target_cmd = &child_m2r_cmds[head % capacity];
            target_cmd->type = 5; // EVENT_POST_MESSAGE
            target_cmd->arg1 = 0;
            target_cmd->payload_len = cmd->payload_len;
            if (cmd->payload_len > 0 && cmd->payload_len <= 1024) {
              memcpy(target_cmd->payload, (void *)payload_ptr,
                     cmd->payload_len);
            }
            atomic_store(&m2r->head, (head + 1) % capacity);
          }

          NSLog(@"[OOPIF] Routing postMessage to iframe PID=%d node=%u",
                oopif_child_pids[i], target_node);
          break;
        }
      }
    } else if (cmd->type == CMD_WORKER_REGISTERED) {
      // CMD_WORKER_REGISTERED: Main process requests worker spawn
      // arg1 = Tab ID, Payload = Script URL
      uint32_t tabId = (uint32_t)cmd->arg1;
      NSString *scriptUrl = @"sw.js";
      if (cmd->payload_len > 0) {
        uint64_t full_ptr = cmd->payload_ptr;
        scriptUrl = [NSString stringWithUTF8String:(const char *)full_ptr];
      }
      [self registerServiceWorker:scriptUrl forTab:tabId];
    } else if (cmd->type == CMD_FETCH_INTERCEPT) {
      // CMD_FETCH_INTERCEPT: Renderer requests fetch intercept (arg1 =
      // fetch_id)
      uint64_t fetch_id = cmd->arg1;
      // Route to target Service Worker (for Phase 1, assume sw_shm_ptrs[0])
      if (sw_count > 0 && sw_shm_ptrs[0]) {
        uint8_t *sw_shm = sw_shm_ptrs[0];
        RingHeader *m2r = (RingHeader *)sw_shm;
        uint32_t head = atomic_load(&m2r->head);
        uint32_t tail = atomic_load(&m2r->tail);
        uint32_t capacity = (131072 - 8) / sizeof(ChildIPCCmd);

        if (((head + 1) % capacity) != tail) {
          ChildIPCCmd *sw_cmds = (ChildIPCCmd *)(sw_shm + 8);
          ChildIPCCmd *target_cmd = &sw_cmds[head % capacity];
          target_cmd->type = CMD_FETCH_INTERCEPT;
          target_cmd->arg1 = fetch_id;
          target_cmd->payload_len = cmd->payload_len;
          if (cmd->payload_len > 0 && cmd->payload_len <= 1024) {
            uint64_t full_ptr = cmd->payload_ptr;
            memcpy(target_cmd->payload, (void *)full_ptr, cmd->payload_len);
          }
          atomic_store(&m2r->head, (head + 1) % capacity);
        }
      }
    } else if (cmd->type == CMD_CDM_INIT) {
      // CMD_CDM_INIT: Route EME request to sandboxed CDM process
      if (cdm_count > 0 && cdm_shm_ptrs[0]) {
        uint8_t *cdm_shm = cdm_shm_ptrs[0];
        RingHeader *m2r = (RingHeader *)cdm_shm;
        uint32_t head = atomic_load(&m2r->head);
        uint32_t tail = atomic_load(&m2r->tail);
        uint32_t capacity = (131072 - 8) / sizeof(ChildIPCCmd);

        if (((head + 1) % capacity) != tail) {
          ChildIPCCmd *cdm_cmds = (ChildIPCCmd *)(cdm_shm + 8);
          ChildIPCCmd *target_cmd = &cdm_cmds[head % capacity];
          target_cmd->type = CMD_CDM_INIT;
          target_cmd->arg1 = cmd->arg1; // Promise ID
          target_cmd->payload_len = cmd->payload_len;
          if (cmd->payload_len > 0 && cmd->payload_len <= 1024) {
            uint64_t full_ptr = cmd->payload_ptr;
            memcpy(target_cmd->payload, (void *)full_ptr, cmd->payload_len);
          }
          atomic_store(&m2r->head, (head + 1) % capacity);
        }
      }
    } else if (cmd->type == 12 /* CMD_FETCH_REQUEST */) {
      uint64_t fetch_id = cmd->arg1;
      uint64_t full_ptr = cmd->payload_ptr;
      NSString *urlString =
          [[NSString alloc] initWithBytes:(const void *)full_ptr
                                   length:cmd->payload_len
                                 encoding:NSUTF8StringEncoding];
      NSLog(@"[Network] Renderer requested fetch via Native MacOS proxy: %@",
            urlString);

      NSURL *url = [NSURL URLWithString:urlString];
      if (url) {
        if (!url.scheme) {
          NSURL *baseURL = [NSURL URLWithString:@"https://google.com"];
          url = [NSURL URLWithString:urlString relativeToURL:baseURL];
        }
        NSMutableURLRequest *request = [NSMutableURLRequest requestWithURL:url];
        [request setValue:@"Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36" forHTTPHeaderField:@"User-Agent"];
        NSURLSessionDataTask *task = [[NSURLSession sharedSession]
              dataTaskWithRequest:request
            completionHandler:^(NSData *data, NSURLResponse *response,
                                NSError *error) {
              if (data && !error) {
                dispatch_async(dispatch_get_main_queue(), ^{
                  extern void sys_ipc_push_command(uint32_t cmd, uint64_t arg1,
                                                   uint32_t arg2);
                  extern uint64_t sys_ipc_get_bulk_ingress_ptr(void);

                  uint64_t bulk_ptr = sys_ipc_get_bulk_ingress_ptr();
                  if (bulk_ptr != 0) {
                    uint32_t fetch_len = data.length < (2097152 - 131072)
                                             ? (uint32_t)data.length
                                             : (2097152 - 131072);
                    if (fetch_len > 0 && data.bytes) {
                      memcpy((void *)bulk_ptr, data.bytes, fetch_len);
                    }

                    NSLog(@"[Network] Pushing CMD_FETCH_RESPONSE: "
                          @"fetch_id=0x%llx len=%u bulk_ptr=0x%llx",
                          (unsigned long long)fetch_id, fetch_len,
                          (unsigned long long)bulk_ptr);
                    sys_ipc_push_command(9 /* CMD_FETCH_RESPONSE */, fetch_id,
                                         fetch_len);
                  }

                  NSLog(@"[Network] Fetched %lu bytes natively into Bulk SHM "
                        @"for ID=%llu",
                        (unsigned long)data.length,
                        (unsigned long long)fetch_id);
                });
              } else {
                NSLog(@"[Network] Fetch failed: %@", error);
              }
            }];
        [task resume];
      }
    }
  }

  // ═══════════════════════════════════════════════════════════
  // Phase B: Poll each OOPIF child's R2M ring
  // ═══════════════════════════════════════════════════════════
  for (int i = 0; i < oopif_child_count; i++) {
    uint8_t *child_shm = oopif_child_shm_ptrs[i];
    if (!child_shm)
      continue;

    RingHeader *r2m = (RingHeader *)(child_shm + 65536);
    uint32_t head = atomic_load(&r2m->head);
    uint32_t tail = atomic_load(&r2m->tail);
    uint32_t capacity = (65536 - 8) / sizeof(ChildIPCCmd);

    while (tail != head) {
      ChildIPCCmd *child_r2m_cmds = (ChildIPCCmd *)(child_shm + 65536 + 8);
      ChildIPCCmd child_cmd = child_r2m_cmds[tail % capacity];

      if (child_cmd.type == 1) { // COMMAND_NEW_FRAME
        uint32_t iframe_surface_id = (uint32_t)child_cmd.arg1;
        uint32_t parent_node = oopif_child_node_ids[i];

        // Relay to parent renderer: CMD_IFRAME_SURFACE(6)
        // Note: renderer expects EVENT_IFRAME_SURFACE=6
        sys_ipc_push_command(6 /* EVENT_IFRAME_SURFACE */,
                             (uint64_t)parent_node, iframe_surface_id);

        NSLog(@"[OOPIF] Iframe PID=%d surface %u → parent node %u",
              oopif_child_pids[i], iframe_surface_id, parent_node);
      } else if (child_cmd.type == 4) { // CMD_POST_MESSAGE from child to parent
        // arg1 = 0 (parent context), payload = data
        // Relay to parent renderer's M2R as EVENT_POST_MESSAGE(5)
        uint64_t payload_addr = (uint64_t)child_cmd.payload;
        extern void sys_ipc_push_command_with_payload(
            uint32_t cmd_type, uint64_t arg1, uint64_t payload_ptr,
            uint32_t payload_len);
        sys_ipc_push_command_with_payload(5 /* EVENT_POST_MESSAGE */, 0,
                                          payload_addr, child_cmd.payload_len);
        NSLog(@"[OOPIF] Iframe PID=%d sent postMessage to parent",
              oopif_child_pids[i]);
      }

      tail = (tail + 1) % capacity;
      atomic_store(&r2m->tail, tail);
    }
  }

  // ═══════════════════════════════════════════════════════════
  // Phase B.5: Poll Service Workers for FETCH_RESPONSE
  // ═══════════════════════════════════════════════════════════
  for (int i = 0; i < sw_count; i++) {
    uint8_t *sw_shm = sw_shm_ptrs[i];
    if (!sw_shm)
      continue;

    RingHeader *r2m = (RingHeader *)(sw_shm + 131072);
    uint32_t head = atomic_load(&r2m->head);
    uint32_t tail = atomic_load(&r2m->tail);
    uint32_t capacity = (131072 - 8) / sizeof(ChildIPCCmd);

    while (tail != head) {
      ChildIPCCmd *sw_r2m_cmds = (ChildIPCCmd *)(sw_shm + 131072 + 8);
      ChildIPCCmd sw_cmd = sw_r2m_cmds[tail % capacity];

      if (sw_cmd.type == CMD_FETCH_RESPONSE) {
        // Relay back to Tab Renderer
        extern void sys_ipc_push_command_with_payload(
            uint32_t cmd_type, uint64_t arg1, uint64_t payload_ptr,
            uint32_t payload_len);
        sys_ipc_push_command_with_payload(9 /* CMD_FETCH_RESPONSE */,
                                          sw_cmd.arg1, (uint64_t)sw_cmd.payload,
                                          sw_cmd.payload_len);
        NSLog(@"[ServiceWorker] Routing fetch response for ID=%llu",
              sw_cmd.arg1);
      }
      tail = (tail + 1) % capacity;
      atomic_store(&r2m->tail, tail);
    }
  }

  // ═══════════════════════════════════════════════════════════
  // Phase C: Detect crashed children (Aw, Snap!)
  // ═══════════════════════════════════════════════════════════
  // Handle tab renderer crashes
  for (NSNumber *pidNum in self.tabPIDs) {
    int status;
    pid_t p = [pidNum intValue];
    if (waitpid(p, &status, WNOHANG) > 0) {
      if (WIFSIGNALED(status) || WEXITSTATUS(status) != 0) {
        NSLog(@"[Multi-Process] Child Renderer %d Crashed! Aw, Snap!", p);
        self.tabContentView.layer.contents = nil;
        self.tabContentView.layer.backgroundColor =
            [[NSColor redColor] CGColor];
        [self.tabPIDs removeObject:pidNum];
        break;
      }
    }
  }

  // Handle OOPIF child crashes
  for (int i = 0; i < oopif_child_count; i++) {
    int status;
    pid_t p = oopif_child_pids[i];
    if (p > 0 && waitpid(p, &status, WNOHANG) > 0) {
      if (WIFSIGNALED(status) || WEXITSTATUS(status) != 0) {
        NSLog(@"[OOPIF] Iframe renderer PID=%d crashed! node=%u", p,
              oopif_child_node_ids[i]);
        // Clean up: unmap, close shm, compact array
        if (oopif_child_shm_ptrs[i]) {
          munmap(oopif_child_shm_ptrs[i], 2097152);
          oopif_child_shm_ptrs[i] = NULL;
        }
        close(oopif_child_shm_fds[i]);
        oopif_child_pids[i] = 0;
        oopif_child_node_ids[i] = 0;
      }
    }
  }
}

- (void)registerServiceWorker:(NSString *)scriptUrl forTab:(uint32_t)tabId {
  if (sw_count >= MAX_OOPIF_CHILDREN)
    return;

  char shm_name[64];
  snprintf(shm_name, sizeof(shm_name), "/prisimi_sw_%u", tabId);
  shm_unlink(shm_name);
  int sw_shm_fd = shm_open(shm_name, O_CREAT | O_RDWR, 0666);
  if (sw_shm_fd < 0)
    return;
  ftruncate(sw_shm_fd, 262144); // 256KB for Worker IPC

  void *sw_shm_ptr =
      mmap(NULL, 262144, PROT_READ | PROT_WRITE, MAP_SHARED, sw_shm_fd, 0);
  if (sw_shm_ptr == MAP_FAILED) {
    close(sw_shm_fd);
    return;
  }
  memset(sw_shm_ptr, 0, 262144);

  pid_t pid = fork();
  if (pid == 0) {
    char fd_str[16];
    sprintf(fd_str, "%d", sw_shm_fd);
    // Execute the headless worker binary
    execl("/tmp/salt_build/prisimi_worker", "prisimi_worker", "--ipc-fd",
          fd_str, "--script", [scriptUrl UTF8String], NULL);
    _exit(1);
  } else {
    int slot = sw_count++;
    sw_shm_fds[slot] = sw_shm_fd;
    sw_shm_ptrs[slot] = (uint8_t *)sw_shm_ptr;
    sw_pids[slot] = pid;

    // Map the Worker FD to the Tab's Renderer Process if needed
    // For now, we relay everything through Main Process
    sys_ipc_push_command(CMD_WORKER_REGISTERED, (uint64_t)sw_shm_fd, 0);
    NSLog(@"[ServiceWorker] Registered for tab %u, PID %d", tabId, pid);
  }
}

- (void)spawnCDMSandboxForTab:(uint32_t)tabId {
  if (cdm_count >= MAX_OOPIF_CHILDREN)
    return;

  char shm_name[64];
  snprintf(shm_name, sizeof(shm_name), "/prisimi_cdm_%d", tabId);
  shm_unlink(shm_name);
  int cdm_shm_fd = shm_open(shm_name, O_CREAT | O_RDWR, 0666);
  if (cdm_shm_fd < 0)
    return;
  ftruncate(cdm_shm_fd, 16777216); // 16MB for DECRYPTION_ARENA & IPC

  void *cdm_shm_ptr =
      mmap(NULL, 16777216, PROT_READ | PROT_WRITE, MAP_SHARED, cdm_shm_fd, 0);
  if (cdm_shm_ptr == MAP_FAILED) {
    close(cdm_shm_fd);
    return;
  }
  memset(cdm_shm_ptr, 0, 16777216);

  pid_t pid = fork();
  if (pid == 0) {
    // --- ENTER MAXIMUM SECURITY SANDBOX ---
    extern int sandbox_init(const char *profile, uint64_t flags,
                            char **errorbuf);
    const char *sandbox_profile =
        "(version 1)"
        "(deny default)"
        "(allow file-read* (subpath "
        "\"/System/Library/Frameworks/CoreMedia.framework\"))"
        "(allow file-read* (subpath "
        "\"/System/Library/Frameworks/VideoToolbox.framework\"))"
        "(allow ipc-posix-shm* (ipc-posix-name-regex \"^prisimi_cdm_\"))";
    char *errorbuf;
    if (sandbox_init(sandbox_profile, 0, &errorbuf) != 0) {
      fprintf(stderr, "[CDM Sandbox] sandbox_init failed: %s\n", errorbuf);
      _exit(1);
    }

    char fd_str[16];
    sprintf(fd_str, "%d", cdm_shm_fd);
    execl("/tmp/salt_build/prisimi_cdm", "prisimi_cdm", "--ipc-fd", fd_str,
          NULL);
    _exit(1);
  } else {
    int slot = cdm_count++;
    cdm_shm_fds[slot] = cdm_shm_fd;
    cdm_shm_ptrs[slot] = (uint8_t *)cdm_shm_ptr;
    cdm_pids[slot] = pid;

    // Notify Renderer that CDM is ready and provide the SHM FD
    sys_ipc_push_command(CMD_CDM_REGISTERED, (uint64_t)cdm_shm_fd, 0);
    NSLog(@"[CDM Sandbox] Spawned PID %d for tab %u", pid, tabId);
  }
}

- (void)controlTextDidEndEditing:(NSNotification *)obj {
  NSString *url = self.omnibox.stringValue;
  const char *url_c = [url UTF8String];
  sys_ipc_push_command(1 /* NAVIGATE */, (uint64_t)url_c,
                       (uint32_t)strlen(url_c));
}

- (void)onBackButtonClicked {
  sys_ipc_push_command(3 /* GO_BACK */, 0, 0);
}

- (void)scrollWheel:(NSEvent *)event {
  int32_t dy = (int32_t)([event scrollingDeltaY] * -3.0);
  sys_ipc_push_command(6 /* CMD_SCROLL */, (uint64_t)(uint32_t)dy, 0);
}

@end

// FFI callback from Salt to update the Omnibox during SPA pushState or hard
// navigation
void ext_mac_update_omnibox(uint64_t ptr, uint32_t len) {
  if (!globalChrome)
    return;
  char *c_str = malloc(len + 1);
  memcpy(c_str, (void *)ptr, len);
  c_str[len] = '\0';
  NSString *new_url = [NSString stringWithUTF8String:c_str];
  free(c_str);
  dispatch_async(dispatch_get_main_queue(), ^{
    [globalChrome.omnibox setStringValue:new_url];
  });
}

int main(int argc, const char *argv[]) {
  @autoreleasepool {
    // Setup simple app environment
    NSApplication *app = [NSApplication sharedApplication];
    [app setActivationPolicy:NSApplicationActivationPolicyRegular];

    // Create an initial window to hold our BrowserChrome
    NSRect frame = NSMakeRect(0, 0, 800, 600);
    NSWindow *window =
        [[NSWindow alloc] initWithContentRect:frame
                                    styleMask:(NSWindowStyleMaskTitled |
                                               NSWindowStyleMaskClosable |
                                               NSWindowStyleMaskResizable |
                                               NSWindowStyleMaskMiniaturizable)
                                      backing:NSBackingStoreBuffered
                                        defer:NO];
    [window cascadeTopLeftFromPoint:NSMakePoint(20, 20)];
    [window setTitle:@"Prisimi Sovereign Multiverse"];
    [window makeKeyAndOrderFront:nil];

    // Initialize Browser Chrome with the window
    BrowserChrome *chrome = [[BrowserChrome alloc] initWithWindow:window];
    [app activateIgnoringOtherApps:YES];

    // Instead of NSApplicationMain, we can run the runloop or just use NSApp
    // run
  }
  return NSApplicationMain(argc, argv);
}
