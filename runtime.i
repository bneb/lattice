# 1 "/Users/kevin/projects/lattice/salt-front/runtime.c"
# 1 "<built-in>" 1
# 1 "<built-in>" 3
# 469 "<built-in>" 3
# 1 "<command line>" 1
# 1 "<built-in>" 2
# 1 "/Users/kevin/projects/lattice/salt-front/runtime.c" 2
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/fcntl.h" 1 3 4
# 23 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/fcntl.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/fcntl.h" 1 3 4
# 78 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/fcntl.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types.h" 1 3 4
# 32 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/cdefs.h" 1 3 4
# 782 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/cdefs.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_symbol_aliasing.h" 1 3 4
# 783 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/cdefs.h" 2 3 4
# 848 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/cdefs.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_posix_availability.h" 1 3 4
# 849 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/cdefs.h" 2 3 4
# 33 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/machine/_types.h" 1 3 4
# 34 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/machine/_types.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/arm/_types.h" 1 3 4
# 28 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/arm/_types.h" 3 4
typedef signed char __int8_t;



typedef unsigned char __uint8_t;
typedef short __int16_t;
typedef unsigned short __uint16_t;
typedef int __int32_t;
typedef unsigned int __uint32_t;
typedef long long __int64_t;
typedef unsigned long long __uint64_t;

typedef long __darwin_intptr_t;
typedef unsigned int __darwin_natural_t;
# 61 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/arm/_types.h" 3 4
typedef int __darwin_ct_rune_t;





typedef union {
 char __mbstate8[128];
 long long _mbstateL;
} __mbstate_t;

typedef __mbstate_t __darwin_mbstate_t;




typedef long int __darwin_ptrdiff_t;
# 87 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/arm/_types.h" 3 4
typedef long unsigned int __darwin_size_t;







typedef __builtin_va_list __darwin_va_list;







typedef int __darwin_wchar_t;




typedef __darwin_wchar_t __darwin_rune_t;


typedef int __darwin_wint_t;




typedef unsigned long __darwin_clock_t;
typedef __uint32_t __darwin_socklen_t;
typedef long __darwin_ssize_t;
typedef long __darwin_time_t;
# 35 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/machine/_types.h" 2 3 4
# 34 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types.h" 2 3 4
# 67 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types.h" 3 4
typedef __int64_t __darwin_blkcnt_t;
typedef __int32_t __darwin_blksize_t;
typedef __int32_t __darwin_dev_t;
typedef unsigned int __darwin_fsblkcnt_t;
typedef unsigned int __darwin_fsfilcnt_t;
typedef __uint32_t __darwin_gid_t;
typedef __uint32_t __darwin_id_t;
typedef __uint64_t __darwin_ino64_t;

typedef __darwin_ino64_t __darwin_ino_t;



typedef __darwin_natural_t __darwin_mach_port_name_t;
typedef __darwin_mach_port_name_t __darwin_mach_port_t;
typedef __uint16_t __darwin_mode_t;
typedef __int64_t __darwin_off_t;
typedef __int32_t __darwin_pid_t;
typedef __uint32_t __darwin_sigset_t;
typedef __int32_t __darwin_suseconds_t;
typedef __uint32_t __darwin_uid_t;
typedef __uint32_t __darwin_useconds_t;
typedef unsigned char __darwin_uuid_t[16];
typedef char __darwin_uuid_string_t[37];



# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_pthread/_pthread_types.h" 1 3 4
# 57 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_pthread/_pthread_types.h" 3 4
struct __darwin_pthread_handler_rec {
 void (*__routine)(void *);
 void *__arg;
 struct __darwin_pthread_handler_rec *__next;
};

struct _opaque_pthread_attr_t {
 long __sig;
 char __opaque[56];
};

struct _opaque_pthread_cond_t {
 long __sig;
 char __opaque[40];
};

struct _opaque_pthread_condattr_t {
 long __sig;
 char __opaque[8];
};

struct _opaque_pthread_mutex_t {
 long __sig;
 char __opaque[56];
};

struct _opaque_pthread_mutexattr_t {
 long __sig;
 char __opaque[8];
};

struct _opaque_pthread_once_t {
 long __sig;
 char __opaque[8];
};

struct _opaque_pthread_rwlock_t {
 long __sig;
 char __opaque[192];
};

struct _opaque_pthread_rwlockattr_t {
 long __sig;
 char __opaque[16];
};

struct _opaque_pthread_t {
 long __sig;
 struct __darwin_pthread_handler_rec *__cleanup_stack;
 char __opaque[8176];
};

typedef struct _opaque_pthread_attr_t __darwin_pthread_attr_t;
typedef struct _opaque_pthread_cond_t __darwin_pthread_cond_t;
typedef struct _opaque_pthread_condattr_t __darwin_pthread_condattr_t;
typedef unsigned long __darwin_pthread_key_t;
typedef struct _opaque_pthread_mutex_t __darwin_pthread_mutex_t;
typedef struct _opaque_pthread_mutexattr_t __darwin_pthread_mutexattr_t;
typedef struct _opaque_pthread_once_t __darwin_pthread_once_t;
typedef struct _opaque_pthread_rwlock_t __darwin_pthread_rwlock_t;
typedef struct _opaque_pthread_rwlockattr_t __darwin_pthread_rwlockattr_t;
typedef struct _opaque_pthread_t *__darwin_pthread_t;
# 95 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types.h" 2 3 4
# 79 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/fcntl.h" 2 3 4

# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/Availability.h" 1 3 4
# 196 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/Availability.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/AvailabilityVersions.h" 1 3 4
# 197 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/Availability.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/AvailabilityInternal.h" 1 3 4
# 33 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/AvailabilityInternal.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/AvailabilityVersions.h" 1 3 4
# 34 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/AvailabilityInternal.h" 2 3 4
# 198 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/Availability.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/AvailabilityInternalLegacy.h" 1 3 4
# 34 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/AvailabilityInternalLegacy.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/AvailabilityInternal.h" 1 3 4
# 35 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/AvailabilityInternalLegacy.h" 2 3 4
# 199 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/Availability.h" 2 3 4
# 81 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/fcntl.h" 2 3 4


# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_size_t.h" 1 3 4
# 50 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_size_t.h" 3 4
typedef __darwin_size_t size_t;
# 84 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/fcntl.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_mode_t.h" 1 3 4
# 31 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_mode_t.h" 3 4
typedef __darwin_mode_t mode_t;
# 85 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/fcntl.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_off_t.h" 1 3 4
# 31 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_off_t.h" 3 4
typedef __darwin_off_t off_t;
# 86 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/fcntl.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_pid_t.h" 1 3 4
# 31 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_pid_t.h" 3 4
typedef __darwin_pid_t pid_t;
# 87 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/fcntl.h" 2 3 4
# 116 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/fcntl.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_o_sync.h" 1 3 4
# 117 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/fcntl.h" 2 3 4
# 148 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/fcntl.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_o_dsync.h" 1 3 4
# 149 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/fcntl.h" 2 3 4
# 367 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/fcntl.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_seek_set.h" 1 3 4
# 368 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/fcntl.h" 2 3 4





# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_s_ifmt.h" 1 3 4
# 374 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/fcntl.h" 2 3 4
# 393 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/fcntl.h" 3 4
struct flock {
 off_t l_start;
 off_t l_len;
 pid_t l_pid;
 short l_type;
 short l_whence;
};

# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_timespec.h" 1 3 4
# 31 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_timespec.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/machine/types.h" 1 3 4
# 37 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/machine/types.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/arm/types.h" 1 3 4
# 55 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/arm/types.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_int8_t.h" 1 3 4
# 30 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_int8_t.h" 3 4
typedef signed char int8_t;
# 56 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/arm/types.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_int16_t.h" 1 3 4
# 30 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_int16_t.h" 3 4
typedef short int16_t;
# 57 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/arm/types.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_int32_t.h" 1 3 4
# 30 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_int32_t.h" 3 4
typedef int int32_t;
# 58 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/arm/types.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_int64_t.h" 1 3 4
# 30 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_int64_t.h" 3 4
typedef long long int64_t;
# 59 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/arm/types.h" 2 3 4

# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_u_int8_t.h" 1 3 4
# 30 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_u_int8_t.h" 3 4
typedef unsigned char u_int8_t;
# 61 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/arm/types.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_u_int16_t.h" 1 3 4
# 30 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_u_int16_t.h" 3 4
typedef unsigned short u_int16_t;
# 62 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/arm/types.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_u_int32_t.h" 1 3 4
# 30 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_u_int32_t.h" 3 4
typedef unsigned int u_int32_t;
# 63 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/arm/types.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_u_int64_t.h" 1 3 4
# 30 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_u_int64_t.h" 3 4
typedef unsigned long long u_int64_t;
# 64 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/arm/types.h" 2 3 4


typedef int64_t register_t;




# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_intptr_t.h" 1 3 4
# 32 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_intptr_t.h" 3 4
typedef __darwin_intptr_t intptr_t;
# 72 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/arm/types.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_uintptr_t.h" 1 3 4
# 34 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_uintptr_t.h" 3 4
typedef unsigned long uintptr_t;
# 73 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/arm/types.h" 2 3 4




typedef u_int64_t user_addr_t;
typedef u_int64_t user_size_t;
typedef int64_t user_ssize_t;
typedef int64_t user_long_t;
typedef u_int64_t user_ulong_t;
typedef int64_t user_time_t;
typedef int64_t user_off_t;
# 105 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/arm/types.h" 3 4
typedef u_int64_t syscall_arg_t;
# 38 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/machine/types.h" 2 3 4
# 32 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_timespec.h" 2 3 4

struct timespec
{
 __darwin_time_t tv_sec;
 long tv_nsec;
};
# 402 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/fcntl.h" 2 3 4






struct flocktimeout {
 struct flock fl;
 struct timespec timeout;
};
# 421 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/fcntl.h" 3 4
struct radvisory {
 off_t ra_offset;
 int ra_count;
};
# 434 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/fcntl.h" 3 4
typedef struct fsignatures {
 off_t fs_file_start;
 void *fs_blob_start;
 size_t fs_blob_size;



 size_t fs_fsignatures_size;
 char fs_cdhash[20];
 int fs_hash_type;
} fsignatures_t;

typedef struct fsupplement {
 off_t fs_file_start;
 off_t fs_blob_start;
 size_t fs_blob_size;
 int fs_orig_fd;
} fsupplement_t;
# 465 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/fcntl.h" 3 4
typedef struct fchecklv {
 off_t lv_file_start;
 size_t lv_error_message_size;
 void *lv_error_message;
} fchecklv_t;
# 479 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/fcntl.h" 3 4
typedef struct fgetsigsinfo {
 off_t fg_file_start;
 int fg_info_request;
 int fg_sig_is_platform;
} fgetsigsinfo_t;
# 494 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/fcntl.h" 3 4
typedef struct fstore {
 unsigned int fst_flags;
 int fst_posmode;
 off_t fst_offset;
 off_t fst_length;
 off_t fst_bytesalloc;
} fstore_t;


typedef struct fpunchhole {
 unsigned int fp_flags;
 unsigned int reserved;
 off_t fp_offset;
 off_t fp_length;
} fpunchhole_t;


typedef struct ftrimactivefile {
 off_t fta_offset;
 off_t fta_length;
} ftrimactivefile_t;


typedef struct fspecread {
 unsigned int fsr_flags;
 unsigned int reserved;
 off_t fsr_offset;
 off_t fsr_length;
} fspecread_t;




typedef struct fattributiontag {
 unsigned int ft_flags;
 unsigned long long ft_hash;
 char ft_attribution_name[255];
} fattributiontag_t;
# 559 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/fcntl.h" 3 4
#pragma pack(4)

struct log2phys {
 unsigned int l2p_flags;
 off_t l2p_contigbytes;


 off_t l2p_devoffset;


};

#pragma pack()
# 582 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/fcntl.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_filesec_t.h" 1 3 4
# 30 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_filesec_t.h" 3 4
struct _filesec;
typedef struct _filesec *filesec_t;
# 583 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/fcntl.h" 2 3 4

typedef enum {
 FILESEC_OWNER = 1,
 FILESEC_GROUP = 2,
 FILESEC_UUID = 3,
 FILESEC_MODE = 4,
 FILESEC_ACL = 5,
 FILESEC_GRPUUID = 6,


 FILESEC_ACL_RAW = 100,
 FILESEC_ACL_ALLOCSIZE = 101
} filesec_property_t;






int open(const char *, int, ...) __asm("_" "open" );

int openat(int, const char *, int, ...) __asm("_" "openat" ) __attribute__((availability(macosx,introduced=10.10)));

int creat(const char *, mode_t) __asm("_" "creat" );
int fcntl(int, int, ...) __asm("_" "fcntl" );


int openx_np(const char *, int, filesec_t);




int open_dprotected_np( const char *, int, int, int, ...);
int openat_dprotected_np( int, const char *, int, int, int, ...);
int openat_authenticated_np(int, const char *, int, int);
int flock(int, int);
filesec_t filesec_init(void);
filesec_t filesec_dup(filesec_t);
void filesec_free(filesec_t);
int filesec_get_property(filesec_t, filesec_property_t, void *);
int filesec_query_property(filesec_t, filesec_property_t, int *);
int filesec_set_property(filesec_t, filesec_property_t, const void *);
int filesec_unset_property(filesec_t, filesec_property_t) __attribute__((availability(macosx,introduced=10.6)));
# 24 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/fcntl.h" 2 3 4
# 2 "/Users/kevin/projects/lattice/salt-front/runtime.c" 2

# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/crt_externs.h" 1 3 4
# 35 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/crt_externs.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_bounds.h" 1 3 4
# 36 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/crt_externs.h" 2 3 4




extern char * * *_NSGetArgv(void);
extern int *_NSGetArgc(void);
extern char * * *_NSGetEnviron(void);
extern char * *_NSGetProgname(void);

extern struct mach_header_64 *



    _NSGetMachExecuteHeader(void);
# 4 "/Users/kevin/projects/lattice/salt-front/runtime.c" 2
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_time.h" 1 3 4
# 32 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_time.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_types.h" 1 3 4
# 76 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_types.h" 3 4
# 1 "/opt/homebrew/Cellar/llvm/21.1.8/lib/clang/21/include/stdint.h" 1 3 4
# 56 "/opt/homebrew/Cellar/llvm/21.1.8/lib/clang/21/include/stdint.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/stdint.h" 1 3 4
# 23 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/stdint.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_types/_uint8_t.h" 1 3 4
# 31 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_types/_uint8_t.h" 3 4
typedef unsigned char uint8_t;
# 24 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/stdint.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_types/_uint16_t.h" 1 3 4
# 31 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_types/_uint16_t.h" 3 4
typedef unsigned short uint16_t;
# 25 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/stdint.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_types/_uint32_t.h" 1 3 4
# 31 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_types/_uint32_t.h" 3 4
typedef unsigned int uint32_t;
# 26 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/stdint.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_types/_uint64_t.h" 1 3 4
# 31 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_types/_uint64_t.h" 3 4
typedef unsigned long long uint64_t;
# 27 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/stdint.h" 2 3 4


typedef int8_t int_least8_t;
typedef int16_t int_least16_t;
typedef int32_t int_least32_t;
typedef int64_t int_least64_t;
typedef uint8_t uint_least8_t;
typedef uint16_t uint_least16_t;
typedef uint32_t uint_least32_t;
typedef uint64_t uint_least64_t;



typedef int8_t int_fast8_t;
typedef int16_t int_fast16_t;
typedef int32_t int_fast32_t;
typedef int64_t int_fast64_t;
typedef uint8_t uint_fast8_t;
typedef uint16_t uint_fast16_t;
typedef uint32_t uint_fast32_t;
typedef uint64_t uint_fast64_t;
# 58 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/stdint.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_types/_intmax_t.h" 1 3 4
# 32 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_types/_intmax_t.h" 3 4
typedef long int intmax_t;
# 59 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/stdint.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_types/_uintmax_t.h" 1 3 4
# 32 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_types/_uintmax_t.h" 3 4
typedef long unsigned int uintmax_t;
# 60 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/stdint.h" 2 3 4
# 57 "/opt/homebrew/Cellar/llvm/21.1.8/lib/clang/21/include/stdint.h" 2 3 4
# 77 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_types.h" 2 3 4



# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/host_info.h" 1 3 4
# 66 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/host_info.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/message.h" 1 3 4
# 74 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/message.h" 3 4
# 1 "/opt/homebrew/Cellar/llvm/21.1.8/lib/clang/21/include/stddef.h" 1 3 4
# 84 "/opt/homebrew/Cellar/llvm/21.1.8/lib/clang/21/include/stddef.h" 3 4
# 1 "/opt/homebrew/Cellar/llvm/21.1.8/lib/clang/21/include/__stddef_header_macro.h" 1 3 4
# 85 "/opt/homebrew/Cellar/llvm/21.1.8/lib/clang/21/include/stddef.h" 2 3 4



# 1 "/opt/homebrew/Cellar/llvm/21.1.8/lib/clang/21/include/__stddef_ptrdiff_t.h" 1 3 4
# 18 "/opt/homebrew/Cellar/llvm/21.1.8/lib/clang/21/include/__stddef_ptrdiff_t.h" 3 4
typedef long int ptrdiff_t;
# 89 "/opt/homebrew/Cellar/llvm/21.1.8/lib/clang/21/include/stddef.h" 2 3 4




# 1 "/opt/homebrew/Cellar/llvm/21.1.8/lib/clang/21/include/__stddef_size_t.h" 1 3 4
# 94 "/opt/homebrew/Cellar/llvm/21.1.8/lib/clang/21/include/stddef.h" 2 3 4




# 1 "/opt/homebrew/Cellar/llvm/21.1.8/lib/clang/21/include/__stddef_rsize_t.h" 1 3 4
# 18 "/opt/homebrew/Cellar/llvm/21.1.8/lib/clang/21/include/__stddef_rsize_t.h" 3 4
typedef long unsigned int rsize_t;
# 99 "/opt/homebrew/Cellar/llvm/21.1.8/lib/clang/21/include/stddef.h" 2 3 4




# 1 "/opt/homebrew/Cellar/llvm/21.1.8/lib/clang/21/include/__stddef_wchar_t.h" 1 3 4
# 24 "/opt/homebrew/Cellar/llvm/21.1.8/lib/clang/21/include/__stddef_wchar_t.h" 3 4
typedef int wchar_t;
# 104 "/opt/homebrew/Cellar/llvm/21.1.8/lib/clang/21/include/stddef.h" 2 3 4




# 1 "/opt/homebrew/Cellar/llvm/21.1.8/lib/clang/21/include/__stddef_null.h" 1 3 4
# 109 "/opt/homebrew/Cellar/llvm/21.1.8/lib/clang/21/include/stddef.h" 2 3 4
# 123 "/opt/homebrew/Cellar/llvm/21.1.8/lib/clang/21/include/stddef.h" 3 4
# 1 "/opt/homebrew/Cellar/llvm/21.1.8/lib/clang/21/include/__stddef_max_align_t.h" 1 3 4
# 16 "/opt/homebrew/Cellar/llvm/21.1.8/lib/clang/21/include/__stddef_max_align_t.h" 3 4
typedef long double max_align_t;
# 124 "/opt/homebrew/Cellar/llvm/21.1.8/lib/clang/21/include/stddef.h" 2 3 4




# 1 "/opt/homebrew/Cellar/llvm/21.1.8/lib/clang/21/include/__stddef_offsetof.h" 1 3 4
# 129 "/opt/homebrew/Cellar/llvm/21.1.8/lib/clang/21/include/stddef.h" 2 3 4
# 75 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/message.h" 2 3 4

# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/machine/limits.h" 1 3 4
# 11 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/machine/limits.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/arm/limits.h" 1 3 4
# 45 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/arm/limits.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/arm/_limits.h" 1 3 4
# 46 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/arm/limits.h" 2 3 4
# 12 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/machine/limits.h" 2 3 4
# 77 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/message.h" 2 3 4

# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/port.h" 1 3 4
# 91 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/port.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/boolean.h" 1 3 4
# 73 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/boolean.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/machine/boolean.h" 1 3 4
# 35 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/machine/boolean.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/arm/boolean.h" 1 3 4
# 70 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/arm/boolean.h" 3 4
typedef int boolean_t;
# 36 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/machine/boolean.h" 2 3 4
# 74 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/boolean.h" 2 3 4
# 92 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/port.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/machine/vm_types.h" 1 3 4
# 35 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/machine/vm_types.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/arm/vm_types.h" 1 3 4
# 76 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/arm/vm_types.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/Availability.h" 1 3 4
# 77 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/arm/vm_types.h" 2 3 4
# 96 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/arm/vm_types.h" 3 4
typedef __darwin_natural_t natural_t;
typedef int integer_t;






typedef uintptr_t vm_offset_t ;
typedef uintptr_t vm_size_t;

typedef uint64_t mach_vm_address_t ;
typedef uint64_t mach_vm_offset_t ;
typedef uint64_t mach_vm_size_t;

typedef uint64_t vm_map_offset_t ;
typedef uint64_t vm_map_address_t ;
typedef uint64_t vm_map_size_t;
# 146 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/arm/vm_types.h" 3 4
typedef uint32_t vm32_offset_t;
typedef uint32_t vm32_address_t;
typedef uint32_t vm32_size_t;

typedef vm_offset_t mach_port_context_t;
# 36 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/machine/vm_types.h" 2 3 4
# 93 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/port.h" 2 3 4
# 106 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/port.h" 3 4
typedef natural_t mach_port_name_t;
typedef mach_port_name_t *mach_port_name_array_t;
# 127 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/port.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_mach_port_t.h" 1 3 4
# 50 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_mach_port_t.h" 3 4
typedef __darwin_mach_port_t mach_port_t;
# 128 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/port.h" 2 3 4


typedef mach_port_t *mach_port_array_t;
# 188 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/port.h" 3 4
typedef natural_t mach_port_right_t;
# 202 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/port.h" 3 4
typedef natural_t mach_port_type_t;
typedef mach_port_type_t *mach_port_type_array_t;
# 232 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/port.h" 3 4
typedef natural_t mach_port_urefs_t;
typedef integer_t mach_port_delta_t;



typedef natural_t mach_port_seqno_t;
typedef natural_t mach_port_mscount_t;
typedef natural_t mach_port_msgcount_t;
typedef natural_t mach_port_rights_t;






typedef unsigned int mach_port_srights_t;

typedef struct mach_port_status {
 mach_port_rights_t mps_pset;
 mach_port_seqno_t mps_seqno;
 mach_port_mscount_t mps_mscount;
 mach_port_msgcount_t mps_qlimit;
 mach_port_msgcount_t mps_msgcount;
 mach_port_rights_t mps_sorights;
 boolean_t mps_srights;
 boolean_t mps_pdrequest;
 boolean_t mps_nsrequest;
 natural_t mps_flags;
} mach_port_status_t;
# 272 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/port.h" 3 4
typedef struct mach_port_limits {
 mach_port_msgcount_t mpl_qlimit;
} mach_port_limits_t;
# 286 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/port.h" 3 4
typedef struct mach_port_info_ext {
 mach_port_status_t mpie_status;
 mach_port_msgcount_t mpie_boost_cnt;
 uint32_t reserved[6];
} mach_port_info_ext_t;

typedef struct mach_port_guard_info {
 uint64_t mpgi_guard;
} mach_port_guard_info_t;

typedef integer_t *mach_port_info_t;


typedef int mach_port_flavor_t;
# 325 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/port.h" 3 4
typedef struct mach_port_qos {
 unsigned int name:1;
 unsigned int prealloc:1;
 boolean_t pad1:30;
 natural_t len;
} mach_port_qos_t;






typedef struct mach_service_port_info {
 char mspi_string_name[255];
 uint8_t mspi_domain_type;
} mach_service_port_info_data_t;




typedef struct mach_service_port_info * mach_service_port_info_t;
# 375 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/port.h" 3 4
typedef struct mach_port_options {
 uint32_t flags;
 mach_port_limits_t mpl;
 union {
  uint64_t reserved[2];
  mach_port_name_t work_interval_port;
  mach_service_port_info_t service_port_info;
  mach_port_name_t service_port_name;
 };
}mach_port_options_t;

typedef mach_port_options_t *mach_port_options_ptr_t;
# 407 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/port.h" 3 4
enum mach_port_guard_exception_codes {
 kGUARD_EXC_DESTROY = 1,
 kGUARD_EXC_MOD_REFS = 2,
 kGUARD_EXC_INVALID_OPTIONS = 3,
 kGUARD_EXC_SET_CONTEXT = 4,
 kGUARD_EXC_THREAD_SET_STATE = 5,
 kGUARD_EXC_EXCEPTION_BEHAVIOR_ENFORCE = 6,
 kGUARD_EXC_SERVICE_PORT_VIOLATION_FATAL = 7,
 kGUARD_EXC_UNGUARDED = 8,
 kGUARD_EXC_INCORRECT_GUARD = 16,
 kGUARD_EXC_IMMOVABLE = 32,
 kGUARD_EXC_STRICT_REPLY = 64,
 kGUARD_EXC_MSG_FILTERED = 128,

 kGUARD_EXC_INVALID_RIGHT = 256,
 kGUARD_EXC_INVALID_NAME = 512,
 kGUARD_EXC_INVALID_VALUE = 1u << 10,
 kGUARD_EXC_INVALID_ARGUMENT = 1u << 11,
 kGUARD_EXC_RIGHT_EXISTS = 1u << 12,
 kGUARD_EXC_KERN_NO_SPACE = 1u << 13,
 kGUARD_EXC_KERN_FAILURE = 1u << 14,
 kGUARD_EXC_KERN_RESOURCE = 1u << 15,
 kGUARD_EXC_SEND_INVALID_REPLY = 1u << 16,
 kGUARD_EXC_SEND_INVALID_VOUCHER = 1u << 17,
 kGUARD_EXC_SEND_INVALID_RIGHT = 1u << 18,
 kGUARD_EXC_RCV_INVALID_NAME = 1u << 19,

 kGUARD_EXC_RCV_GUARDED_DESC = 0x00100000,
 kGUARD_EXC_SERVICE_PORT_VIOLATION_NON_FATAL = 0x00100001,
 kGUARD_EXC_PROVISIONAL_REPLY_PORT = 0x00100002,
 kGUARD_EXC_MOD_REFS_NON_FATAL = 1u << 21,
 kGUARD_EXC_IMMOVABLE_NON_FATAL = 1u << 22,
 kGUARD_EXC_REQUIRE_REPLY_PORT_SEMANTICS = 1u << 23,
};
# 79 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/message.h" 2 3 4

# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/kern_return.h" 1 3 4
# 70 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/kern_return.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/machine/kern_return.h" 1 3 4
# 35 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/machine/kern_return.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/arm/kern_return.h" 1 3 4
# 73 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/arm/kern_return.h" 3 4
typedef int kern_return_t;
# 36 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/machine/kern_return.h" 2 3 4
# 71 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/kern_return.h" 2 3 4
# 81 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/message.h" 2 3 4



# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/appleapiopts.h" 1 3 4
# 85 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/message.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/Availability.h" 1 3 4
# 86 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/message.h" 2 3 4
# 97 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/message.h" 3 4
typedef natural_t mach_msg_timeout_t;
# 227 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/message.h" 3 4
typedef unsigned int mach_msg_bits_t;
typedef natural_t mach_msg_size_t;
typedef integer_t mach_msg_id_t;



typedef unsigned int mach_msg_priority_t;




typedef unsigned int mach_msg_type_name_t;
# 251 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/message.h" 3 4
typedef unsigned int mach_msg_copy_options_t;
# 265 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/message.h" 3 4
typedef unsigned int mach_msg_guard_flags_t;
# 279 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/message.h" 3 4
typedef unsigned int mach_msg_descriptor_type_t;
# 291 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/message.h" 3 4
#pragma pack(push, 4)

typedef struct {
 natural_t pad1;
 mach_msg_size_t pad2;
 unsigned int pad3 : 24;
 mach_msg_descriptor_type_t type : 8;
} mach_msg_type_descriptor_t;

typedef struct {
 mach_port_t name;
 mach_msg_size_t pad1;
 unsigned int pad2 : 16;
 mach_msg_type_name_t disposition : 8;
 mach_msg_descriptor_type_t type : 8;
} mach_msg_port_descriptor_t;


typedef struct {
 uint32_t address;
 mach_msg_size_t size;
 boolean_t deallocate: 8;
 mach_msg_copy_options_t copy: 8;
 unsigned int pad1: 8;
 mach_msg_descriptor_type_t type: 8;
} mach_msg_ool_descriptor32_t;

typedef struct {
 uint64_t address;
 boolean_t deallocate: 8;
 mach_msg_copy_options_t copy: 8;
 unsigned int pad1: 8;
 mach_msg_descriptor_type_t type: 8;
 mach_msg_size_t size;
} mach_msg_ool_descriptor64_t;

typedef struct {
 void *address;



 boolean_t deallocate: 8;
 mach_msg_copy_options_t copy: 8;
 unsigned int pad1: 8;
 mach_msg_descriptor_type_t type: 8;

 mach_msg_size_t size;

} mach_msg_ool_descriptor_t;

typedef struct {
 uint32_t address;
 mach_msg_size_t count;
 boolean_t deallocate: 8;
 mach_msg_copy_options_t copy: 8;
 mach_msg_type_name_t disposition : 8;
 mach_msg_descriptor_type_t type : 8;
} mach_msg_ool_ports_descriptor32_t;

typedef struct {
 uint64_t address;
 boolean_t deallocate: 8;
 mach_msg_copy_options_t copy: 8;
 mach_msg_type_name_t disposition : 8;
 mach_msg_descriptor_type_t type : 8;
 mach_msg_size_t count;
} mach_msg_ool_ports_descriptor64_t;

typedef struct {
 void *address;



 boolean_t deallocate: 8;
 mach_msg_copy_options_t copy: 8;
 mach_msg_type_name_t disposition : 8;
 mach_msg_descriptor_type_t type : 8;

 mach_msg_size_t count;

} mach_msg_ool_ports_descriptor_t;

typedef struct {
 uint32_t context;
 mach_port_name_t name;
 mach_msg_guard_flags_t flags : 16;
 mach_msg_type_name_t disposition : 8;
 mach_msg_descriptor_type_t type : 8;
} mach_msg_guarded_port_descriptor32_t;

typedef struct {
 uint64_t context;
 mach_msg_guard_flags_t flags : 16;
 mach_msg_type_name_t disposition : 8;
 mach_msg_descriptor_type_t type : 8;
 mach_port_name_t name;
} mach_msg_guarded_port_descriptor64_t;

typedef struct {
 mach_port_context_t context;



 mach_msg_guard_flags_t flags : 16;
 mach_msg_type_name_t disposition : 8;
 mach_msg_descriptor_type_t type : 8;

 mach_port_name_t name;

} mach_msg_guarded_port_descriptor_t;






typedef union {
 mach_msg_port_descriptor_t port;
 mach_msg_ool_descriptor_t out_of_line;
 mach_msg_ool_ports_descriptor_t ool_ports;
 mach_msg_type_descriptor_t type;
 mach_msg_guarded_port_descriptor_t guarded_port;
} mach_msg_descriptor_t;

typedef struct {
 mach_msg_size_t msgh_descriptor_count;
} mach_msg_body_t;




typedef struct {
 mach_msg_bits_t msgh_bits;
 mach_msg_size_t msgh_size;
 mach_port_t msgh_remote_port;
 mach_port_t msgh_local_port;
 mach_port_name_t msgh_voucher_port;
 mach_msg_id_t msgh_id;
} mach_msg_header_t;





typedef struct {
 mach_msg_header_t header;
 mach_msg_body_t body;
} mach_msg_base_t;


typedef unsigned int mach_msg_trailer_type_t;



typedef unsigned int mach_msg_trailer_size_t;
typedef char *mach_msg_trailer_info_t;

typedef struct {
 mach_msg_trailer_type_t msgh_trailer_type;
 mach_msg_trailer_size_t msgh_trailer_size;
} mach_msg_trailer_t;
# 462 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/message.h" 3 4
typedef struct {
 mach_msg_trailer_type_t msgh_trailer_type;
 mach_msg_trailer_size_t msgh_trailer_size;
 mach_port_seqno_t msgh_seqno;
} mach_msg_seqno_trailer_t;

typedef struct {
 unsigned int val[2];
} security_token_t;

typedef struct {
 mach_msg_trailer_type_t msgh_trailer_type;
 mach_msg_trailer_size_t msgh_trailer_size;
 mach_port_seqno_t msgh_seqno;
 security_token_t msgh_sender;
} mach_msg_security_trailer_t;
# 488 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/message.h" 3 4
typedef struct {
 unsigned int val[8];
} audit_token_t;
# 508 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/message.h" 3 4
typedef struct {
 mach_msg_trailer_type_t msgh_trailer_type;
 mach_msg_trailer_size_t msgh_trailer_size;
 mach_port_seqno_t msgh_seqno;
 security_token_t msgh_sender;
 audit_token_t msgh_audit;
} mach_msg_audit_trailer_t;

typedef struct {
 mach_msg_trailer_type_t msgh_trailer_type;
 mach_msg_trailer_size_t msgh_trailer_size;
 mach_port_seqno_t msgh_seqno;
 security_token_t msgh_sender;
 audit_token_t msgh_audit;
 mach_port_context_t msgh_context;
} mach_msg_context_trailer_t;



typedef struct {
 mach_port_name_t sender;
} msg_labels_t;

typedef int mach_msg_filter_id;







typedef struct {
 mach_msg_trailer_type_t msgh_trailer_type;
 mach_msg_trailer_size_t msgh_trailer_size;
 mach_port_seqno_t msgh_seqno;
 security_token_t msgh_sender;
 audit_token_t msgh_audit;
 mach_port_context_t msgh_context;
 mach_msg_filter_id msgh_ad;
 msg_labels_t msgh_labels;
} mach_msg_mac_trailer_t;
# 563 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/message.h" 3 4
typedef mach_msg_mac_trailer_t mach_msg_max_trailer_t;
# 573 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/message.h" 3 4
typedef mach_msg_security_trailer_t mach_msg_format_0_trailer_t;







extern const security_token_t KERNEL_SECURITY_TOKEN;


extern const audit_token_t KERNEL_AUDIT_TOKEN;

typedef integer_t mach_msg_options_t;



typedef struct {
 mach_msg_header_t header;
} mach_msg_empty_send_t;

typedef struct {
 mach_msg_header_t header;
 mach_msg_trailer_t trailer;
} mach_msg_empty_rcv_t;

typedef union{
 mach_msg_empty_send_t send;
 mach_msg_empty_rcv_t rcv;
} mach_msg_empty_t;

#pragma pack(pop)
# 636 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/message.h" 3 4
typedef natural_t mach_msg_type_size_t;
typedef natural_t mach_msg_type_number_t;
# 684 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/message.h" 3 4
typedef integer_t mach_msg_option_t;
# 787 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/message.h" 3 4
typedef kern_return_t mach_msg_return_t;
# 907 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/message.h" 3 4
__attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)))
extern mach_msg_return_t mach_msg_overwrite(
 mach_msg_header_t *msg,
 mach_msg_option_t option,
 mach_msg_size_t send_size,
 mach_msg_size_t rcv_size,
 mach_port_name_t rcv_name,
 mach_msg_timeout_t timeout,
 mach_port_name_t notify,
 mach_msg_header_t *rcv_msg,
 mach_msg_size_t rcv_limit);
# 928 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/message.h" 3 4
__attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)))
extern mach_msg_return_t mach_msg(
 mach_msg_header_t *msg,
 mach_msg_option_t option,
 mach_msg_size_t send_size,
 mach_msg_size_t rcv_size,
 mach_port_name_t rcv_name,
 mach_msg_timeout_t timeout,
 mach_port_name_t notify);
# 945 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/message.h" 3 4
__attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)))
extern kern_return_t mach_voucher_deallocate(
 mach_port_name_t voucher);
# 67 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/host_info.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_statistics.h" 1 3 4
# 69 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_statistics.h" 3 4
# 1 "/opt/homebrew/Cellar/llvm/21.1.8/lib/clang/21/include/stdbool.h" 1 3 4
# 70 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_statistics.h" 2 3 4
# 89 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_statistics.h" 3 4
struct vm_statistics {
 natural_t free_count;
 natural_t active_count;
 natural_t inactive_count;
 natural_t wire_count;
 natural_t zero_fill_count;
 natural_t reactivations;
 natural_t pageins;
 natural_t pageouts;
 natural_t faults;
 natural_t cow_faults;
 natural_t lookups;
 natural_t hits;


 natural_t purgeable_count;
 natural_t purges;
# 114 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_statistics.h" 3 4
 natural_t speculative_count;
};


typedef struct vm_statistics *vm_statistics_t;
typedef struct vm_statistics vm_statistics_data_t;
# 137 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_statistics.h" 3 4
struct vm_statistics64 {
 natural_t free_count;
 natural_t active_count;
 natural_t inactive_count;
 natural_t wire_count;
 uint64_t zero_fill_count;
 uint64_t reactivations;
 uint64_t pageins;
 uint64_t pageouts;
 uint64_t faults;
 uint64_t cow_faults;
 uint64_t lookups;
 uint64_t hits;
 uint64_t purges;
 natural_t purgeable_count;






 natural_t speculative_count;


 uint64_t decompressions;
 uint64_t compressions;
 uint64_t swapins;
 uint64_t swapouts;
 natural_t compressor_page_count;
 natural_t throttled_count;
 natural_t external_page_count;
 natural_t internal_page_count;
 uint64_t total_uncompressed_pages_in_compressor;
} __attribute__((aligned(8)));

typedef struct vm_statistics64 *vm_statistics64_t;
typedef struct vm_statistics64 vm_statistics64_data_t;

kern_return_t vm_stats(void *info, unsigned int *count);
# 195 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_statistics.h" 3 4
struct vm_extmod_statistics {
 int64_t task_for_pid_count;
 int64_t task_for_pid_caller_count;
 int64_t thread_creation_count;
 int64_t thread_creation_caller_count;
 int64_t thread_set_state_count;
 int64_t thread_set_state_caller_count;
} __attribute__((aligned(8)));

typedef struct vm_extmod_statistics *vm_extmod_statistics_t;
typedef struct vm_extmod_statistics vm_extmod_statistics_data_t;

typedef struct vm_purgeable_stat {
 uint64_t count;
 uint64_t size;
}vm_purgeable_stat_t;

struct vm_purgeable_info {
 vm_purgeable_stat_t fifo_data[8];
 vm_purgeable_stat_t obsolete_data;
 vm_purgeable_stat_t lifo_data[8];
};

typedef struct vm_purgeable_info *vm_purgeable_info_t;
# 334 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_statistics.h" 3 4
typedef enum : uint32_t { kGUARD_EXC_DEALLOC_GAP = 1, kGUARD_EXC_RECLAIM_COPYIO_FAILURE = 2, kGUARD_EXC_SEC_LOOKUP_DENIED = 3, kGUARD_EXC_RECLAIM_INDEX_FAILURE = 4, kGUARD_EXC_SEC_RANGE_DENIED = 6, kGUARD_EXC_SEC_ACCESS_FAULT = 7, kGUARD_EXC_RECLAIM_DEALLOCATE_FAILURE = 8, kGUARD_EXC_SEC_COPY_DENIED = 16, kGUARD_EXC_SEC_SHARING_DENIED = 32, kGUARD_EXC_SEC_ASYNC_ACCESS_FAULT = 64,} __attribute__((__enum_extensibility__(open))) virtual_memory_guard_exception_code_t;
# 68 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/host_info.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/machine.h" 1 3 4
# 70 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/machine.h" 3 4
typedef integer_t cpu_type_t;
typedef integer_t cpu_subtype_t;
typedef integer_t cpu_threadtype_t;
# 69 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/host_info.h" 2 3 4

# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/time_value.h" 1 3 4
# 66 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/time_value.h" 3 4
struct time_value {
 integer_t seconds;
 integer_t microseconds;
};

typedef struct time_value time_value_t;
# 71 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/host_info.h" 2 3 4






typedef integer_t *host_info_t;
typedef integer_t *host_info64_t;


typedef integer_t host_info_data_t[(1024)];


typedef char kernel_version_t[(512)];


typedef char kernel_boot_info_t[(4096)];





typedef integer_t host_flavor_t;
# 106 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/host_info.h" 3 4
struct host_can_has_debugger_info {
 boolean_t can_has_debugger;
};
typedef struct host_can_has_debugger_info host_can_has_debugger_info_data_t;
typedef struct host_can_has_debugger_info *host_can_has_debugger_info_t;



#pragma pack(push, 4)

struct host_basic_info {
 integer_t max_cpus;
 integer_t avail_cpus;
 natural_t memory_size;
 cpu_type_t cpu_type;
 cpu_subtype_t cpu_subtype;
 cpu_threadtype_t cpu_threadtype;
 integer_t physical_cpu;
 integer_t physical_cpu_max;
 integer_t logical_cpu;
 integer_t logical_cpu_max;
 uint64_t max_mem;
};

#pragma pack(pop)

typedef struct host_basic_info host_basic_info_data_t;
typedef struct host_basic_info *host_basic_info_t;



struct host_sched_info {
 integer_t min_timeout;
 integer_t min_quantum;
};

typedef struct host_sched_info host_sched_info_data_t;
typedef struct host_sched_info *host_sched_info_t;



struct kernel_resource_sizes {
 natural_t task;
 natural_t thread;
 natural_t port;
 natural_t memory_region;
 natural_t memory_object;
};

typedef struct kernel_resource_sizes kernel_resource_sizes_data_t;
typedef struct kernel_resource_sizes *kernel_resource_sizes_t;



struct host_priority_info {
 integer_t kernel_priority;
 integer_t system_priority;
 integer_t server_priority;
 integer_t user_priority;
 integer_t depress_priority;
 integer_t idle_priority;
 integer_t minimum_priority;
 integer_t maximum_priority;
};

typedef struct host_priority_info host_priority_info_data_t;
typedef struct host_priority_info *host_priority_info_t;
# 188 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/host_info.h" 3 4
struct host_load_info {
 integer_t avenrun[3];
 integer_t mach_factor[3];
};

typedef struct host_load_info host_load_info_data_t;
typedef struct host_load_info *host_load_info_t;



typedef struct vm_purgeable_info host_purgable_info_data_t;
typedef struct vm_purgeable_info *host_purgable_info_t;
# 239 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/host_info.h" 3 4
struct host_cpu_load_info {
 natural_t cpu_ticks[4];
};

typedef struct host_cpu_load_info host_cpu_load_info_data_t;
typedef struct host_cpu_load_info *host_cpu_load_info_t;



struct host_preferred_user_arch {
 cpu_type_t cpu_type;
 cpu_subtype_t cpu_subtype;
};

typedef struct host_preferred_user_arch host_preferred_user_arch_data_t;
typedef struct host_preferred_user_arch *host_preferred_user_arch_t;
# 81 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_types.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/host_notify.h" 1 3 4
# 82 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_types.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/host_special_ports.h" 1 3 4
# 83 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_types.h" 2 3 4


# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/memory_object_types.h" 1 3 4
# 75 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/memory_object_types.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_prot.h" 1 3 4
# 75 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_prot.h" 3 4
typedef int vm_prot_t;
# 76 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/memory_object_types.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_sync.h" 1 3 4
# 66 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_sync.h" 3 4
typedef unsigned vm_sync_t;
# 77 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/memory_object_types.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_types.h" 1 3 4
# 43 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_types.h" 3 4
typedef vm_offset_t pointer_t ;
typedef vm_offset_t vm_address_t ;







typedef uint64_t addr64_t;
# 64 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_types.h" 3 4
typedef uint32_t reg64_t;






typedef uint32_t ppnum_t ;



typedef mach_port_t vm_map_t, vm_map_read_t, vm_map_inspect_t;
typedef mach_port_t upl_t;
typedef mach_port_t vm_named_entry_t;


typedef mach_vm_offset_t *mach_vm_offset_list_t;
# 92 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_types.h" 3 4
typedef uint64_t vm_object_offset_t;
typedef uint64_t vm_object_size_t;
# 104 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_types.h" 3 4
typedef struct mach_vm_range {
 mach_vm_offset_t min_address;
 mach_vm_offset_t max_address;
} *mach_vm_range_t;
# 118 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_types.h" 3 4
typedef enum : uint32_t { MACH_VM_RANGE_FLAVOR_INVALID, MACH_VM_RANGE_FLAVOR_V1,} __attribute__((__enum_extensibility__(open))) mach_vm_range_flavor_t;
# 130 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_types.h" 3 4
typedef enum : uint64_t { MACH_VM_RANGE_NONE = 0x000000000000,} __attribute__((__enum_extensibility__(open))) __attribute__((__flag_enum__)) mach_vm_range_flags_t;
# 158 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_types.h" 3 4
typedef enum : uint16_t { MACH_VM_RANGE_DEFAULT, MACH_VM_RANGE_DATA, MACH_VM_RANGE_FIXED,} __attribute__((__enum_extensibility__(open))) mach_vm_range_tag_t;





#pragma pack(1)

typedef struct {
 mach_vm_range_flags_t flags: 48;
 mach_vm_range_tag_t range_tag : 8;
 uint8_t vm_tag : 8;
 struct mach_vm_range range;
} mach_vm_range_recipe_v1_t;

#pragma pack()


typedef mach_vm_range_recipe_v1_t mach_vm_range_recipe_t;

typedef uint8_t *mach_vm_range_recipes_raw_t;
# 78 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/memory_object_types.h" 2 3 4







typedef unsigned long long memory_object_offset_t;
typedef unsigned long long memory_object_size_t;
typedef natural_t memory_object_cluster_size_t;
typedef natural_t * memory_object_fault_info_t;

typedef unsigned long long vm_object_id_t;







typedef mach_port_t memory_object_t;





typedef mach_port_t memory_object_control_t;


typedef memory_object_t *memory_object_array_t;




typedef mach_port_t memory_object_name_t;



typedef mach_port_t memory_object_default_t;
# 126 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/memory_object_types.h" 3 4
typedef int memory_object_copy_strategy_t;
# 168 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/memory_object_types.h" 3 4
typedef int memory_object_return_t;
# 197 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/memory_object_types.h" 3 4
typedef int *memory_object_info_t;
typedef int memory_object_flavor_t;
typedef int memory_object_info_data_t[(1024)];







struct memory_object_perf_info {
 memory_object_cluster_size_t cluster_size;
 boolean_t may_cache;
};

struct memory_object_attr_info {
 memory_object_copy_strategy_t copy_strategy;
 memory_object_cluster_size_t cluster_size;
 boolean_t may_cache_object;
 boolean_t temporary;
};

struct memory_object_behave_info {
 memory_object_copy_strategy_t copy_strategy;
 boolean_t temporary;
 boolean_t invalidate;
 boolean_t silent_overwrite;
 boolean_t advisory_pageout;
};


typedef struct memory_object_behave_info *memory_object_behave_info_t;
typedef struct memory_object_behave_info memory_object_behave_info_data_t;

typedef struct memory_object_perf_info *memory_object_perf_info_t;
typedef struct memory_object_perf_info memory_object_perf_info_data_t;

typedef struct memory_object_attr_info *memory_object_attr_info_t;
typedef struct memory_object_attr_info memory_object_attr_info_data_t;
# 86 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_types.h" 2 3 4

# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/exception_types.h" 1 3 4
# 62 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/exception_types.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/machine/exception.h" 1 3 4
# 35 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/machine/exception.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/arm/exception.h" 1 3 4
# 36 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/machine/exception.h" 2 3 4
# 63 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/exception_types.h" 2 3 4
# 195 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/exception_types.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/thread_status.h" 1 3 4
# 76 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/thread_status.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/machine/thread_status.h" 1 3 4
# 35 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/machine/thread_status.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/arm/thread_status.h" 1 3 4
# 38 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/arm/thread_status.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/machine/_structs.h" 1 3 4
# 35 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/machine/_structs.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/arm/_structs.h" 1 3 4
# 41 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/arm/_structs.h" 3 4
struct __darwin_arm_exception_state
{
 __uint32_t __exception;
 __uint32_t __fsr;
 __uint32_t __far;
};
# 59 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/arm/_structs.h" 3 4
struct __darwin_arm_exception_state64
{
 __uint64_t __far;
 __uint32_t __esr;
 __uint32_t __exception;
};

struct __darwin_arm_exception_state64_v2
{
 __uint64_t __far;
 __uint64_t __esr;
};
# 89 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/arm/_structs.h" 3 4
struct __darwin_arm_thread_state
{
 __uint32_t __r[13];
 __uint32_t __sp;
 __uint32_t __lr;
 __uint32_t __pc;
 __uint32_t __cpsr;
};
# 148 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/arm/_structs.h" 3 4
struct __darwin_arm_thread_state64
{
 __uint64_t __x[29];
 __uint64_t __fp;
 __uint64_t __lr;
 __uint64_t __sp;
 __uint64_t __pc;
 __uint32_t __cpsr;
 __uint32_t __pad;
};
# 519 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/arm/_structs.h" 3 4
struct __darwin_arm_vfp_state
{
 __uint32_t __r[64];
 __uint32_t __fpscr;
};
# 538 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/arm/_structs.h" 3 4
struct __darwin_arm_neon_state64
{
 __uint128_t __v[32];
 __uint32_t __fpsr;
 __uint32_t __fpcr;
};

struct __darwin_arm_neon_state
{
 __uint128_t __v[16];
 __uint32_t __fpsr;
 __uint32_t __fpcr;
};
# 609 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/arm/_structs.h" 3 4
struct __arm_pagein_state
{
 int __pagein_error;
};



struct __darwin_arm_sme_state
{
 __uint64_t __svcr;
 __uint64_t __tpidr2_el0;
 __uint16_t __svl_b;
};


struct __darwin_arm_sve_z_state
{
 char __z[16][256];
} __attribute__((aligned(4)));


struct __darwin_arm_sve_p_state
{
 char __p[16][256 / 8];
} __attribute__((aligned(4)));


struct __darwin_arm_sme_za_state
{
 char __za[4096];
} __attribute__((aligned(4)));


struct __darwin_arm_sme2_state
{
 char __zt0[64];
} __attribute__((aligned(4)));
# 712 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/arm/_structs.h" 3 4
struct __arm_legacy_debug_state
{
 __uint32_t __bvr[16];
 __uint32_t __bcr[16];
 __uint32_t __wvr[16];
 __uint32_t __wcr[16];
};
# 735 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/arm/_structs.h" 3 4
struct __darwin_arm_debug_state32
{
 __uint32_t __bvr[16];
 __uint32_t __bcr[16];
 __uint32_t __wvr[16];
 __uint32_t __wcr[16];
 __uint64_t __mdscr_el1;
};


struct __darwin_arm_debug_state64
{
 __uint64_t __bvr[16];
 __uint64_t __bcr[16];
 __uint64_t __wvr[16];
 __uint64_t __wcr[16];
 __uint64_t __mdscr_el1;
};
# 777 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/arm/_structs.h" 3 4
struct __darwin_arm_cpmu_state64
{
 __uint64_t __ctrs[16];
};
# 36 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/machine/_structs.h" 2 3 4
# 39 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/arm/thread_status.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/machine/thread_state.h" 1 3 4
# 35 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/machine/thread_state.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/arm/thread_state.h" 1 3 4
# 36 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/machine/thread_state.h" 2 3 4
# 40 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/arm/thread_status.h" 2 3 4
# 132 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/arm/thread_status.h" 3 4
struct arm_state_hdr {
 uint32_t flavor;
 uint32_t count;
};
typedef struct arm_state_hdr arm_state_hdr_t;

typedef struct __darwin_arm_thread_state arm_thread_state_t;
typedef struct __darwin_arm_thread_state arm_thread_state32_t;
typedef struct __darwin_arm_thread_state64 arm_thread_state64_t;
# 192 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/arm/thread_status.h" 3 4
struct arm_unified_thread_state {
 arm_state_hdr_t ash;
 union {
  arm_thread_state32_t ts_32;
  arm_thread_state64_t ts_64;
 } uts;
};


typedef struct arm_unified_thread_state arm_unified_thread_state_t;
# 213 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/arm/thread_status.h" 3 4
typedef struct __darwin_arm_vfp_state arm_vfp_state_t;
typedef struct __darwin_arm_neon_state arm_neon_state_t;
typedef struct __darwin_arm_neon_state arm_neon_state32_t;
typedef struct __darwin_arm_neon_state64 arm_neon_state64_t;


typedef struct __darwin_arm_exception_state arm_exception_state_t;
typedef struct __darwin_arm_exception_state arm_exception_state32_t;
typedef struct __darwin_arm_exception_state64 arm_exception_state64_t;
typedef struct __darwin_arm_exception_state64_v2 arm_exception_state64_v2_t;

typedef struct __darwin_arm_debug_state32 arm_debug_state32_t;
typedef struct __darwin_arm_debug_state64 arm_debug_state64_t;

typedef struct __arm_pagein_state arm_pagein_state_t;

typedef struct __darwin_arm_sme_state arm_sme_state_t;
typedef struct __darwin_arm_sve_z_state arm_sve_z_state_t;
typedef struct __darwin_arm_sve_p_state arm_sve_p_state_t;
typedef struct __darwin_arm_sme_za_state arm_sme_za_state_t;
typedef struct __darwin_arm_sme2_state arm_sme2_state_t;
# 242 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/arm/thread_status.h" 3 4
typedef struct __arm_legacy_debug_state arm_debug_state_t;
# 36 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/machine/thread_status.h" 2 3 4
# 77 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/thread_status.h" 2 3 4






typedef natural_t *thread_state_t;


typedef natural_t thread_state_data_t[1296];







typedef int thread_state_flavor_t;
typedef thread_state_flavor_t *thread_state_flavor_array_t;
# 196 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/exception_types.h" 2 3 4

# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach_debug/ipc_info.h" 1 3 4
# 78 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach_debug/ipc_info.h" 3 4
typedef struct ipc_info_space {
 natural_t iis_genno_mask;
 natural_t iis_table_size;
 natural_t iis_table_next;
 natural_t iis_tree_size;
 natural_t iis_tree_small;
 natural_t iis_tree_hash;
} ipc_info_space_t;

typedef struct ipc_info_space_basic {
 natural_t iisb_genno_mask;
 natural_t iisb_table_size;
 natural_t iisb_table_next;
 natural_t iisb_table_inuse;
 natural_t iisb_reserved[2];
} ipc_info_space_basic_t;

typedef struct ipc_info_name {
 mach_port_name_t iin_name;
              integer_t iin_collision;
 mach_port_type_t iin_type;
 mach_port_urefs_t iin_urefs;
 natural_t iin_object;
 natural_t iin_next;
 natural_t iin_hash;
} ipc_info_name_t;

typedef ipc_info_name_t *ipc_info_name_array_t;


typedef struct ipc_info_tree_name {
 ipc_info_name_t iitn_name;
 mach_port_name_t iitn_lchild;
 mach_port_name_t iitn_rchild;
} ipc_info_tree_name_t;

typedef ipc_info_tree_name_t *ipc_info_tree_name_array_t;

typedef struct ipc_info_port {
 natural_t iip_port_object;
 natural_t iip_receiver_object;
} ipc_info_port_t;

typedef ipc_info_port_t *exception_handler_info_array_t;
# 198 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/exception_types.h" 2 3 4




typedef int exception_type_t;
typedef integer_t exception_data_type_t;
typedef int64_t mach_exception_data_type_t;
typedef int exception_behavior_t;
typedef exception_data_type_t *exception_data_t;
typedef mach_exception_data_type_t *mach_exception_data_t;
typedef unsigned int exception_mask_t;
typedef exception_mask_t *exception_mask_array_t;
typedef exception_behavior_t *exception_behavior_array_t;
typedef thread_state_flavor_t *exception_flavor_array_t;
typedef mach_port_t *exception_port_array_t;
typedef ipc_info_port_t *exception_port_info_array_t;
typedef mach_exception_data_type_t mach_exception_code_t;
typedef mach_exception_data_type_t mach_exception_subcode_t;
# 88 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_types.h" 2 3 4

# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_voucher_types.h" 1 3 4
# 32 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_voucher_types.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/std_types.h" 1 3 4
# 73 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/std_types.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_uuid_t.h" 1 3 4
# 31 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_uuid_t.h" 3 4
typedef __darwin_uuid_t uuid_t;
# 74 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/std_types.h" 2 3 4
# 33 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_voucher_types.h" 2 3 4
# 54 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_voucher_types.h" 3 4
typedef mach_port_t mach_voucher_t;


typedef mach_port_name_t mach_voucher_name_t;


typedef mach_voucher_name_t *mach_voucher_name_array_t;






typedef mach_voucher_t ipc_voucher_t;







typedef uint32_t mach_voucher_selector_t;
# 85 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_voucher_types.h" 3 4
typedef uint32_t mach_voucher_attr_key_t;
typedef mach_voucher_attr_key_t *mach_voucher_attr_key_array_t;
# 112 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_voucher_types.h" 3 4
typedef uint8_t *mach_voucher_attr_content_t;
typedef uint32_t mach_voucher_attr_content_size_t;





typedef uint32_t mach_voucher_attr_command_t;
# 129 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_voucher_types.h" 3 4
typedef uint32_t mach_voucher_attr_recipe_command_t;
typedef mach_voucher_attr_recipe_command_t *mach_voucher_attr_recipe_command_array_t;
# 157 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_voucher_types.h" 3 4
#pragma pack(push, 1)

typedef struct mach_voucher_attr_recipe_data {
 mach_voucher_attr_key_t key;
 mach_voucher_attr_recipe_command_t command;
 mach_voucher_name_t previous_voucher;
 mach_voucher_attr_content_size_t content_size;
 uint8_t content[];
} mach_voucher_attr_recipe_data_t;
typedef mach_voucher_attr_recipe_data_t *mach_voucher_attr_recipe_t;
typedef mach_msg_type_number_t mach_voucher_attr_recipe_size_t;


typedef uint8_t *mach_voucher_attr_raw_recipe_t;
typedef mach_voucher_attr_raw_recipe_t mach_voucher_attr_raw_recipe_array_t;
typedef mach_msg_type_number_t mach_voucher_attr_raw_recipe_size_t;
typedef mach_msg_type_number_t mach_voucher_attr_raw_recipe_array_size_t;




#pragma pack(pop)
# 190 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_voucher_types.h" 3 4
typedef mach_port_t mach_voucher_attr_manager_t;
# 199 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_voucher_types.h" 3 4
typedef mach_port_t mach_voucher_attr_control_t;







typedef mach_port_t ipc_voucher_attr_manager_t;
typedef mach_port_t ipc_voucher_attr_control_t;
# 218 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_voucher_types.h" 3 4
typedef uint64_t mach_voucher_attr_value_handle_t ;
typedef mach_voucher_attr_value_handle_t *mach_voucher_attr_value_handle_array_t;

typedef mach_msg_type_number_t mach_voucher_attr_value_handle_array_size_t;


typedef uint32_t mach_voucher_attr_value_reference_t;
typedef uint32_t mach_voucher_attr_value_flags_t;




typedef uint32_t mach_voucher_attr_control_flags_t;
# 241 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_voucher_types.h" 3 4
typedef uint32_t mach_voucher_attr_importance_refs;
# 90 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_types.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/processor_info.h" 1 3 4
# 72 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/processor_info.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/machine/processor_info.h" 1 3 4
# 35 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/machine/processor_info.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/arm/processor_info.h" 1 3 4
# 39 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/arm/processor_info.h" 3 4
struct processor_cpu_stat {
 uint32_t irq_ex_cnt;
 uint32_t ipi_cnt;
 uint32_t timer_cnt;
 uint32_t undef_ex_cnt;
 uint32_t unaligned_cnt;
 uint32_t vfp_cnt;
 uint32_t vfp_shortv_cnt;
 uint32_t data_ex_cnt;
 uint32_t instr_ex_cnt;
};

typedef struct processor_cpu_stat processor_cpu_stat_data_t;
typedef struct processor_cpu_stat *processor_cpu_stat_t;



struct processor_cpu_stat64 {
 uint64_t irq_ex_cnt;
 uint64_t ipi_cnt;
 uint64_t timer_cnt;
 uint64_t undef_ex_cnt;
 uint64_t unaligned_cnt;
 uint64_t vfp_cnt;
 uint64_t vfp_shortv_cnt;
 uint64_t data_ex_cnt;
 uint64_t instr_ex_cnt;
 uint64_t pmi_cnt;
} __attribute__((packed, aligned(4)));

typedef struct processor_cpu_stat64 processor_cpu_stat64_data_t;
typedef struct processor_cpu_stat64 *processor_cpu_stat64_t;
# 36 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/machine/processor_info.h" 2 3 4
# 73 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/processor_info.h" 2 3 4




typedef integer_t *processor_info_t;
typedef integer_t *processor_info_array_t;


typedef integer_t processor_info_data_t[(1024)];


typedef integer_t *processor_set_info_t;


typedef integer_t processor_set_info_data_t[(1024)];




typedef int processor_flavor_t;





struct processor_basic_info {
 cpu_type_t cpu_type;
 cpu_subtype_t cpu_subtype;
 boolean_t running;
 int slot_num;
 union {
  boolean_t is_master;
  boolean_t is_main;
 };
};

typedef struct processor_basic_info processor_basic_info_data_t;
typedef struct processor_basic_info *processor_basic_info_t;



struct processor_cpu_load_info {
 unsigned int cpu_ticks[4];
};

typedef struct processor_cpu_load_info processor_cpu_load_info_data_t;
typedef struct processor_cpu_load_info *processor_cpu_load_info_t;
# 128 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/processor_info.h" 3 4
typedef int processor_set_flavor_t;


struct processor_set_basic_info {
 int processor_count;
 int default_policy;
};

typedef struct processor_set_basic_info processor_set_basic_info_data_t;
typedef struct processor_set_basic_info *processor_set_basic_info_t;





struct processor_set_load_info {
 int task_count;
 int thread_count;
 integer_t load_average;
 integer_t mach_factor;
};

typedef struct processor_set_load_info processor_set_load_info_data_t;
typedef struct processor_set_load_info *processor_set_load_info_t;
# 91 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_types.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/task_info.h" 1 3 4
# 71 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/task_info.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/policy.h" 1 3 4
# 79 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/policy.h" 3 4
typedef int policy_t;
typedef integer_t *policy_info_t;
typedef integer_t *policy_base_t;
typedef integer_t *policy_limit_t;
# 113 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/policy.h" 3 4
struct policy_timeshare_base {
 integer_t base_priority;
};
struct policy_timeshare_limit {
 integer_t max_priority;
};
struct policy_timeshare_info {
 integer_t max_priority;
 integer_t base_priority;
 integer_t cur_priority;
 boolean_t depressed;
 integer_t depress_priority;
};

typedef struct policy_timeshare_base *policy_timeshare_base_t;
typedef struct policy_timeshare_limit *policy_timeshare_limit_t;
typedef struct policy_timeshare_info *policy_timeshare_info_t;

typedef struct policy_timeshare_base policy_timeshare_base_data_t;
typedef struct policy_timeshare_limit policy_timeshare_limit_data_t;
typedef struct policy_timeshare_info policy_timeshare_info_data_t;
# 147 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/policy.h" 3 4
struct policy_rr_base {
 integer_t base_priority;
 integer_t quantum;
};
struct policy_rr_limit {
 integer_t max_priority;
};
struct policy_rr_info {
 integer_t max_priority;
 integer_t base_priority;
 integer_t quantum;
 boolean_t depressed;
 integer_t depress_priority;
};

typedef struct policy_rr_base *policy_rr_base_t;
typedef struct policy_rr_limit *policy_rr_limit_t;
typedef struct policy_rr_info *policy_rr_info_t;

typedef struct policy_rr_base policy_rr_base_data_t;
typedef struct policy_rr_limit policy_rr_limit_data_t;
typedef struct policy_rr_info policy_rr_info_data_t;
# 181 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/policy.h" 3 4
struct policy_fifo_base {
 integer_t base_priority;
};
struct policy_fifo_limit {
 integer_t max_priority;
};
struct policy_fifo_info {
 integer_t max_priority;
 integer_t base_priority;
 boolean_t depressed;
 integer_t depress_priority;
};

typedef struct policy_fifo_base *policy_fifo_base_t;
typedef struct policy_fifo_limit *policy_fifo_limit_t;
typedef struct policy_fifo_info *policy_fifo_info_t;

typedef struct policy_fifo_base policy_fifo_base_data_t;
typedef struct policy_fifo_limit policy_fifo_limit_data_t;
typedef struct policy_fifo_info policy_fifo_info_data_t;
# 213 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/policy.h" 3 4
struct policy_bases {
 policy_timeshare_base_data_t ts;
 policy_rr_base_data_t rr;
 policy_fifo_base_data_t fifo;
};

struct policy_limits {
 policy_timeshare_limit_data_t ts;
 policy_rr_limit_data_t rr;
 policy_fifo_limit_data_t fifo;
};

struct policy_infos {
 policy_timeshare_info_data_t ts;
 policy_rr_info_data_t rr;
 policy_fifo_info_data_t fifo;
};

typedef struct policy_bases policy_base_data_t;
typedef struct policy_limits policy_limit_data_t;
typedef struct policy_infos policy_info_data_t;
# 72 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/task_info.h" 2 3 4

# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/Availability.h" 1 3 4
# 74 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/task_info.h" 2 3 4






typedef natural_t task_flavor_t;
typedef integer_t *task_info_t;



typedef integer_t task_info_data_t[(1024)];





#pragma pack(push, 4)





struct task_basic_info_32 {
 integer_t suspend_count;
 natural_t virtual_size;
 natural_t resident_size;
 time_value_t user_time;

 time_value_t system_time;

 policy_t policy;
};
typedef struct task_basic_info_32 task_basic_info_32_data_t;
typedef struct task_basic_info_32 *task_basic_info_32_t;




struct task_basic_info_64 {
 integer_t suspend_count;

 mach_vm_size_t virtual_size;
 mach_vm_size_t resident_size;




 time_value_t user_time;

 time_value_t system_time;

 policy_t policy;
};
typedef struct task_basic_info_64 task_basic_info_64_data_t;
typedef struct task_basic_info_64 *task_basic_info_64_t;
# 155 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/task_info.h" 3 4
struct task_basic_info {
 integer_t suspend_count;
 vm_size_t virtual_size;
 vm_size_t resident_size;
 time_value_t user_time;

 time_value_t system_time;

 policy_t policy;
};

typedef struct task_basic_info task_basic_info_data_t;
typedef struct task_basic_info *task_basic_info_t;
# 180 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/task_info.h" 3 4
struct task_events_info {
 integer_t faults;
 integer_t pageins;
 integer_t cow_faults;
 integer_t messages_sent;
 integer_t messages_received;
 integer_t syscalls_mach;
 integer_t syscalls_unix;
 integer_t csw;
};
typedef struct task_events_info task_events_info_data_t;
typedef struct task_events_info *task_events_info_t;






struct task_thread_times_info {
 time_value_t user_time;

 time_value_t system_time;

};

typedef struct task_thread_times_info task_thread_times_info_data_t;
typedef struct task_thread_times_info *task_thread_times_info_t;





struct task_absolutetime_info {
 uint64_t total_user;
 uint64_t total_system;
 uint64_t threads_user;
 uint64_t threads_system;
};

typedef struct task_absolutetime_info task_absolutetime_info_data_t;
typedef struct task_absolutetime_info *task_absolutetime_info_t;





struct task_kernelmemory_info {
 uint64_t total_palloc;
 uint64_t total_pfree;
 uint64_t total_salloc;
 uint64_t total_sfree;
};

typedef struct task_kernelmemory_info task_kernelmemory_info_data_t;
typedef struct task_kernelmemory_info *task_kernelmemory_info_t;
# 249 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/task_info.h" 3 4
struct task_affinity_tag_info {
 integer_t set_count;
 integer_t min;
 integer_t max;
 integer_t task_count;
};
typedef struct task_affinity_tag_info task_affinity_tag_info_data_t;
typedef struct task_affinity_tag_info *task_affinity_tag_info_t;





struct task_dyld_info {
 mach_vm_address_t all_image_info_addr;
 mach_vm_size_t all_image_info_size;
 integer_t all_image_info_format;
};
typedef struct task_dyld_info task_dyld_info_data_t;
typedef struct task_dyld_info *task_dyld_info_t;
# 280 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/task_info.h" 3 4
struct task_basic_info_64_2 {
 integer_t suspend_count;
 mach_vm_size_t virtual_size;
 mach_vm_size_t resident_size;
 time_value_t user_time;

 time_value_t system_time;

 policy_t policy;
};
typedef struct task_basic_info_64_2 task_basic_info_64_2_data_t;
typedef struct task_basic_info_64_2 *task_basic_info_64_2_t;






struct task_extmod_info {
 unsigned char task_uuid[16];
 vm_extmod_statistics_data_t extmod_statistics;
};
typedef struct task_extmod_info task_extmod_info_data_t;
typedef struct task_extmod_info *task_extmod_info_t;





struct mach_task_basic_info {
 mach_vm_size_t virtual_size;
 mach_vm_size_t resident_size;
 mach_vm_size_t resident_size_max;
 time_value_t user_time;

 time_value_t system_time;

 policy_t policy;
 integer_t suspend_count;
};
typedef struct mach_task_basic_info mach_task_basic_info_data_t;
typedef struct mach_task_basic_info *mach_task_basic_info_t;






struct task_power_info {
 uint64_t total_user;
 uint64_t total_system;
 uint64_t task_interrupt_wakeups;
 uint64_t task_platform_idle_wakeups;
 uint64_t task_timer_wakeups_bin_1;
 uint64_t task_timer_wakeups_bin_2;
};

typedef struct task_power_info task_power_info_data_t;
typedef struct task_power_info *task_power_info_t;







struct task_vm_info {
 mach_vm_size_t virtual_size;
 integer_t region_count;
 integer_t page_size;
 mach_vm_size_t resident_size;
 mach_vm_size_t resident_size_peak;

 mach_vm_size_t device;
 mach_vm_size_t device_peak;
 mach_vm_size_t internal;
 mach_vm_size_t internal_peak;
 mach_vm_size_t external;
 mach_vm_size_t external_peak;
 mach_vm_size_t reusable;
 mach_vm_size_t reusable_peak;
 mach_vm_size_t purgeable_volatile_pmap;
 mach_vm_size_t purgeable_volatile_resident;
 mach_vm_size_t purgeable_volatile_virtual;
 mach_vm_size_t compressed;
 mach_vm_size_t compressed_peak;
 mach_vm_size_t compressed_lifetime;


 mach_vm_size_t phys_footprint;


 mach_vm_address_t min_address;
 mach_vm_address_t max_address;


 int64_t ledger_phys_footprint_peak;
 int64_t ledger_purgeable_nonvolatile;
 int64_t ledger_purgeable_novolatile_compressed;
 int64_t ledger_purgeable_volatile;
 int64_t ledger_purgeable_volatile_compressed;
 int64_t ledger_tag_network_nonvolatile;
 int64_t ledger_tag_network_nonvolatile_compressed;
 int64_t ledger_tag_network_volatile;
 int64_t ledger_tag_network_volatile_compressed;
 int64_t ledger_tag_media_footprint;
 int64_t ledger_tag_media_footprint_compressed;
 int64_t ledger_tag_media_nofootprint;
 int64_t ledger_tag_media_nofootprint_compressed;
 int64_t ledger_tag_graphics_footprint;
 int64_t ledger_tag_graphics_footprint_compressed;
 int64_t ledger_tag_graphics_nofootprint;
 int64_t ledger_tag_graphics_nofootprint_compressed;
 int64_t ledger_tag_neural_footprint;
 int64_t ledger_tag_neural_footprint_compressed;
 int64_t ledger_tag_neural_nofootprint;
 int64_t ledger_tag_neural_nofootprint_compressed;


 uint64_t limit_bytes_remaining;


 integer_t decompressions;


 int64_t ledger_swapins;


 int64_t ledger_tag_neural_nofootprint_total;
 int64_t ledger_tag_neural_nofootprint_peak;
};
typedef struct task_vm_info task_vm_info_data_t;
typedef struct task_vm_info *task_vm_info_t;
# 434 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/task_info.h" 3 4
typedef struct vm_purgeable_info task_purgable_info_t;



struct task_trace_memory_info {
 uint64_t user_memory_address;
 uint64_t buffer_size;
 uint64_t mailbox_array_size;
};
typedef struct task_trace_memory_info task_trace_memory_info_data_t;
typedef struct task_trace_memory_info * task_trace_memory_info_t;




struct task_wait_state_info {
 uint64_t total_wait_state_time;
 uint64_t total_wait_sfi_state_time;
 uint32_t _reserved[4];
};
typedef struct task_wait_state_info task_wait_state_info_data_t;
typedef struct task_wait_state_info * task_wait_state_info_t;





typedef struct {
 uint64_t task_gpu_utilisation;
 uint64_t task_gpu_stat_reserved0;
 uint64_t task_gpu_stat_reserved1;
 uint64_t task_gpu_stat_reserved2;
} gpu_energy_data;

typedef gpu_energy_data *gpu_energy_data_t;
struct task_power_info_v2 {
 task_power_info_data_t cpu_energy;
 gpu_energy_data gpu_energy;

 uint64_t task_energy;

 uint64_t task_ptime;
 uint64_t task_pset_switches;
};

typedef struct task_power_info_v2 task_power_info_v2_data_t;
typedef struct task_power_info_v2 *task_power_info_v2_t;
# 490 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/task_info.h" 3 4
struct task_flags_info {
 uint32_t flags;
};
typedef struct task_flags_info task_flags_info_data_t;
typedef struct task_flags_info * task_flags_info_t;
# 506 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/task_info.h" 3 4
struct task_security_config_info {
 uint32_t config;
};

typedef struct task_security_config_info * task_security_config_info_t;







typedef uint32_t task_exc_guard_behavior_t;
# 541 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/task_info.h" 3 4
typedef uint32_t task_corpse_forking_behavior_t;
# 556 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/task_info.h" 3 4
#pragma pack(pop)
# 92 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_types.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/task_inspect.h" 1 3 4
# 40 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/task_inspect.h" 3 4
typedef natural_t task_inspect_flavor_t;

enum task_inspect_flavor {
 TASK_INSPECT_BASIC_COUNTS = 1,
};

struct task_inspect_basic_counts {
 uint64_t instructions;
 uint64_t cycles;
};


typedef struct task_inspect_basic_counts task_inspect_basic_counts_data_t;
typedef struct task_inspect_basic_counts *task_inspect_basic_counts_t;

typedef integer_t *task_inspect_info_t;
# 93 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_types.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/task_policy.h" 1 3 4
# 32 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/task_policy.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_types.h" 1 3 4
# 33 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/task_policy.h" 2 3 4
# 51 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/task_policy.h" 3 4
typedef natural_t task_policy_flavor_t;
typedef integer_t *task_policy_t;
# 113 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/task_policy.h" 3 4
typedef enum task_role {
 TASK_RENICED = -1,
 TASK_UNSPECIFIED = 0,
 TASK_FOREGROUND_APPLICATION = 1,
 TASK_BACKGROUND_APPLICATION = 2,
 TASK_CONTROL_APPLICATION = 3,
 TASK_GRAPHICS_SERVER = 4,
 TASK_THROTTLE_APPLICATION = 5,
 TASK_NONUI_APPLICATION = 6,
 TASK_DEFAULT_APPLICATION = 7,
 TASK_DARWINBG_APPLICATION = 8,
} task_role_t;

struct task_category_policy {
 task_role_t role;
};

typedef struct task_category_policy task_category_policy_data_t;
typedef struct task_category_policy *task_category_policy_t;





enum task_latency_qos {
 LATENCY_QOS_TIER_UNSPECIFIED = 0x0,
 LATENCY_QOS_TIER_0 = ((0xFF << 16) | 1),
 LATENCY_QOS_TIER_1 = ((0xFF << 16) | 2),
 LATENCY_QOS_TIER_2 = ((0xFF << 16) | 3),
 LATENCY_QOS_TIER_3 = ((0xFF << 16) | 4),
 LATENCY_QOS_TIER_4 = ((0xFF << 16) | 5),
 LATENCY_QOS_TIER_5 = ((0xFF << 16) | 6)
};
typedef integer_t task_latency_qos_t;
enum task_throughput_qos {
 THROUGHPUT_QOS_TIER_UNSPECIFIED = 0x0,
 THROUGHPUT_QOS_TIER_0 = ((0xFE << 16) | 1),
 THROUGHPUT_QOS_TIER_1 = ((0xFE << 16) | 2),
 THROUGHPUT_QOS_TIER_2 = ((0xFE << 16) | 3),
 THROUGHPUT_QOS_TIER_3 = ((0xFE << 16) | 4),
 THROUGHPUT_QOS_TIER_4 = ((0xFE << 16) | 5),
 THROUGHPUT_QOS_TIER_5 = ((0xFE << 16) | 6),
};




typedef integer_t task_throughput_qos_t;

struct task_qos_policy {
 task_latency_qos_t task_latency_qos_tier;
 task_throughput_qos_t task_throughput_qos_tier;
};

typedef struct task_qos_policy *task_qos_policy_t;
# 94 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_types.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/task_special_ports.h" 1 3 4
# 70 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/task_special_ports.h" 3 4
typedef int task_special_port_t;
# 95 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_types.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/thread_info.h" 1 3 4
# 81 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/thread_info.h" 3 4
typedef natural_t thread_flavor_t;
typedef integer_t *thread_info_t;


typedef integer_t thread_info_data_t[(32)];






struct thread_basic_info {
 time_value_t user_time;
 time_value_t system_time;
 integer_t cpu_usage;
 policy_t policy;
 integer_t run_state;
 integer_t flags;
 integer_t suspend_count;
 integer_t sleep_time;

};

typedef struct thread_basic_info thread_basic_info_data_t;
typedef struct thread_basic_info *thread_basic_info_t;





struct thread_identifier_info {
 uint64_t thread_id;
 uint64_t thread_handle;
 uint64_t dispatch_qaddr;
};

typedef struct thread_identifier_info thread_identifier_info_data_t;
typedef struct thread_identifier_info *thread_identifier_info_t;
# 152 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/thread_info.h" 3 4
struct thread_extended_info {
 uint64_t pth_user_time;
 uint64_t pth_system_time;
 int32_t pth_cpu_usage;
 int32_t pth_policy;
 int32_t pth_run_state;
 int32_t pth_flags;
 int32_t pth_sleep_time;
 int32_t pth_curpri;
 int32_t pth_priority;
 int32_t pth_maxpriority;
 char pth_name[64];
};
typedef struct thread_extended_info thread_extended_info_data_t;
typedef struct thread_extended_info * thread_extended_info_t;
# 187 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/thread_info.h" 3 4
struct io_stat_entry {
 uint64_t count;
 uint64_t size;
};

struct io_stat_info {
 struct io_stat_entry disk_reads;
 struct io_stat_entry io_priority[4];
 struct io_stat_entry paging;
 struct io_stat_entry metadata;
 struct io_stat_entry total_io;
};

typedef struct io_stat_info *io_stat_info_t;
# 96 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_types.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/thread_policy.h" 1 3 4
# 32 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/thread_policy.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_types.h" 1 3 4
# 33 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/thread_policy.h" 2 3 4
# 51 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/thread_policy.h" 3 4
typedef natural_t thread_policy_flavor_t;
typedef integer_t *thread_policy_t;
# 86 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/thread_policy.h" 3 4
struct thread_standard_policy {
 natural_t no_data;
};

typedef struct thread_standard_policy thread_standard_policy_data_t;
typedef struct thread_standard_policy *thread_standard_policy_t;
# 109 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/thread_policy.h" 3 4
struct thread_extended_policy {
 boolean_t timeshare;
};

typedef struct thread_extended_policy thread_extended_policy_data_t;
typedef struct thread_extended_policy *thread_extended_policy_t;
# 152 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/thread_policy.h" 3 4
struct thread_time_constraint_policy {
 uint32_t period;
 uint32_t computation;
 uint32_t constraint;
 boolean_t preemptible;
};

typedef struct thread_time_constraint_policy thread_time_constraint_policy_data_t;

typedef struct thread_time_constraint_policy *thread_time_constraint_policy_t;
# 180 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/thread_policy.h" 3 4
struct thread_precedence_policy {
 integer_t importance;
};

typedef struct thread_precedence_policy thread_precedence_policy_data_t;
typedef struct thread_precedence_policy *thread_precedence_policy_t;
# 210 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/thread_policy.h" 3 4
struct thread_affinity_policy {
 integer_t affinity_tag;
};



typedef struct thread_affinity_policy thread_affinity_policy_data_t;
typedef struct thread_affinity_policy *thread_affinity_policy_t;
# 228 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/thread_policy.h" 3 4
struct thread_background_policy {
 integer_t priority;
};



typedef struct thread_background_policy thread_background_policy_data_t;
typedef struct thread_background_policy *thread_background_policy_t;






typedef integer_t thread_latency_qos_t;

struct thread_latency_qos_policy {
 thread_latency_qos_t thread_latency_qos_tier;
};

typedef struct thread_latency_qos_policy thread_latency_qos_policy_data_t;
typedef struct thread_latency_qos_policy *thread_latency_qos_policy_t;





typedef integer_t thread_throughput_qos_t;

struct thread_throughput_qos_policy {
 thread_throughput_qos_t thread_throughput_qos_tier;
};

typedef struct thread_throughput_qos_policy thread_throughput_qos_policy_data_t;
typedef struct thread_throughput_qos_policy *thread_throughput_qos_policy_t;
# 97 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_types.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/thread_special_ports.h" 1 3 4
# 98 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_types.h" 2 3 4


# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/clock_types.h" 1 3 4
# 51 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/clock_types.h" 3 4
typedef int alarm_type_t;
typedef int sleep_type_t;
typedef int clock_id_t;
typedef int clock_flavor_t;
typedef int *clock_attr_t;
typedef int clock_res_t;




struct mach_timespec {
 unsigned int tv_sec;
 clock_res_t tv_nsec;
};
typedef struct mach_timespec mach_timespec_t;
# 101 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_types.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_attributes.h" 1 3 4
# 76 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_attributes.h" 3 4
typedef unsigned int vm_machine_attribute_t;
# 85 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_attributes.h" 3 4
typedef int vm_machine_attribute_val_t;
# 102 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_types.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_inherit.h" 1 3 4
# 75 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_inherit.h" 3 4
typedef unsigned int vm_inherit_t;
# 103 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_types.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_purgable.h" 1 3 4
# 53 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_purgable.h" 3 4
typedef int vm_purgable_t;
# 104 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_types.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_behavior.h" 1 3 4
# 47 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_behavior.h" 3 4
typedef int vm_behavior_t;
# 105 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_types.h" 2 3 4




# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_region.h" 1 3 4
# 47 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_region.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/machine/vm_param.h" 1 3 4
# 35 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/machine/vm_param.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/arm/vm_param.h" 1 3 4
# 44 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/arm/vm_param.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_page_size.h" 1 3 4
# 32 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_page_size.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/Availability.h" 1 3 4
# 33 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_page_size.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_types.h" 1 3 4
# 34 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_page_size.h" 2 3 4








extern vm_size_t vm_page_size;
extern vm_size_t vm_page_mask;
extern int vm_page_shift;
# 59 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_page_size.h" 3 4
extern vm_size_t vm_kernel_page_size __attribute__((availability(macosx,introduced=10.9)));
extern vm_size_t vm_kernel_page_mask __attribute__((availability(macosx,introduced=10.9)));
extern int vm_kernel_page_shift __attribute__((availability(macosx,introduced=10.9)));
# 45 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/arm/vm_param.h" 2 3 4
# 36 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/machine/vm_param.h" 2 3 4
# 48 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_region.h" 2 3 4





#pragma pack(push, 4)




typedef uint32_t vm32_object_id_t;
# 67 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_region.h" 3 4
typedef int *vm_region_info_t;
typedef int *vm_region_info_64_t;
typedef int *vm_region_recurse_info_t;
typedef int *vm_region_recurse_info_64_t;
typedef int vm_region_flavor_t;
typedef int vm_region_info_data_t[(1024)];



struct vm_region_basic_info_64 {
 vm_prot_t protection;
 vm_prot_t max_protection;
 vm_inherit_t inheritance;
 boolean_t shared;
 boolean_t reserved;
 memory_object_offset_t offset;
 vm_behavior_t behavior;
 unsigned short user_wired_count;
};
typedef struct vm_region_basic_info_64 *vm_region_basic_info_64_t;
typedef struct vm_region_basic_info_64 vm_region_basic_info_data_64_t;
# 104 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_region.h" 3 4
struct vm_region_basic_info {
 vm_prot_t protection;
 vm_prot_t max_protection;
 vm_inherit_t inheritance;
 boolean_t shared;
 boolean_t reserved;
 uint32_t offset;
 vm_behavior_t behavior;
 unsigned short user_wired_count;
};

typedef struct vm_region_basic_info *vm_region_basic_info_t;
typedef struct vm_region_basic_info vm_region_basic_info_data_t;
# 142 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_region.h" 3 4
struct vm_region_extended_info {
 vm_prot_t protection;
 unsigned int user_tag;
 unsigned int pages_resident;
 unsigned int pages_shared_now_private;
 unsigned int pages_swapped_out;
 unsigned int pages_dirtied;
 unsigned int ref_count;
 unsigned short shadow_depth;
 unsigned char external_pager;
 unsigned char share_mode;
 unsigned int pages_reusable;
};
typedef struct vm_region_extended_info *vm_region_extended_info_t;
typedef struct vm_region_extended_info vm_region_extended_info_data_t;
# 166 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_region.h" 3 4
struct vm_region_top_info {
 unsigned int obj_id;
 unsigned int ref_count;
 unsigned int private_pages_resident;
 unsigned int shared_pages_resident;
 unsigned char share_mode;
};

typedef struct vm_region_top_info *vm_region_top_info_t;
typedef struct vm_region_top_info vm_region_top_info_data_t;
# 203 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_region.h" 3 4
struct vm_region_submap_info {
 vm_prot_t protection;
 vm_prot_t max_protection;
 vm_inherit_t inheritance;
 uint32_t offset;
 unsigned int user_tag;
 unsigned int pages_resident;
 unsigned int pages_shared_now_private;
 unsigned int pages_swapped_out;
 unsigned int pages_dirtied;
 unsigned int ref_count;
 unsigned short shadow_depth;
 unsigned char external_pager;
 unsigned char share_mode;
 boolean_t is_submap;
 vm_behavior_t behavior;
 vm32_object_id_t object_id;
 unsigned short user_wired_count;
};

typedef struct vm_region_submap_info *vm_region_submap_info_t;
typedef struct vm_region_submap_info vm_region_submap_info_data_t;





struct vm_region_submap_info_64 {
 vm_prot_t protection;
 vm_prot_t max_protection;
 vm_inherit_t inheritance;
 memory_object_offset_t offset;
 unsigned int user_tag;
 unsigned int pages_resident;
 unsigned int pages_shared_now_private;
 unsigned int pages_swapped_out;
 unsigned int pages_dirtied;
 unsigned int ref_count;
 unsigned short shadow_depth;
 unsigned char external_pager;
 unsigned char share_mode;
 boolean_t is_submap;
 vm_behavior_t behavior;
 vm32_object_id_t object_id;
 unsigned short user_wired_count;
 unsigned int pages_reusable;
 vm_object_id_t object_id_full;
};

typedef struct vm_region_submap_info_64 *vm_region_submap_info_64_t;
typedef struct vm_region_submap_info_64 vm_region_submap_info_data_64_t;
# 277 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_region.h" 3 4
struct vm_region_submap_short_info_64 {
 vm_prot_t protection;
 vm_prot_t max_protection;
 vm_inherit_t inheritance;
 memory_object_offset_t offset;
 unsigned int user_tag;
 unsigned int ref_count;
 unsigned short shadow_depth;
 unsigned char external_pager;
 unsigned char share_mode;
 boolean_t is_submap;
 vm_behavior_t behavior;
 vm32_object_id_t object_id;
 unsigned short user_wired_count;
};

typedef struct vm_region_submap_short_info_64 *vm_region_submap_short_info_64_t;
typedef struct vm_region_submap_short_info_64 vm_region_submap_short_info_data_64_t;





struct mach_vm_read_entry {
 mach_vm_address_t address;
 mach_vm_size_t size;
};

struct vm_read_entry {
 vm_address_t address;
 vm_size_t size;
};
# 320 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/vm_region.h" 3 4
typedef struct mach_vm_read_entry mach_vm_read_entry_t[(256)];
typedef struct vm_read_entry vm_read_entry_t[(256)];




#pragma pack(pop)



typedef int *vm_page_info_t;
typedef int vm_page_info_data_t[];
typedef int vm_page_info_flavor_t;


struct vm_page_info_basic {
 int disposition;
 int ref_count;
 vm_object_id_t object_id;
 memory_object_offset_t offset;
 int depth;
 int __pad;
};
typedef struct vm_page_info_basic *vm_page_info_basic_t;
typedef struct vm_page_info_basic vm_page_info_basic_data_t;
# 110 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_types.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/kmod.h" 1 3 4
# 39 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/kmod.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_types.h" 1 3 4
# 40 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/kmod.h" 2 3 4
# 56 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/kmod.h" 3 4
typedef int kmod_t;

struct kmod_info;
typedef kern_return_t kmod_start_func_t(struct kmod_info * ki, void * data);
typedef kern_return_t kmod_stop_func_t(struct kmod_info * ki, void * data);
# 70 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/kmod.h" 3 4
#pragma pack(push, 4)


typedef struct kmod_reference {
 struct kmod_reference * next;
 struct kmod_info * info;
} kmod_reference_t;
# 87 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/kmod.h" 3 4
typedef struct kmod_info {
 struct kmod_info * next;
 int32_t info_version;
 uint32_t id;
 char name[64];
 char version[64];
 int32_t reference_count;
 kmod_reference_t * reference_list;
 vm_address_t address;
 vm_size_t size;
 vm_size_t hdr_size;
 kmod_start_func_t * start;
 kmod_stop_func_t * stop;
} kmod_info_t;



typedef struct kmod_info_32_v1 {
 uint32_t next_addr;
 int32_t info_version;
 uint32_t id;
 uint8_t name[64];
 uint8_t version[64];
 int32_t reference_count;
 uint32_t reference_list_addr;
 uint32_t address;
 uint32_t size;
 uint32_t hdr_size;
 uint32_t start_addr;
 uint32_t stop_addr;
} kmod_info_32_v1_t;



typedef struct kmod_info_64_v1 {
 uint64_t next_addr;
 int32_t info_version;
 uint32_t id;
 uint8_t name[64];
 uint8_t version[64];
 int32_t reference_count;
 uint64_t reference_list_addr;
 uint64_t address;
 uint64_t size;
 uint64_t hdr_size;
 uint64_t start_addr;
 uint64_t stop_addr;
} kmod_info_64_v1_t;

#pragma pack(pop)
# 174 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/kmod.h" 3 4
typedef void * kmod_args_t;
typedef int kmod_control_flavor_t;
typedef kmod_info_t * kmod_info_array_t;
# 111 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_types.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/dyld_kernel.h" 1 3 4
# 34 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/dyld_kernel.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_fsid_t.h" 1 3 4
# 31 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_fsid_t.h" 3 4
typedef struct fsid { int32_t val[2]; } fsid_t;
# 35 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/dyld_kernel.h" 2 3 4

# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_fsobj_id_t.h" 1 3 4
# 33 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_fsobj_id_t.h" 3 4
typedef struct fsobj_id {
 u_int32_t fid_objno;
 u_int32_t fid_generation;
} fsobj_id_t;
# 37 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/dyld_kernel.h" 2 3 4






struct dyld_kernel_image_info {
 uuid_t uuid;
 fsobj_id_t fsobjid;
 fsid_t fsid;
 uint64_t load_addr;
};

struct dyld_kernel_process_info {
 struct dyld_kernel_image_info cache_image_info;
 uint64_t timestamp;
 uint32_t imageCount;
 uint32_t initialImageCount;
 uint8_t dyldState;
 boolean_t no_cache;
 boolean_t private_cache;
};



typedef struct dyld_kernel_image_info dyld_kernel_image_info_t;
typedef struct dyld_kernel_process_info dyld_kernel_process_info_t;
typedef dyld_kernel_image_info_t *dyld_kernel_image_info_array_t;
# 112 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_types.h" 2 3 4






typedef mach_port_t task_t;
typedef mach_port_t task_name_t;
typedef mach_port_t task_policy_set_t;
typedef mach_port_t task_policy_get_t;
typedef mach_port_t task_inspect_t;
typedef mach_port_t task_read_t;
typedef mach_port_t task_suspension_token_t;
typedef mach_port_t thread_t;
typedef mach_port_t thread_act_t;
typedef mach_port_t thread_inspect_t;
typedef mach_port_t thread_read_t;
typedef mach_port_t ipc_space_t;
typedef mach_port_t ipc_space_read_t;
typedef mach_port_t ipc_space_inspect_t;
typedef mach_port_t coalition_t;
typedef mach_port_t host_t;
typedef mach_port_t host_priv_t;
typedef mach_port_t host_security_t;
typedef mach_port_t processor_t;
typedef mach_port_t processor_set_t;
typedef mach_port_t processor_set_control_t;
typedef mach_port_t semaphore_t;
typedef mach_port_t lock_set_t;
typedef mach_port_t ledger_t;
typedef mach_port_t alarm_t;
typedef mach_port_t clock_serv_t;
typedef mach_port_t clock_ctrl_t;
typedef mach_port_t arcade_register_t;
typedef mach_port_t ipc_eventlink_t;
typedef mach_port_t eventlink_port_pair_t[2];
typedef mach_port_t task_id_token_t;
typedef mach_port_t kcdata_object_t;







typedef processor_set_t processor_set_name_t;




typedef mach_port_t clock_reply_t;
typedef mach_port_t bootstrap_t;
typedef mach_port_t mem_entry_name_port_t;
typedef mach_port_t exception_handler_t;
typedef exception_handler_t *exception_handler_array_t;
typedef mach_port_t vm_task_entry_t;
typedef mach_port_t io_main_t;
typedef mach_port_t UNDServerRef;
typedef mach_port_t mach_eventlink_t;

typedef ipc_info_port_t exception_handler_info_t;
# 181 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_types.h" 3 4
typedef task_t *task_array_t;
typedef thread_t *thread_array_t;
typedef processor_set_t *processor_set_array_t;
typedef processor_set_t *processor_set_name_array_t;
typedef processor_t *processor_array_t;
typedef thread_act_t *thread_act_array_t;
typedef ledger_t *ledger_array_t;







typedef task_t task_port_t;
typedef task_array_t task_port_array_t;
typedef thread_t thread_port_t;
typedef thread_array_t thread_port_array_t;
typedef ipc_space_t ipc_space_port_t;
typedef host_t host_name_t;
typedef host_t host_name_port_t;
typedef processor_set_t processor_set_port_t;
typedef processor_set_t processor_set_name_port_t;
typedef processor_set_array_t processor_set_name_port_array_t;
typedef processor_set_t processor_set_control_port_t;
typedef processor_t processor_port_t;
typedef processor_array_t processor_port_array_t;
typedef thread_act_t thread_act_port_t;
typedef thread_act_array_t thread_act_port_array_t;
typedef semaphore_t semaphore_port_t;
typedef lock_set_t lock_set_port_t;
typedef ledger_t ledger_port_t;
typedef ledger_array_t ledger_port_array_t;
typedef alarm_t alarm_port_t;
typedef clock_serv_t clock_serv_port_t;
typedef clock_ctrl_t clock_ctrl_port_t;
typedef exception_handler_t exception_port_t;
typedef exception_handler_array_t exception_port_arrary_t;
typedef char vfs_path_t[4096];




typedef char nspace_path_t[8192];
typedef char nspace_name_t[8192];
# 261 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_types.h" 3 4
typedef unsigned int mach_task_flavor_t;
# 270 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_types.h" 3 4
typedef unsigned int mach_thread_flavor_t;







typedef natural_t ledger_item_t;


typedef int64_t ledger_amount_t;


typedef mach_vm_offset_t *emulation_vector_t;
typedef char *user_subsystem_t;

typedef char *labelstr_t;
# 33 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_time.h" 2 3 4

# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/Availability.h" 1 3 4
# 35 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/mach/mach_time.h" 2 3 4

struct mach_timebase_info {
 uint32_t numer;
 uint32_t denom;
};

typedef struct mach_timebase_info *mach_timebase_info_t;
typedef struct mach_timebase_info mach_timebase_info_data_t;



kern_return_t mach_timebase_info(
 mach_timebase_info_t info);

kern_return_t mach_wait_until(
 uint64_t deadline);


uint64_t mach_absolute_time(void);

__attribute__((availability(macosx,introduced=10.10)))
uint64_t mach_approximate_time(void);




__attribute__((availability(macosx,introduced=10.12))) __attribute__((availability(ios,introduced=10.0))) __attribute__((availability(tvos,introduced=10.0))) __attribute__((availability(watchos,introduced=3.0)))
uint64_t mach_continuous_time(void);




__attribute__((availability(macosx,introduced=10.12))) __attribute__((availability(ios,introduced=10.0))) __attribute__((availability(tvos,introduced=10.0))) __attribute__((availability(watchos,introduced=3.0)))
uint64_t mach_continuous_approximate_time(void);
# 5 "/Users/kevin/projects/lattice/salt-front/runtime.c" 2




# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/stdio.h" 1 3 4
# 61 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/stdio.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdio.h" 1 3 4
# 71 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdio.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/Availability.h" 1 3 4
# 72 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdio.h" 2 3 4

# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_types.h" 1 3 4
# 43 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_types.h" 3 4
typedef int __darwin_nl_item;
typedef int __darwin_wctrans_t;

typedef __uint32_t __darwin_wctype_t;
# 74 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdio.h" 2 3 4



# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_va_list.h" 1 3 4
# 44 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_va_list.h" 3 4
typedef __darwin_va_list va_list;
# 78 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdio.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_size_t.h" 1 3 4
# 79 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdio.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_null.h" 1 3 4
# 80 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdio.h" 2 3 4

# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/stdio.h" 1 3 4
# 43 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/stdio.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/Availability.h" 1 3 4
# 44 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/stdio.h" 2 3 4



int renameat(int, const char *, int, const char *) __attribute__((availability(macosx,introduced=10.10)));



int renamex_np(const char *, const char *, unsigned int) __attribute__((availability(macosx,introduced=10.12))) __attribute__((availability(ios,introduced=10.0))) __attribute__((availability(tvos,introduced=10.0))) __attribute__((availability(watchos,introduced=3.0)));
int renameatx_np(int, const char *, int, const char *, unsigned int) __attribute__((availability(macosx,introduced=10.12))) __attribute__((availability(ios,introduced=10.0))) __attribute__((availability(tvos,introduced=10.0))) __attribute__((availability(watchos,introduced=3.0)));
# 82 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdio.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_printf.h" 1 3 4
# 34 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_printf.h" 3 4
int printf(const char * restrict, ...) __attribute__((__format__ (__printf__, 1, 2)));
# 83 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdio.h" 2 3 4



typedef __darwin_off_t fpos_t;
# 97 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdio.h" 3 4
struct __sbuf {
 unsigned char * _base;
 int _size;
};


struct __sFILEX;
# 131 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdio.h" 3 4
typedef struct __sFILE {
 unsigned char * _p;
 int _r;
 int _w;
 short _flags;
 short _file;
 struct __sbuf _bf;
 int _lbfsize;


 void *_cookie;
 int (* _Nullable _close)(void *);
 int (* _Nullable _read) (void *, char *, int __n);
 fpos_t (* _Nullable _seek) (void *, fpos_t, int);
 int (* _Nullable _write)(void *, const char *, int __n);


 struct __sbuf _ub;
 struct __sFILEX *_extra;
 int _ur;


 unsigned char _ubuf[3];
 unsigned char _nbuf[1];


 struct __sbuf _lb;


 int _blksize;
 fpos_t _offset;
} FILE;




extern FILE *__stdinp __attribute__((__swift_attr__("nonisolated(unsafe)")));
extern FILE *__stdoutp __attribute__((__swift_attr__("nonisolated(unsafe)")));
extern FILE *__stderrp __attribute__((__swift_attr__("nonisolated(unsafe)")));
# 232 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdio.h" 3 4
void clearerr(FILE *);
int fclose(FILE *);
int feof(FILE *);
int ferror(FILE *);
int fflush(FILE *);
int fgetc(FILE *);
int fgetpos(FILE * restrict, fpos_t *);
char * fgets(char * restrict , int __size, FILE *);



FILE *fopen(const char * restrict __filename, const char * restrict __mode) __asm("_" "fopen" );

int fprintf(FILE * restrict, const char * restrict, ...) __attribute__((__format__ (__printf__, 2, 3)));
int fputc(int, FILE *);
int fputs(const char * restrict, FILE * restrict) __asm("_" "fputs" );
size_t fread(void * restrict __ptr, size_t __size, size_t __nitems, FILE * restrict __stream);
FILE *freopen(const char * restrict, const char * restrict,
     FILE * restrict) __asm("_" "freopen" );
int fscanf(FILE * restrict, const char * restrict, ...) __attribute__((__format__ (__scanf__, 2, 3)));
int fseek(FILE *, long, int);
int fsetpos(FILE *, const fpos_t *);
long ftell(FILE *);
size_t fwrite(const void * restrict __ptr, size_t __size, size_t __nitems, FILE * restrict __stream) __asm("_" "fwrite" );
int getc(FILE *);
int getchar(void);


__attribute__((__deprecated__("This function is provided for compatibility reasons only.  Due to security concerns inherent in the design of gets(3), it is highly recommended that you use fgets(3) instead.")))

char * gets(char *) ;

void perror(const char *) __attribute__((__cold__));
int putc(int, FILE *);
int putchar(int);
int puts(const char *);
int remove(const char *);
int rename (const char *__old, const char *__new);
void rewind(FILE *);
int scanf(const char * restrict, ...) __attribute__((__format__ (__scanf__, 1, 2)));
void setbuf(FILE * restrict, char * restrict );
int setvbuf(FILE * restrict, char * restrict , int, size_t __size);

__attribute__((__availability__(swift, unavailable, message="Use snprintf instead.")))


__attribute__((__deprecated__("This function is provided for compatibility reasons only.  Due to security concerns inherent in the design of sprintf(3), it is highly recommended that you use snprintf(3) instead.")))

int sprintf(char * restrict , const char * restrict, ...) __attribute__((__format__ (__printf__, 2, 3))) ;

int sscanf(const char * restrict, const char * restrict, ...) __attribute__((__format__ (__scanf__, 2, 3)));
FILE *tmpfile(void);

__attribute__((__availability__(swift, unavailable, message="Use mkstemp(3) instead.")))

__attribute__((__deprecated__("This function is provided for compatibility reasons only.  Due to security concerns inherent in the design of tmpnam(3), it is highly recommended that you use mkstemp(3) instead.")))

char * tmpnam(char *);

int ungetc(int, FILE *);
int vfprintf(FILE * restrict, const char * restrict, va_list) __attribute__((__format__ (__printf__, 2, 0)));
int vprintf(const char * restrict, va_list) __attribute__((__format__ (__printf__, 1, 0)));

__attribute__((__availability__(swift, unavailable, message="Use vsnprintf instead.")))


__attribute__((__deprecated__("This function is provided for compatibility reasons only.  Due to security concerns inherent in the design of sprintf(3), it is highly recommended that you use vsnprintf(3) instead.")))

int vsprintf(char * restrict , const char * restrict, va_list) __attribute__((__format__ (__printf__, 2, 0))) ;
# 311 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdio.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_ctermid.h" 1 3 4
# 36 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_ctermid.h" 3 4
char * ctermid(char *);
# 312 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdio.h" 2 3 4






FILE *fdopen(int, const char *) __asm("_" "fdopen" );

int fileno(FILE *);
# 331 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdio.h" 3 4
int pclose(FILE *) __attribute__((__availability__(swift, unavailable, message="Use posix_spawn APIs or NSTask instead. (On iOS, process spawning is unavailable.)")));



FILE *popen(const char *, const char *) __asm("_" "popen" ) __attribute__((__availability__(swift, unavailable, message="Use posix_spawn APIs or NSTask instead. (On iOS, process spawning is unavailable.)")));
# 350 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdio.h" 3 4
int __srget(FILE *);
int __svfscanf(FILE *, const char *, va_list) __attribute__((__format__ (__scanf__, 2, 0)));
int __swbuf(int, FILE *);
# 361 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdio.h" 3 4
inline __attribute__ ((__always_inline__)) int __sputc(int _c, FILE *_p) {
 if (--_p->_w >= 0 || (_p->_w >= _p->_lbfsize && (char)_c != '\n'))
  return (*_p->_p++ = _c);
 else
  return (__swbuf(_c, _p));
}
# 387 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdio.h" 3 4
void flockfile(FILE *);
int ftrylockfile(FILE *);
void funlockfile(FILE *);
int getc_unlocked(FILE *);
int getchar_unlocked(void);
int putc_unlocked(int, FILE *);
int putchar_unlocked(int);



int getw(FILE *);
int putw(int, FILE *);


__attribute__((__availability__(swift, unavailable, message="Use mkstemp(3) instead.")))

__attribute__((__deprecated__("This function is provided for compatibility reasons only.  Due to security concerns inherent in the design of tempnam(3), it is highly recommended that you use mkstemp(3) instead.")))

char * tempnam(const char *__dir, const char *__prefix) __asm("_" "tempnam" );
# 428 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdio.h" 3 4
int fseeko(FILE * __stream, off_t __offset, int __whence);
off_t ftello(FILE * __stream);





int snprintf(char * restrict __str, size_t __size, const char * restrict __format, ...) __attribute__((__format__ (__printf__, 3, 4)));
int vfscanf(FILE * restrict __stream, const char * restrict __format, va_list) __attribute__((__format__ (__scanf__, 2, 0)));
int vscanf(const char * restrict __format, va_list) __attribute__((__format__ (__scanf__, 1, 0)));
int vsnprintf(char * restrict __str, size_t __size, const char * restrict __format, va_list) __attribute__((__format__ (__printf__, 3, 0)));
int vsscanf(const char * restrict __str, const char * restrict __format, va_list) __attribute__((__format__ (__scanf__, 2, 0)));
# 450 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdio.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_ssize_t.h" 1 3 4
# 31 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_ssize_t.h" 3 4
typedef __darwin_ssize_t ssize_t;
# 451 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdio.h" 2 3 4


int dprintf(int, const char * restrict, ...) __attribute__((__format__ (__printf__, 2, 3))) __attribute__((availability(macosx,introduced=10.7)));
int vdprintf(int, const char * restrict, va_list) __attribute__((__format__ (__printf__, 2, 0))) __attribute__((availability(macosx,introduced=10.7)));
ssize_t getdelim(char * *restrict __linep, size_t * restrict __linecapp, int __delimiter, FILE * restrict __stream) __attribute__((availability(macosx,introduced=10.7)));
ssize_t getline(char * *restrict __linep, size_t * restrict __linecapp, FILE * restrict __stream) __attribute__((availability(macosx,introduced=10.7)));
FILE *fmemopen(void * restrict __buf , size_t __size, const char * restrict __mode) __attribute__((availability(macos,introduced=10.13))) __attribute__((availability(ios,introduced=11.0))) __attribute__((availability(tvos,introduced=11.0))) __attribute__((availability(watchos,introduced=4.0)));
FILE *open_memstream(char * *__bufp, size_t *__sizep) __attribute__((availability(macos,introduced=10.13))) __attribute__((availability(ios,introduced=11.0))) __attribute__((availability(tvos,introduced=11.0))) __attribute__((availability(watchos,introduced=4.0)));
# 468 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdio.h" 3 4
extern const int sys_nerr;
extern const char *const sys_errlist[];

int asprintf(char * *restrict, const char * restrict, ...) __attribute__((__format__ (__printf__, 2, 3)));
char * ctermid_r(char *);
char * fgetln(FILE *, size_t *__len);
const char *fmtcheck(const char *, const char *) __attribute__((format_arg(2)));
int fpurge(FILE *);
void setbuffer(FILE *, char *, int __size);
int setlinebuf(FILE *);
int vasprintf(char * *restrict, const char * restrict, va_list) __attribute__((__format__ (__printf__, 2, 0)));





FILE *funopen(const void *,
     int (* _Nullable)(void *, char *, int __n),
     int (* _Nullable)(void *, const char *, int __n),
     fpos_t (* _Nullable)(void *, fpos_t, int),
     int (* _Nullable)(void *));
# 503 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdio.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/secure/_stdio.h" 1 3 4
# 32 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/secure/_stdio.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/secure/_common.h" 1 3 4
# 33 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/secure/_stdio.h" 2 3 4
# 45 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/secure/_stdio.h" 3 4
extern int __sprintf_chk (char * restrict , int, size_t,
     const char * restrict, ...);
# 55 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/secure/_stdio.h" 3 4
extern int __snprintf_chk (char * restrict , size_t __maxlen, int, size_t,
      const char * restrict, ...);







extern int __vsprintf_chk (char * restrict , int, size_t,
      const char * restrict, va_list);







extern int __vsnprintf_chk (char * restrict , size_t __maxlen, int, size_t,
       const char * restrict, va_list);
# 504 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdio.h" 2 3 4
# 62 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/stdio.h" 2 3 4
# 10 "/Users/kevin/projects/lattice/salt-front/runtime.c" 2
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/stdlib.h" 1 3 4
# 58 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/stdlib.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdlib.h" 1 3 4
# 64 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdlib.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/Availability.h" 1 3 4
# 65 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdlib.h" 2 3 4





# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/wait.h" 1 3 4
# 79 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/wait.h" 3 4
typedef enum {
 P_ALL,
 P_PID,
 P_PGID
} idtype_t;






# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_id_t.h" 1 3 4
# 31 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_id_t.h" 3 4
typedef __darwin_id_t id_t;
# 91 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/wait.h" 2 3 4
# 109 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/wait.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/signal.h" 1 3 4
# 74 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/signal.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/Availability.h" 1 3 4
# 75 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/signal.h" 2 3 4







# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/machine/signal.h" 1 3 4
# 34 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/machine/signal.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/arm/signal.h" 1 3 4
# 17 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/arm/signal.h" 3 4
typedef int sig_atomic_t;
# 35 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/machine/signal.h" 2 3 4
# 83 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/signal.h" 2 3 4
# 146 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/signal.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/machine/_mcontext.h" 1 3 4
# 34 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/machine/_mcontext.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/arm/_mcontext.h" 1 3 4
# 41 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/arm/_mcontext.h" 3 4
struct __darwin_mcontext32
{
 struct __darwin_arm_exception_state __es;
 struct __darwin_arm_thread_state __ss;
 struct __darwin_arm_vfp_state __fs;
};
# 64 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/arm/_mcontext.h" 3 4
struct __darwin_mcontext64
{
 struct __darwin_arm_exception_state64 __es;
 struct __darwin_arm_thread_state64 __ss;
 struct __darwin_arm_neon_state64 __ns;
};
# 85 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/arm/_mcontext.h" 3 4
typedef struct __darwin_mcontext64 *mcontext_t;
# 35 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/machine/_mcontext.h" 2 3 4
# 147 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/signal.h" 2 3 4

# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_pthread/_pthread_attr_t.h" 1 3 4
# 31 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_pthread/_pthread_attr_t.h" 3 4
typedef __darwin_pthread_attr_t pthread_attr_t;
# 149 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/signal.h" 2 3 4

# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_sigaltstack.h" 1 3 4
# 42 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_sigaltstack.h" 3 4
struct __darwin_sigaltstack
{
 void *ss_sp;
 __darwin_size_t ss_size;
 int ss_flags;
};
typedef struct __darwin_sigaltstack stack_t;
# 151 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/signal.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_ucontext.h" 1 3 4
# 43 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_ucontext.h" 3 4
struct __darwin_ucontext
{
 int uc_onstack;
 __darwin_sigset_t uc_sigmask;
 struct __darwin_sigaltstack uc_stack;
 struct __darwin_ucontext *uc_link;
 __darwin_size_t uc_mcsize;
 struct __darwin_mcontext64 *uc_mcontext;



};


typedef struct __darwin_ucontext ucontext_t;
# 152 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/signal.h" 2 3 4


# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_sigset_t.h" 1 3 4
# 31 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_sigset_t.h" 3 4
typedef __darwin_sigset_t sigset_t;
# 155 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/signal.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_size_t.h" 1 3 4
# 156 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/signal.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_uid_t.h" 1 3 4
# 31 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_uid_t.h" 3 4
typedef __darwin_uid_t uid_t;
# 157 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/signal.h" 2 3 4

union sigval {

 int sival_int;
 void *sival_ptr;
};





struct sigevent {
 int sigev_notify;
 int sigev_signo;
 union sigval sigev_value;
 void (*sigev_notify_function)(union sigval);
 pthread_attr_t *sigev_notify_attributes;
};


typedef struct __siginfo {
 int si_signo;
 int si_errno;
 int si_code;
 pid_t si_pid;
 uid_t si_uid;
 int si_status;
 void *si_addr;
 union sigval si_value;
 long si_band;
 unsigned long __pad[7];
} siginfo_t;
# 269 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/signal.h" 3 4
union __sigaction_u {
 void (*__sa_handler)(int);
 void (*__sa_sigaction)(int, struct __siginfo *,
     void *);
};


struct __sigaction {
 union __sigaction_u __sigaction_u;
 void (*sa_tramp)(void *, int, int, siginfo_t *, void *);
 sigset_t sa_mask;
 int sa_flags;
};




struct sigaction {
 union __sigaction_u __sigaction_u;
 sigset_t sa_mask;
 int sa_flags;
};
# 331 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/signal.h" 3 4
typedef void (*sig_t)(int);
# 348 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/signal.h" 3 4
struct sigvec {
 void (*sv_handler)(int);
 int sv_mask;
 int sv_flags;
};
# 367 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/signal.h" 3 4
struct sigstack {
 char *ss_sp;
 int ss_onstack;
};
# 390 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/signal.h" 3 4
void(*signal(int, void (*)(int)))(int);
# 110 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/wait.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/resource.h" 1 3 4
# 75 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/resource.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/Availability.h" 1 3 4
# 76 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/resource.h" 2 3 4




# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_timeval.h" 1 3 4
# 34 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_timeval.h" 3 4
struct timeval
{
 __darwin_time_t tv_sec;
 __darwin_suseconds_t tv_usec;
};
# 81 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/resource.h" 2 3 4








typedef __uint64_t rlim_t;
# 152 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/resource.h" 3 4
struct rusage {
 struct timeval ru_utime;
 struct timeval ru_stime;
# 163 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/resource.h" 3 4
 long ru_maxrss;

 long ru_ixrss;
 long ru_idrss;
 long ru_isrss;
 long ru_minflt;
 long ru_majflt;
 long ru_nswap;
 long ru_inblock;
 long ru_oublock;
 long ru_msgsnd;
 long ru_msgrcv;
 long ru_nsignals;
 long ru_nvcsw;
 long ru_nivcsw;


};
# 200 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/resource.h" 3 4
typedef void *rusage_info_t;

struct rusage_info_v0 {
 uint8_t ri_uuid[16];
 uint64_t ri_user_time;
 uint64_t ri_system_time;
 uint64_t ri_pkg_idle_wkups;
 uint64_t ri_interrupt_wkups;
 uint64_t ri_pageins;
 uint64_t ri_wired_size;
 uint64_t ri_resident_size;
 uint64_t ri_phys_footprint;
 uint64_t ri_proc_start_abstime;
 uint64_t ri_proc_exit_abstime;
};

struct rusage_info_v1 {
 uint8_t ri_uuid[16];
 uint64_t ri_user_time;
 uint64_t ri_system_time;
 uint64_t ri_pkg_idle_wkups;
 uint64_t ri_interrupt_wkups;
 uint64_t ri_pageins;
 uint64_t ri_wired_size;
 uint64_t ri_resident_size;
 uint64_t ri_phys_footprint;
 uint64_t ri_proc_start_abstime;
 uint64_t ri_proc_exit_abstime;
 uint64_t ri_child_user_time;
 uint64_t ri_child_system_time;
 uint64_t ri_child_pkg_idle_wkups;
 uint64_t ri_child_interrupt_wkups;
 uint64_t ri_child_pageins;
 uint64_t ri_child_elapsed_abstime;
};

struct rusage_info_v2 {
 uint8_t ri_uuid[16];
 uint64_t ri_user_time;
 uint64_t ri_system_time;
 uint64_t ri_pkg_idle_wkups;
 uint64_t ri_interrupt_wkups;
 uint64_t ri_pageins;
 uint64_t ri_wired_size;
 uint64_t ri_resident_size;
 uint64_t ri_phys_footprint;
 uint64_t ri_proc_start_abstime;
 uint64_t ri_proc_exit_abstime;
 uint64_t ri_child_user_time;
 uint64_t ri_child_system_time;
 uint64_t ri_child_pkg_idle_wkups;
 uint64_t ri_child_interrupt_wkups;
 uint64_t ri_child_pageins;
 uint64_t ri_child_elapsed_abstime;
 uint64_t ri_diskio_bytesread;
 uint64_t ri_diskio_byteswritten;
};

struct rusage_info_v3 {
 uint8_t ri_uuid[16];
 uint64_t ri_user_time;
 uint64_t ri_system_time;
 uint64_t ri_pkg_idle_wkups;
 uint64_t ri_interrupt_wkups;
 uint64_t ri_pageins;
 uint64_t ri_wired_size;
 uint64_t ri_resident_size;
 uint64_t ri_phys_footprint;
 uint64_t ri_proc_start_abstime;
 uint64_t ri_proc_exit_abstime;
 uint64_t ri_child_user_time;
 uint64_t ri_child_system_time;
 uint64_t ri_child_pkg_idle_wkups;
 uint64_t ri_child_interrupt_wkups;
 uint64_t ri_child_pageins;
 uint64_t ri_child_elapsed_abstime;
 uint64_t ri_diskio_bytesread;
 uint64_t ri_diskio_byteswritten;
 uint64_t ri_cpu_time_qos_default;
 uint64_t ri_cpu_time_qos_maintenance;
 uint64_t ri_cpu_time_qos_background;
 uint64_t ri_cpu_time_qos_utility;
 uint64_t ri_cpu_time_qos_legacy;
 uint64_t ri_cpu_time_qos_user_initiated;
 uint64_t ri_cpu_time_qos_user_interactive;
 uint64_t ri_billed_system_time;
 uint64_t ri_serviced_system_time;
};

struct rusage_info_v4 {
 uint8_t ri_uuid[16];
 uint64_t ri_user_time;
 uint64_t ri_system_time;
 uint64_t ri_pkg_idle_wkups;
 uint64_t ri_interrupt_wkups;
 uint64_t ri_pageins;
 uint64_t ri_wired_size;
 uint64_t ri_resident_size;
 uint64_t ri_phys_footprint;
 uint64_t ri_proc_start_abstime;
 uint64_t ri_proc_exit_abstime;
 uint64_t ri_child_user_time;
 uint64_t ri_child_system_time;
 uint64_t ri_child_pkg_idle_wkups;
 uint64_t ri_child_interrupt_wkups;
 uint64_t ri_child_pageins;
 uint64_t ri_child_elapsed_abstime;
 uint64_t ri_diskio_bytesread;
 uint64_t ri_diskio_byteswritten;
 uint64_t ri_cpu_time_qos_default;
 uint64_t ri_cpu_time_qos_maintenance;
 uint64_t ri_cpu_time_qos_background;
 uint64_t ri_cpu_time_qos_utility;
 uint64_t ri_cpu_time_qos_legacy;
 uint64_t ri_cpu_time_qos_user_initiated;
 uint64_t ri_cpu_time_qos_user_interactive;
 uint64_t ri_billed_system_time;
 uint64_t ri_serviced_system_time;
 uint64_t ri_logical_writes;
 uint64_t ri_lifetime_max_phys_footprint;
 uint64_t ri_instructions;
 uint64_t ri_cycles;
 uint64_t ri_billed_energy;
 uint64_t ri_serviced_energy;
 uint64_t ri_interval_max_phys_footprint;
 uint64_t ri_runnable_time;
};

struct rusage_info_v5 {
 uint8_t ri_uuid[16];
 uint64_t ri_user_time;
 uint64_t ri_system_time;
 uint64_t ri_pkg_idle_wkups;
 uint64_t ri_interrupt_wkups;
 uint64_t ri_pageins;
 uint64_t ri_wired_size;
 uint64_t ri_resident_size;
 uint64_t ri_phys_footprint;
 uint64_t ri_proc_start_abstime;
 uint64_t ri_proc_exit_abstime;
 uint64_t ri_child_user_time;
 uint64_t ri_child_system_time;
 uint64_t ri_child_pkg_idle_wkups;
 uint64_t ri_child_interrupt_wkups;
 uint64_t ri_child_pageins;
 uint64_t ri_child_elapsed_abstime;
 uint64_t ri_diskio_bytesread;
 uint64_t ri_diskio_byteswritten;
 uint64_t ri_cpu_time_qos_default;
 uint64_t ri_cpu_time_qos_maintenance;
 uint64_t ri_cpu_time_qos_background;
 uint64_t ri_cpu_time_qos_utility;
 uint64_t ri_cpu_time_qos_legacy;
 uint64_t ri_cpu_time_qos_user_initiated;
 uint64_t ri_cpu_time_qos_user_interactive;
 uint64_t ri_billed_system_time;
 uint64_t ri_serviced_system_time;
 uint64_t ri_logical_writes;
 uint64_t ri_lifetime_max_phys_footprint;
 uint64_t ri_instructions;
 uint64_t ri_cycles;
 uint64_t ri_billed_energy;
 uint64_t ri_serviced_energy;
 uint64_t ri_interval_max_phys_footprint;
 uint64_t ri_runnable_time;
 uint64_t ri_flags;
};

struct rusage_info_v6 {
 uint8_t ri_uuid[16];
 uint64_t ri_user_time;
 uint64_t ri_system_time;
 uint64_t ri_pkg_idle_wkups;
 uint64_t ri_interrupt_wkups;
 uint64_t ri_pageins;
 uint64_t ri_wired_size;
 uint64_t ri_resident_size;
 uint64_t ri_phys_footprint;
 uint64_t ri_proc_start_abstime;
 uint64_t ri_proc_exit_abstime;
 uint64_t ri_child_user_time;
 uint64_t ri_child_system_time;
 uint64_t ri_child_pkg_idle_wkups;
 uint64_t ri_child_interrupt_wkups;
 uint64_t ri_child_pageins;
 uint64_t ri_child_elapsed_abstime;
 uint64_t ri_diskio_bytesread;
 uint64_t ri_diskio_byteswritten;
 uint64_t ri_cpu_time_qos_default;
 uint64_t ri_cpu_time_qos_maintenance;
 uint64_t ri_cpu_time_qos_background;
 uint64_t ri_cpu_time_qos_utility;
 uint64_t ri_cpu_time_qos_legacy;
 uint64_t ri_cpu_time_qos_user_initiated;
 uint64_t ri_cpu_time_qos_user_interactive;
 uint64_t ri_billed_system_time;
 uint64_t ri_serviced_system_time;
 uint64_t ri_logical_writes;
 uint64_t ri_lifetime_max_phys_footprint;
 uint64_t ri_instructions;
 uint64_t ri_cycles;
 uint64_t ri_billed_energy;
 uint64_t ri_serviced_energy;
 uint64_t ri_interval_max_phys_footprint;
 uint64_t ri_runnable_time;
 uint64_t ri_flags;
 uint64_t ri_user_ptime;
 uint64_t ri_system_ptime;
 uint64_t ri_pinstructions;
 uint64_t ri_pcycles;
 uint64_t ri_energy_nj;
 uint64_t ri_penergy_nj;
 uint64_t ri_secure_time_in_system;
 uint64_t ri_secure_ptime_in_system;
 uint64_t ri_neural_footprint;
 uint64_t ri_lifetime_max_neural_footprint;
 uint64_t ri_interval_max_neural_footprint;
 uint64_t ri_reserved[9];
};

typedef struct rusage_info_v6 rusage_info_current;
# 464 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/resource.h" 3 4
struct rlimit {
 rlim_t rlim_cur;
 rlim_t rlim_max;
};
# 499 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/resource.h" 3 4
struct proc_rlimit_control_wakeupmon {
 uint32_t wm_flags;
 int32_t wm_rate;
};
# 571 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/resource.h" 3 4
int getpriority(int, id_t);

int getiopolicy_np(int, int) __attribute__((availability(macosx,introduced=10.5)));

int getrlimit(int, struct rlimit *) __asm("_" "getrlimit" );
int getrusage(int, struct rusage *);
int setpriority(int, id_t, int);

int setiopolicy_np(int, int, int) __attribute__((availability(macosx,introduced=10.5)));

int setrlimit(int, const struct rlimit *) __asm("_" "setrlimit" );
# 111 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/wait.h" 2 3 4
# 186 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/wait.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/machine/endian.h" 1 3 4
# 37 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/machine/endian.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/arm/endian.h" 1 3 4
# 61 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/arm/endian.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_endian.h" 1 3 4
# 94 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_endian.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/machine/_endian.h" 1 3 4
# 37 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/machine/_endian.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/arm/_endian.h" 1 3 4
# 95 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/arm/_endian.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/__endian.h" 1 3 4
# 96 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/arm/_endian.h" 2 3 4
# 38 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/machine/_endian.h" 2 3 4
# 95 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_endian.h" 2 3 4
# 131 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_endian.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/libkern/_OSByteOrder.h" 1 3 4
# 62 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/libkern/_OSByteOrder.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/libkern/arm/_OSByteOrder.h" 1 3 4
# 48 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/libkern/arm/_OSByteOrder.h" 3 4
static inline
__uint16_t
_OSSwapInt16(
 __uint16_t _data
 )
{

 return (__uint16_t)(_data << 8 | _data >> 8);
}

static inline
__uint32_t
_OSSwapInt32(
 __uint32_t _data
 )
{

 _data = __builtin_bswap32(_data);





 return _data;
}

static inline
__uint64_t
_OSSwapInt64(
 __uint64_t _data
 )
{

 return __builtin_bswap64(_data);
# 95 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/libkern/arm/_OSByteOrder.h" 3 4
}
# 63 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/libkern/_OSByteOrder.h" 2 3 4
# 132 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_endian.h" 2 3 4
# 62 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/arm/endian.h" 2 3 4
# 38 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/machine/endian.h" 2 3 4
# 187 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/wait.h" 2 3 4







union wait {
 int w_status;



 struct {

  unsigned int w_Termsig:7,
      w_Coredump:1,
      w_Retcode:8,
      w_Filler:16;






 } w_T;





 struct {

  unsigned int w_Stopval:8,
      w_Stopsig:8,
      w_Filler:16;





 } w_S;
};
# 246 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/wait.h" 3 4
pid_t wait(int *) __asm("_" "wait" );
pid_t waitpid(pid_t, int *, int) __asm("_" "waitpid" );

int waitid(idtype_t, id_t, siginfo_t *, int) __asm("_" "waitid" );


pid_t wait3(int *, int, struct rusage *);
pid_t wait4(pid_t, int *, int, struct rusage *);
# 71 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdlib.h" 2 3 4

# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/alloca.h" 1 3 4
# 30 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/alloca.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_size_t.h" 1 3 4
# 31 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/alloca.h" 2 3 4




void * alloca(size_t __size);
# 73 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdlib.h" 2 3 4
# 91 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdlib.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_size_t.h" 1 3 4
# 92 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdlib.h" 2 3 4


# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_ct_rune_t.h" 1 3 4
# 32 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_ct_rune_t.h" 3 4
typedef __darwin_ct_rune_t ct_rune_t;
# 95 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdlib.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_rune_t.h" 1 3 4
# 31 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_rune_t.h" 3 4
typedef __darwin_rune_t rune_t;
# 96 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdlib.h" 2 3 4


# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_wchar_t.h" 1 3 4
# 99 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdlib.h" 2 3 4



typedef struct {
 int quot;
 int rem;
} div_t;

typedef struct {
 long quot;
 long rem;
} ldiv_t;


typedef struct {
 long long quot;
 long long rem;
} lldiv_t;


# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_null.h" 1 3 4
# 120 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdlib.h" 2 3 4
# 134 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdlib.h" 3 4
extern int __mb_cur_max;




# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/malloc/_malloc.h" 1 3 4
# 33 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/malloc/_malloc.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/Availability.h" 1 3 4
# 34 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/malloc/_malloc.h" 2 3 4


# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_size_t.h" 1 3 4
# 37 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/malloc/_malloc.h" 2 3 4







# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/malloc/_malloc_type.h" 1 3 4
# 27 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/malloc/_malloc_type.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/malloc/_ptrcheck.h" 1 3 4
# 28 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/malloc/_malloc_type.h" 2 3 4


typedef unsigned long long malloc_type_id_t;




# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/Availability.h" 1 3 4
# 36 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/malloc/_malloc_type.h" 2 3 4

# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_size_t.h" 1 3 4
# 38 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/malloc/_malloc_type.h" 2 3 4
# 51 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/malloc/_malloc_type.h" 3 4
__attribute__((availability(macos,introduced=14.0))) __attribute__((availability(ios,introduced=17.0))) __attribute__((availability(tvos,introduced=17.0))) __attribute__((availability(watchos,introduced=10.0))) __attribute__((availability(visionos,introduced=1.0))) __attribute__((availability(driverkit,introduced=23.0))) void * malloc_type_malloc(size_t size, malloc_type_id_t type_id) __attribute__((__warn_unused_result__)) __attribute__((alloc_size(1)));
__attribute__((availability(macos,introduced=14.0))) __attribute__((availability(ios,introduced=17.0))) __attribute__((availability(tvos,introduced=17.0))) __attribute__((availability(watchos,introduced=10.0))) __attribute__((availability(visionos,introduced=1.0))) __attribute__((availability(driverkit,introduced=23.0))) void * malloc_type_calloc(size_t count, size_t size, malloc_type_id_t type_id) __attribute__((__warn_unused_result__)) __attribute__((alloc_size(1,2)));
__attribute__((availability(macos,introduced=14.0))) __attribute__((availability(ios,introduced=17.0))) __attribute__((availability(tvos,introduced=17.0))) __attribute__((availability(watchos,introduced=10.0))) __attribute__((availability(visionos,introduced=1.0))) __attribute__((availability(driverkit,introduced=23.0))) void malloc_type_free(void * ptr, malloc_type_id_t type_id);
__attribute__((availability(macos,introduced=14.0))) __attribute__((availability(ios,introduced=17.0))) __attribute__((availability(tvos,introduced=17.0))) __attribute__((availability(watchos,introduced=10.0))) __attribute__((availability(visionos,introduced=1.0))) __attribute__((availability(driverkit,introduced=23.0))) void * malloc_type_realloc(void * ptr, size_t size, malloc_type_id_t type_id) __attribute__((__warn_unused_result__)) __attribute__((alloc_size(2)));
__attribute__((availability(macos,introduced=14.0))) __attribute__((availability(ios,introduced=17.0))) __attribute__((availability(tvos,introduced=17.0))) __attribute__((availability(watchos,introduced=10.0))) __attribute__((availability(visionos,introduced=1.0))) __attribute__((availability(driverkit,introduced=23.0))) void * malloc_type_valloc(size_t size, malloc_type_id_t type_id) __attribute__((__warn_unused_result__)) __attribute__((alloc_size(1)));
__attribute__((availability(macos,introduced=14.0))) __attribute__((availability(ios,introduced=17.0))) __attribute__((availability(tvos,introduced=17.0))) __attribute__((availability(watchos,introduced=10.0))) __attribute__((availability(visionos,introduced=1.0))) __attribute__((availability(driverkit,introduced=23.0))) void * malloc_type_aligned_alloc(size_t alignment, size_t size, malloc_type_id_t type_id) __attribute__((__warn_unused_result__)) __attribute__((alloc_size(2)));

__attribute__((availability(macos,introduced=14.0))) __attribute__((availability(ios,introduced=17.0))) __attribute__((availability(tvos,introduced=17.0))) __attribute__((availability(watchos,introduced=10.0))) __attribute__((availability(visionos,introduced=1.0))) __attribute__((availability(driverkit,introduced=23.0))) int malloc_type_posix_memalign(void * *memptr, size_t alignment, size_t size, malloc_type_id_t type_id) ;




typedef struct _malloc_zone_t malloc_zone_t;

__attribute__((availability(macos,introduced=14.0))) __attribute__((availability(ios,introduced=17.0))) __attribute__((availability(tvos,introduced=17.0))) __attribute__((availability(watchos,introduced=10.0))) __attribute__((availability(visionos,introduced=1.0))) __attribute__((availability(driverkit,introduced=23.0))) void * malloc_type_zone_malloc(malloc_zone_t *zone, size_t size, malloc_type_id_t type_id) __attribute__((__warn_unused_result__)) __attribute__((alloc_size(2)));
__attribute__((availability(macos,introduced=14.0))) __attribute__((availability(ios,introduced=17.0))) __attribute__((availability(tvos,introduced=17.0))) __attribute__((availability(watchos,introduced=10.0))) __attribute__((availability(visionos,introduced=1.0))) __attribute__((availability(driverkit,introduced=23.0))) void * malloc_type_zone_calloc(malloc_zone_t *zone, size_t count, size_t size, malloc_type_id_t type_id) __attribute__((__warn_unused_result__)) __attribute__((alloc_size(2,3)));
__attribute__((availability(macos,introduced=14.0))) __attribute__((availability(ios,introduced=17.0))) __attribute__((availability(tvos,introduced=17.0))) __attribute__((availability(watchos,introduced=10.0))) __attribute__((availability(visionos,introduced=1.0))) __attribute__((availability(driverkit,introduced=23.0))) void malloc_type_zone_free(malloc_zone_t *zone, void * ptr, malloc_type_id_t type_id);
__attribute__((availability(macos,introduced=14.0))) __attribute__((availability(ios,introduced=17.0))) __attribute__((availability(tvos,introduced=17.0))) __attribute__((availability(watchos,introduced=10.0))) __attribute__((availability(visionos,introduced=1.0))) __attribute__((availability(driverkit,introduced=23.0))) void * malloc_type_zone_realloc(malloc_zone_t *zone, void * ptr, size_t size, malloc_type_id_t type_id) __attribute__((__warn_unused_result__)) __attribute__((alloc_size(3)));
__attribute__((availability(macos,introduced=14.0))) __attribute__((availability(ios,introduced=17.0))) __attribute__((availability(tvos,introduced=17.0))) __attribute__((availability(watchos,introduced=10.0))) __attribute__((availability(visionos,introduced=1.0))) __attribute__((availability(driverkit,introduced=23.0))) void * malloc_type_zone_valloc(malloc_zone_t *zone, size_t size, malloc_type_id_t type_id) __attribute__((__warn_unused_result__)) __attribute__((alloc_size(2)));
__attribute__((availability(macos,introduced=14.0))) __attribute__((availability(ios,introduced=17.0))) __attribute__((availability(tvos,introduced=17.0))) __attribute__((availability(watchos,introduced=10.0))) __attribute__((availability(visionos,introduced=1.0))) __attribute__((availability(driverkit,introduced=23.0))) void * malloc_type_zone_memalign(malloc_zone_t *zone, size_t alignment, size_t size, malloc_type_id_t type_id) __attribute__((__warn_unused_result__)) __attribute__((alloc_size(3)));
# 45 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/malloc/_malloc.h" 2 3 4
# 54 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/malloc/_malloc.h" 3 4
void * malloc(size_t __size) __attribute__((__warn_unused_result__)) __attribute__((alloc_size(1))) ;
void * calloc(size_t __count, size_t __size) __attribute__((__warn_unused_result__)) __attribute__((alloc_size(1,2))) ;
void free(void * );
void * realloc(void * __ptr, size_t __size) __attribute__((__warn_unused_result__)) __attribute__((alloc_size(2))) ;
void * reallocf(void * __ptr, size_t __size) __attribute__((__warn_unused_result__)) __attribute__((alloc_size(2)));

void * valloc(size_t __size) __attribute__((__warn_unused_result__)) __attribute__((alloc_size(1))) ;




void * aligned_alloc(size_t __alignment, size_t __size) __attribute__((__warn_unused_result__)) __attribute__((alloc_align(1))) __attribute__((alloc_size(2))) __attribute__((availability(macosx,introduced=10.15))) __attribute__((availability(ios,introduced=13.0))) __attribute__((availability(tvos,introduced=13.0))) __attribute__((availability(watchos,introduced=6.0)));


int posix_memalign(void * *__memptr, size_t __alignment, size_t __size) __attribute__((availability(macosx,introduced=10.6)));
# 140 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdlib.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_abort.h" 1 3 4
# 33 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_abort.h" 3 4
void abort(void) __attribute__((__cold__)) __attribute__((__noreturn__));
# 141 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdlib.h" 2 3 4


int abs(int) __attribute__((__const__));
int atexit(void (* _Nonnull)(void));


int at_quick_exit(void (*)(void));

double atof(const char *);
int atoi(const char *);
long atol(const char *);

long long
  atoll(const char *);

void *bsearch(const void * __key, const void * __base, size_t __nel,
     size_t __width, int (* _Nonnull __compar)(const void *, const void *));

div_t div(int, int) __attribute__((__const__));
void exit(int) __attribute__((__noreturn__));

char * getenv(const char *);
long labs(long) __attribute__((__const__));
ldiv_t ldiv(long, long) __attribute__((__const__));

long long
  llabs(long long);
lldiv_t lldiv(long long, long long);


int mblen(const char * __s, size_t __n);
size_t mbstowcs(wchar_t * restrict , const char * restrict, size_t __n);
int mbtowc(wchar_t * restrict, const char * restrict , size_t __n);

void qsort(void * __base, size_t __nel, size_t __width,
     int (* _Nonnull __compar)(const void *, const void *));


void quick_exit(int) __attribute__((__noreturn__));

int rand(void) __attribute__((__availability__(swift, unavailable, message="Use arc4random instead.")));

void srand(unsigned) __attribute__((__availability__(swift, unavailable, message="Use arc4random instead.")));
double strtod(const char *, char * *) __asm("_" "strtod" );
float strtof(const char *, char * *) __asm("_" "strtof" );
long strtol(const char *__str, char * *__endptr, int __base);
long double
  strtold(const char *, char * *);

long long
  strtoll(const char *__str, char * *__endptr, int __base);

unsigned long
  strtoul(const char *__str, char * *__endptr, int __base);

unsigned long long
  strtoull(const char *__str, char * *__endptr, int __base);


__attribute__((__availability__(swift, unavailable, message="Use posix_spawn APIs or NSTask instead. (On iOS, process spawning is unavailable.)")))
__attribute__((availability(macos,introduced=10.0))) __attribute__((availability(ios,unavailable)))
__attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)))
int system(const char *) __asm("_" "system" );


size_t wcstombs(char * restrict , const wchar_t * restrict, size_t __n);
int wctomb(char *, wchar_t);


void _Exit(int) __attribute__((__noreturn__));
long a64l(const char *);
double drand48(void);
char * ecvt(double, int, int *restrict, int *restrict);
double erand48(unsigned short[3]);
char * fcvt(double, int, int *restrict, int *restrict);
char * gcvt(double, int, char *) ;
int getsubopt(char * *, char * const *, char * *);
int grantpt(int);

char *
  initstate(unsigned, char *, size_t __size);




long jrand48(unsigned short[3]) __attribute__((__availability__(swift, unavailable, message="Use arc4random instead.")));
char *l64a(long);
void lcong48(unsigned short[7]);
long lrand48(void) __attribute__((__availability__(swift, unavailable, message="Use arc4random instead.")));

__attribute__((__deprecated__("This function is provided for compatibility reasons only.  Due to security concerns inherent in the design of mktemp(3), it is highly recommended that you use mkstemp(3) instead.")))

char * mktemp(char *);
int mkstemp(char *);
long mrand48(void) __attribute__((__availability__(swift, unavailable, message="Use arc4random instead.")));
long nrand48(unsigned short[3]) __attribute__((__availability__(swift, unavailable, message="Use arc4random instead.")));
int posix_openpt(int);
char * ptsname(int);


int ptsname_r(int fildes, char * buffer, size_t buflen) __attribute__((availability(macos,introduced=10.13.4))) __attribute__((availability(ios,introduced=11.3))) __attribute__((availability(tvos,introduced=11.3))) __attribute__((availability(watchos,introduced=4.3)));


int putenv(char *) __asm("_" "putenv" );
long random(void) __attribute__((__availability__(swift, unavailable, message="Use arc4random instead.")));
int rand_r(unsigned *) __attribute__((__availability__(swift, unavailable, message="Use arc4random instead.")));

char * realpath(const char * restrict, char * restrict ) __asm("_" "realpath" "$DARWIN_EXTSN");



unsigned short * seed48(unsigned short[3]);
int setenv(const char * __name, const char * __value, int __overwrite) __asm("_" "setenv" );

void setkey(const char *) __asm("_" "setkey" );



char * setstate(const char *);
void srand48(long);

void srandom(unsigned);



int unlockpt(int);

int unsetenv(const char *) __asm("_" "unsetenv" );
# 277 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdlib.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_dev_t.h" 1 3 4
# 31 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_dev_t.h" 3 4
typedef __darwin_dev_t dev_t;
# 278 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdlib.h" 2 3 4




uint32_t arc4random(void);
void arc4random_addrandom(unsigned char * , int __datlen)
    __attribute__((availability(macosx,introduced=10.0))) __attribute__((availability(macosx,deprecated=10.12,message="use arc4random_stir")))
    __attribute__((availability(ios,introduced=2.0))) __attribute__((availability(ios,deprecated=10.0,message="use arc4random_stir")))
    __attribute__((availability(tvos,introduced=2.0))) __attribute__((availability(tvos,deprecated=10.0,message="use arc4random_stir")))
    __attribute__((availability(watchos,introduced=1.0))) __attribute__((availability(watchos,deprecated=3.0,message="use arc4random_stir")));
void arc4random_buf(void * __buf, size_t __nbytes) __attribute__((availability(macosx,introduced=10.7)));
void arc4random_stir(void);
uint32_t
  arc4random_uniform(uint32_t __upper_bound) __attribute__((availability(macosx,introduced=10.7)));

int atexit_b(void (^ _Nonnull)(void)) __attribute__((availability(macosx,introduced=10.6)));
# 302 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdlib.h" 3 4
void *bsearch_b(const void * __key, const void * __base, size_t __nel,
     size_t __width, int (^ _Nonnull __compar)(const void *, const void *) __attribute__((__noescape__)))
     __attribute__((availability(macosx,introduced=10.6)));



char * cgetcap(char *, const char *, int);
int cgetclose(void);
int cgetent(char * *, char * *, const char *);
int cgetfirst(char * *, char * *);
int cgetmatch(const char *, const char *);
int cgetnext(char * *, char * *);
int cgetnum(char *, const char *, long *);
int cgetset(const char *);
int cgetstr(char *, const char *, char * *);
int cgetustr(char *, const char *, char * *);

int daemon(int, int) __asm("_" "daemon" ) __attribute__((availability(macosx,introduced=10.0,deprecated=10.5,message="Use posix_spawn APIs instead."))) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));
char * devname(dev_t, mode_t);
char * devname_r(dev_t, mode_t, char * buf, int len);
char * getbsize(int *, long *);
int getloadavg(double [], int __nelem);
const char
 *getprogname(void);
void setprogname(const char *);
# 336 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_stdlib.h" 3 4
int heapsort(void * __base, size_t __nel, size_t __width,
     int (* _Nonnull __compar)(const void *, const void *));

int heapsort_b(void * __base, size_t __nel, size_t __width,
     int (^ _Nonnull __compar)(const void *, const void *) __attribute__((__noescape__)))
     __attribute__((availability(macosx,introduced=10.6)));

int mergesort(void * __base, size_t __nel, size_t __width,
     int (* _Nonnull __compar)(const void *, const void *));

int mergesort_b(void * __base, size_t __nel, size_t __width,
     int (^ _Nonnull __compar)(const void *, const void *) __attribute__((__noescape__)))
     __attribute__((availability(macosx,introduced=10.6)));

void psort(void * __base, size_t __nel, size_t __width,
     int (* _Nonnull __compar)(const void *, const void *))
     __attribute__((availability(macosx,introduced=10.6)));

void psort_b(void * __base, size_t __nel, size_t __width,
     int (^ _Nonnull __compar)(const void *, const void *) __attribute__((__noescape__)))
     __attribute__((availability(macosx,introduced=10.6)));

void psort_r(void * __base, size_t __nel, size_t __width, void *,
     int (* _Nonnull __compar)(void *, const void *, const void *))
     __attribute__((availability(macosx,introduced=10.6)));

void qsort_b(void * __base, size_t __nel, size_t __width,
     int (^ _Nonnull __compar)(const void *, const void *) __attribute__((__noescape__)))
     __attribute__((availability(macosx,introduced=10.6)));

void qsort_r(void * __base, size_t __nel, size_t __width, void *,
     int (* _Nonnull __compar)(void *, const void *, const void *));
int radixsort(const unsigned char * * __base, int __nel, const unsigned char * __table,
     unsigned __endbyte);
int rpmatch(const char *)
 __attribute__((availability(macos,introduced=10.15))) __attribute__((availability(ios,introduced=13.0))) __attribute__((availability(tvos,introduced=13.0))) __attribute__((availability(watchos,introduced=6.0)));
int sradixsort(const unsigned char * * __base, int __nel, const unsigned char * __table,
     unsigned __endbyte);
void sranddev(void);
void srandomdev(void);

long long
 strtonum(const char *__numstr, long long __minval, long long __maxval, const char * *__errstrp)
 __attribute__((availability(macos,introduced=11.0))) __attribute__((availability(ios,introduced=14.0))) __attribute__((availability(tvos,introduced=14.0))) __attribute__((availability(watchos,introduced=7.0)));

long long
  strtoq(const char *__str, char * *__endptr, int __base);
unsigned long long
  strtouq(const char *__str, char * *__endptr, int __base);

extern char * suboptarg;
# 59 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/stdlib.h" 2 3 4
# 11 "/Users/kevin/projects/lattice/salt-front/runtime.c" 2
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/string.h" 1 3 4
# 58 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/string.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_string.h" 1 3 4
# 64 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_string.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/Availability.h" 1 3 4
# 65 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_string.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_size_t.h" 1 3 4
# 66 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_string.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_null.h" 1 3 4
# 67 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_string.h" 2 3 4






void *
  memchr(const void * __s, int __c, size_t __n);
int memcmp(const void * __s1, const void * __s2,
  size_t __n);
void *
  memcpy(void * __dst, const void * __src,
  size_t __n);
void *
  memmove(void * __dst,
  const void * __src, size_t __len);
void *
  memset(void * __b, int __c, size_t __len);
char *
  strcat(char * __s1, const char *__s2)
                                  ;
char * strchr(const char *__s, int __c);
int strcmp(const char *__s1, const char *__s2);
int strcoll(const char *__s1, const char *__s2);
char *
  strcpy(char * __dst, const char *__src)
                                  ;
size_t strcspn(const char *__s, const char *__charset);
char * strerror(int __errnum) __asm("_" "strerror" );
size_t strlen(const char *__s);
char *
  strncat(char * __s1,
  const char * __s2, size_t __n)
                                  ;
int strncmp(const char * __s1,
  const char * __s2, size_t __n);
char *
  strncpy(char * __dst,
        const char * __src, size_t __n)
                                        ;
char * strpbrk(const char *__s, const char *__charset);
char * strrchr(const char *__s, int __c);
size_t strspn(const char *__s, const char *__charset);
char * strstr(const char *__big, const char *__little);
char * strtok(char * __str, const char *__sep);
size_t strxfrm(char * __s1, const char *__s2, size_t __n);
# 125 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_string.h" 3 4
char *
        strtok_r(char * __str, const char *__sep,
        char * *__lasts);
# 139 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_string.h" 3 4
int strerror_r(int __errnum, char * __strerrbuf,
        size_t __buflen);
char * strdup(const char *__s1);
void *
        memccpy(void * __dst, const void * __src,
        int __c, size_t __n);
# 156 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_string.h" 3 4
char *
        stpcpy(char * __dst, const char *__src) ;
char *
        stpncpy(char * __dst,
        const char * __src, size_t __n)
        __attribute__((availability(macosx,introduced=10.7)))
                                        ;
char * strndup(const char * __s1, size_t __n) __attribute__((availability(macosx,introduced=10.7)));
size_t strnlen(const char * __s1, size_t __n) __attribute__((availability(macosx,introduced=10.7)));
char * strsignal(int __sig);






# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_rsize_t.h" 1 3 4
# 173 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_string.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_errno_t.h" 1 3 4
# 30 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_errno_t.h" 3 4
typedef int errno_t;
# 174 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_string.h" 2 3 4


errno_t memset_s(void * __s, rsize_t __smax, int __c, rsize_t __n) __attribute__((availability(macosx,introduced=10.9)));
# 186 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_string.h" 3 4
void *
        memmem(const void * __big, size_t __big_len,
        const void * __little, size_t __little_len) __attribute__((availability(macosx,introduced=10.7)));
void memset_pattern4(void * __b, const void * __pattern4, size_t __len) __attribute__((availability(macosx,introduced=10.5)));
void memset_pattern8(void * __b, const void * __pattern8, size_t __len) __attribute__((availability(macosx,introduced=10.5)));
void memset_pattern16(void * __b, const void * __pattern16, size_t __len) __attribute__((availability(macosx,introduced=10.5)));

char *
        strcasestr(const char *__big, const char *__little);
__attribute__((availability(macosx,introduced=15.4))) __attribute__((availability(ios,introduced=18.4)))
__attribute__((availability(tvos,introduced=18.4))) __attribute__((availability(watchos,introduced=11.4)))
char *
        strchrnul(const char *__s, int __c);
char *
        strnstr(const char * __big, const char *__little, size_t __len);
size_t strlcat(char * __dst, const char *__source, size_t __size);
size_t strlcpy(char * __dst, const char *__source, size_t __size);
void strmode(int __mode, char * __bp);
char *
        strsep(char * *__stringp, const char *__delim);


void swab(const void * restrict, void * restrict, ssize_t __len);

__attribute__((availability(macosx,introduced=10.12.1))) __attribute__((availability(ios,introduced=10.1)))
__attribute__((availability(tvos,introduced=10.0.1))) __attribute__((availability(watchos,introduced=3.1)))
int timingsafe_bcmp(const void * __b1, const void * __b2, size_t __len);

__attribute__((availability(macosx,introduced=11.0))) __attribute__((availability(ios,introduced=14.0)))
__attribute__((availability(tvos,introduced=14.0))) __attribute__((availability(watchos,introduced=7.0)))
int strsignal_r(int __sig, char * __strsignalbuf, size_t __buflen);





# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_strings.h" 1 3 4
# 65 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_strings.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/Availability.h" 1 3 4
# 66 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_strings.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_size_t.h" 1 3 4
# 67 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_strings.h" 2 3 4






int bcmp(const void *, const void *, size_t __n) ;
void bcopy(const void *, void *, size_t __n) ;
void bzero(void *, size_t __n) ;
char * index(const char *, int) ;
char * rindex(const char *, int) ;


int ffs(int);
int strcasecmp(const char *, const char *);
int strncasecmp(const char *, const char *, size_t);





int ffsl(long) __attribute__((availability(macosx,introduced=10.5)));
int ffsll(long long) __attribute__((availability(macosx,introduced=10.9)));
int fls(int) __attribute__((availability(macosx,introduced=10.5)));
int flsl(long) __attribute__((availability(macosx,introduced=10.5)));
int flsll(long long) __attribute__((availability(macosx,introduced=10.9)));





# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/secure/_strings.h" 1 3 4
# 32 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/secure/_strings.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/Availability.h" 1 3 4
# 33 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/secure/_strings.h" 2 3 4
# 99 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_strings.h" 2 3 4
# 223 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_string.h" 2 3 4





# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/secure/_string.h" 1 3 4
# 32 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/secure/_string.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/Availability.h" 1 3 4
# 33 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/secure/_string.h" 2 3 4
# 229 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_string.h" 2 3 4
# 59 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/string.h" 2 3 4
# 12 "/Users/kevin/projects/lattice/salt-front/runtime.c" 2
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/mman.h" 1 3 4
# 90 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/mman.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_size_t.h" 1 3 4
# 91 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/mman.h" 2 3 4


# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/Availability.h" 1 3 4
# 94 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/mman.h" 2 3 4
# 242 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/mman.h" 3 4
int mlockall(int);
int munlockall(void);

int mlock(const void *, size_t);



void * mmap(void *, size_t, int, int, int, off_t) __asm("_" "mmap" );


int mprotect(void *, size_t, int) __asm("_" "mprotect" );

int msync(void *, size_t, int) __asm("_" "msync" );

int munlock(const void *, size_t);

int munmap(void *, size_t) __asm("_" "munmap" );

int shm_open(const char *, int, ...);
int shm_unlink(const char *);

int posix_madvise(void *, size_t, int);


int madvise(void *, size_t, int);
int mincore(const void *, size_t, char *);
int minherit(void *, size_t, int);
# 13 "/Users/kevin/projects/lattice/salt-front/runtime.c" 2
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/syscall.h" 1 3 4
# 14 "/Users/kevin/projects/lattice/salt-front/runtime.c" 2
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/unistd.h" 1 3 4
# 73 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/unistd.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/unistd.h" 1 3 4
# 84 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/unistd.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_posix_vdisable.h" 1 3 4
# 85 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/unistd.h" 2 3 4
# 132 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/unistd.h" 3 4
struct accessx_descriptor {
 unsigned int ad_name_offset;
 int ad_flags;
 int ad_pad[2];
};
# 180 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/unistd.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_size_t.h" 1 3 4
# 181 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/unistd.h" 2 3 4



# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/Availability.h" 1 3 4
# 185 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/unistd.h" 2 3 4



int getattrlistbulk(int, void *, void *, size_t, uint64_t) __attribute__((availability(macosx,introduced=10.10)));
int getattrlistat(int, const char *, void *, void *, size_t, unsigned long) __attribute__((availability(macosx,introduced=10.10)));
int setattrlistat(int, const char *, void *, void *, size_t, uint32_t) __attribute__((availability(macosx,introduced=10.13))) __attribute__((availability(ios,introduced=11.0))) __attribute__((availability(tvos,introduced=11.0))) __attribute__((availability(watchos,introduced=4.0)));
ssize_t freadlink(int, char * restrict, size_t) __attribute__((availability(macos,introduced=13.0))) __attribute__((availability(ios,introduced=16.0))) __attribute__((availability(tvos,introduced=16.0))) __attribute__((availability(watchos,introduced=9.0)));
# 200 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/unistd.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_size_t.h" 1 3 4
# 201 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/unistd.h" 2 3 4



# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_gid_t.h" 1 3 4
# 31 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_gid_t.h" 3 4
typedef __darwin_gid_t gid_t;
# 205 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/unistd.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/Availability.h" 1 3 4
# 206 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/unistd.h" 2 3 4



int faccessat(int, const char *, int, int) __attribute__((availability(macosx,introduced=10.10)));
int fchownat(int, const char *, uid_t, gid_t, int) __attribute__((availability(macosx,introduced=10.10)));
int linkat(int, const char *, int, const char *, int) __attribute__((availability(macosx,introduced=10.10)));
ssize_t readlinkat(int, const char *, char *, size_t) __attribute__((availability(macosx,introduced=10.10)));
int symlinkat(const char *, int, const char *) __attribute__((availability(macosx,introduced=10.10)));
int unlinkat(int, const char *, int) __attribute__((availability(macosx,introduced=10.10)));
# 74 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/unistd.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/Availability.h" 1 3 4
# 75 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/unistd.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/syslimits.h" 1 3 4
# 76 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/unistd.h" 2 3 4






# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_size_t.h" 1 3 4
# 83 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/unistd.h" 2 3 4


# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_useconds_t.h" 1 3 4
# 31 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_useconds_t.h" 3 4
typedef __darwin_useconds_t useconds_t;
# 86 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/unistd.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_null.h" 1 3 4
# 87 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/unistd.h" 2 3 4
# 434 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/unistd.h" 3 4
void _exit(int) __attribute__((__noreturn__));
int access(const char *, int);
unsigned int
  alarm(unsigned int);
int chdir(const char *);
int chown(const char *, uid_t, gid_t);

int close(int) __asm("_" "close" );

int dup(int);
int dup2(int, int);
int execl(const char * __path, const char * __arg0, ...) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));
int execle(const char * __path, const char * __arg0, ...) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));
int execlp(const char * __file, const char * __arg0, ...) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));
int execv(const char * __path, char * const * __argv) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));
int execve(const char * __file, char * const * __argv, char * const * __envp) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));
int execvp(const char * __file, char * const * __argv) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));
pid_t fork(void) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));
long fpathconf(int, int);
char * getcwd(char *, size_t __size);
gid_t getegid(void);
uid_t geteuid(void);
gid_t getgid(void);



int getgroups(int __gidsetsize, gid_t []);

char * getlogin(void);
pid_t getpgrp(void);
pid_t getpid(void);
pid_t getppid(void);
uid_t getuid(void);
int isatty(int);
int link(const char *, const char *);
off_t lseek(int, off_t, int);
long pathconf(const char *, int);

int pause(void) __asm("_" "pause" );

int pipe(int [2]);

ssize_t read(int, void *, size_t __nbyte) __asm("_" "read" );

int rmdir(const char *);
int setgid(gid_t);
int setpgid(pid_t, pid_t);
pid_t setsid(void);
int setuid(uid_t);

unsigned int
  sleep(unsigned int) __asm("_" "sleep" );

long sysconf(int);
pid_t tcgetpgrp(int);
int tcsetpgrp(int, pid_t);
char * ttyname(int);


int ttyname_r(int, char *, size_t __len) __asm("_" "ttyname_r" );




int unlink(const char *);

ssize_t write(int __fd, const void * __buf, size_t __nbyte) __asm("_" "write" );
# 511 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/unistd.h" 3 4
size_t confstr(int, char *, size_t __len) __asm("_" "confstr" );

int getopt(int __argc, char * const [], const char *) __asm("_" "getopt" );

extern char *optarg;
extern int optind, opterr, optopt;
# 542 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/unistd.h" 3 4
__attribute__((__deprecated__)) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)))

void * brk(const void *);
int chroot(const char *) ;


char * crypt(const char *, const char *);

void encrypt(char *, int) __asm("_" "encrypt" );



int fchdir(int);
long gethostid(void);
pid_t getpgid(pid_t);
pid_t getsid(pid_t);



int getdtablesize(void) ;
int getpagesize(void) __attribute__((__const__)) ;
char * getpass(const char *) ;




char * getwd(char *) ;


int lchown(const char *, uid_t, gid_t) __asm("_" "lchown" );

int lockf(int, int, off_t) __asm("_" "lockf" );

int nice(int) __asm("_" "nice" );

ssize_t pread(int __fd, void * __buf, size_t __nbyte, off_t __offset) __asm("_" "pread" );

ssize_t pwrite(int __fd, const void * __buf, size_t __nbyte, off_t __offset) __asm("_" "pwrite" );






__attribute__((__deprecated__)) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)))

void * sbrk(int);



pid_t setpgrp(void) __asm("_" "setpgrp" );




int setregid(gid_t, gid_t) __asm("_" "setregid" );

int setreuid(uid_t, uid_t) __asm("_" "setreuid" );

void swab(const void * restrict , void * restrict , ssize_t __nbytes);
void sync(void);
int truncate(const char *, off_t);
useconds_t ualarm(useconds_t, useconds_t);
int usleep(useconds_t) __asm("_" "usleep" );


__attribute__((__deprecated__("Use posix_spawn or fork")))

pid_t vfork(void) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));


int fsync(int) __asm("_" "fsync" );

int ftruncate(int, off_t);
int getlogin_r(char *, size_t __namelen);
# 629 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/unistd.h" 3 4
int fchown(int, uid_t, gid_t);
int gethostname(char *, size_t __namelen);
ssize_t readlink(const char * restrict, char * restrict, size_t __bufsize);
int setegid(gid_t);
int seteuid(uid_t);
int symlink(const char *, const char *);
# 643 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/unistd.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/select.h" 1 3 4
# 75 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/select.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_fd_def.h" 1 3 4
# 32 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_fd_def.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/Availability.h" 1 3 4
# 33 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_fd_def.h" 2 3 4
# 50 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_fd_def.h" 3 4
typedef struct fd_set {
 __int32_t fds_bits[((((1024) % ((sizeof(__int32_t) * 8))) == 0) ? ((1024) / ((sizeof(__int32_t) * 8))) : (((1024) / ((sizeof(__int32_t) * 8))) + 1))];
} fd_set;

int __darwin_check_fd_set_overflow(int, const void *, int) __attribute__((availability(macos,introduced=11.0))) __attribute__((availability(ios,introduced=14.0))) __attribute__((availability(tvos,introduced=14.0))) __attribute__((availability(watchos,introduced=7.0)));


inline __attribute__ ((__always_inline__)) int
__darwin_check_fd_set(int _a, const void *_b)
{

#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wunguarded-availability-new"

 if ((uintptr_t)&__darwin_check_fd_set_overflow != (uintptr_t) 0) {



  return __darwin_check_fd_set_overflow(_a, _b, 0);

 } else {
  return 1;
 }

#pragma clang diagnostic pop

}


inline __attribute__ ((__always_inline__)) int
__darwin_fd_isset(int _fd, const struct fd_set *_p)
{
 if (__darwin_check_fd_set(_fd, (const void *) _p)) {
  return _p->fds_bits[(unsigned long)_fd / (sizeof(__int32_t) * 8)] & ((__int32_t)(((unsigned long)1) << ((unsigned long)_fd % (sizeof(__int32_t) * 8))));
 }

 return 0;
}

inline __attribute__ ((__always_inline__)) void
__darwin_fd_set(int _fd, struct fd_set *const _p)
{
 if (__darwin_check_fd_set(_fd, (const void *) _p)) {
  (_p->fds_bits[(unsigned long)_fd / (sizeof(__int32_t) * 8)] |= ((__int32_t)(((unsigned long)1) << ((unsigned long)_fd % (sizeof(__int32_t) * 8)))));
 }
}

inline __attribute__ ((__always_inline__)) void
__darwin_fd_clr(int _fd, struct fd_set *const _p)
{
 if (__darwin_check_fd_set(_fd, (const void *) _p)) {
  (_p->fds_bits[(unsigned long)_fd / (sizeof(__int32_t) * 8)] &= ~((__int32_t)(((unsigned long)1) << ((unsigned long)_fd % (sizeof(__int32_t) * 8)))));
 }
}
# 76 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/select.h" 2 3 4








# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_time_t.h" 1 3 4
# 31 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_time_t.h" 3 4
typedef __darwin_time_t time_t;
# 85 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/select.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_suseconds_t.h" 1 3 4
# 31 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_suseconds_t.h" 3 4
typedef __darwin_suseconds_t suseconds_t;
# 86 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/select.h" 2 3 4
# 100 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/select.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_fd_setsize.h" 1 3 4
# 101 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/select.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_fd_set.h" 1 3 4
# 102 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/select.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_fd_clr.h" 1 3 4
# 103 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/select.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_fd_isset.h" 1 3 4
# 104 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/select.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_fd_zero.h" 1 3 4
# 105 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/select.h" 2 3 4


# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_fd_copy.h" 1 3 4
# 108 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/select.h" 2 3 4






int pselect(int, fd_set * restrict, fd_set * restrict,
    fd_set * restrict, const struct timespec * restrict,
    const sigset_t * restrict)




__asm("_" "pselect" )




;




# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_select.h" 1 3 4
# 43 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_select.h" 3 4
int select(int, fd_set * restrict, fd_set * restrict,
    fd_set * restrict, struct timeval * restrict)





__asm("_" "select" )




;
# 132 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/select.h" 2 3 4
# 644 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/unistd.h" 2 3 4






void _Exit(int) __attribute__((__noreturn__));
int accessx_np(const struct accessx_descriptor *, size_t __sz, int *, uid_t);
int acct(const char *);
int add_profil(char *, size_t __bufsiz, unsigned long, unsigned int) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));
void endusershell(void);
int execvP(const char * __file, const char * __searchpath, char * const * __argv) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));
char * fflagstostr(unsigned long);
int getdomainname(char *, int __namelen);
int getgrouplist(const char *, int, int *, int *__ngroups);

# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/gethostuuid.h" 1 3 4
# 35 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/gethostuuid.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/Availability.h" 1 3 4
# 36 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/gethostuuid.h" 2 3 4





int gethostuuid(uuid_t, const struct timespec *) __attribute__((availability(macos,introduced=10.5))) __attribute__((availability(ios,unavailable))) __attribute__((availability(tvos,unavailable))) __attribute__((availability(watchos,unavailable)));
# 661 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/unistd.h" 2 3 4

mode_t getmode(const void *, mode_t);
int getpeereid(int, uid_t *, gid_t *);
int getsgroups_np(int *, uuid_t);
char * getusershell(void);
int getwgroups_np(int *, uuid_t);
int initgroups(const char *, int);
int issetugid(void);
char * mkdtemp(char *);
int mknod(const char *, mode_t, dev_t);
int mkpath_np(const char *path, mode_t omode) __attribute__((availability(macosx,introduced=10.8)));
int mkpathat_np(int dfd, const char *path, mode_t omode)
  __attribute__((availability(macosx,introduced=10.12))) __attribute__((availability(ios,introduced=10.0)))
  __attribute__((availability(tvos,introduced=10.0))) __attribute__((availability(watchos,introduced=3.0)));
int mkstemp(char *);
int mkstemps(char *, int);
char * mktemp(char *);
int mkostemp(char * path, int oflags)
  __attribute__((availability(macosx,introduced=10.12))) __attribute__((availability(ios,introduced=10.0)))
  __attribute__((availability(tvos,introduced=10.0))) __attribute__((availability(watchos,introduced=3.0)));
int mkostemps(char * path, int slen, int oflags)
  __attribute__((availability(macosx,introduced=10.12))) __attribute__((availability(ios,introduced=10.0)))
  __attribute__((availability(tvos,introduced=10.0))) __attribute__((availability(watchos,introduced=3.0)));

int mkstemp_dprotected_np(char * path, int dpclass, int dpflags)
  __attribute__((availability(macosx,unavailable))) __attribute__((availability(ios,introduced=10.0)))
  __attribute__((availability(tvos,introduced=10.0))) __attribute__((availability(watchos,introduced=3.0)));
char * mkdtempat_np(int dfd, char * path)
  __attribute__((availability(macosx,introduced=10.13))) __attribute__((availability(ios,introduced=11.0)))
  __attribute__((availability(tvos,introduced=11.0))) __attribute__((availability(watchos,introduced=4.0)));
int mkstempsat_np(int dfd, char * path, int slen)
  __attribute__((availability(macosx,introduced=10.13))) __attribute__((availability(ios,introduced=11.0)))
  __attribute__((availability(tvos,introduced=11.0))) __attribute__((availability(watchos,introduced=4.0)));
int mkostempsat_np(int dfd, char * path, int slen, int oflags)
  __attribute__((availability(macosx,introduced=10.13))) __attribute__((availability(ios,introduced=11.0)))
  __attribute__((availability(tvos,introduced=11.0))) __attribute__((availability(watchos,introduced=4.0)));
int nfssvc(int, void *);
int profil(char *, size_t __bufsiz, unsigned long, unsigned int);

__attribute__((__deprecated__("Use of per-thread security contexts is error-prone and discouraged.")))
int pthread_setugid_np(uid_t, gid_t);
int pthread_getugid_np(uid_t *, gid_t *);

int reboot(int);
int revoke(const char *);

__attribute__((__deprecated__)) int rcmd(char * *, int, const char *, const char *, const char *, int *);
__attribute__((__deprecated__)) int rcmd_af(char * *, int, const char *, const char *, const char *, int *,
  int);
__attribute__((__deprecated__)) int rresvport(int *);
__attribute__((__deprecated__)) int rresvport_af(int *, int);
__attribute__((__deprecated__)) int iruserok(unsigned long, int, const char *, const char *);
__attribute__((__deprecated__)) int iruserok_sa(const void *, int, int, const char *, const char *);
__attribute__((__deprecated__)) int ruserok(const char *, int, const char *, const char *);

int setdomainname(const char *, int __namelen);
int setgroups(int, const gid_t *);
void sethostid(long);
int sethostname(const char *, int __namelen);

void setkey(const char *) __asm("_" "setkey" );



int setlogin(const char *);
void *setmode(const char *) __asm("_" "setmode" );
int setrgid(gid_t);
int setruid(uid_t);
int setsgroups_np(int, const uuid_t);
void setusershell(void);
int setwgroups_np(int, const uuid_t);
int strtofflags(char * *, unsigned long *, unsigned long *);
int swapon(const char *);
int ttyslot(void);
int undelete(const char *);
int unwhiteout(const char *);
void * valloc(size_t __size);

__attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)))
__attribute__((availability(ios,deprecated=10.0,message="syscall(2) is unsupported; " "please switch to a supported interface. For SYS_kdebug_trace use kdebug_signpost().")))

__attribute__((availability(macosx,deprecated=10.12,message="syscall(2) is unsupported; " "please switch to a supported interface. For SYS_kdebug_trace use kdebug_signpost().")))

int syscall(int, ...);

extern char *suboptarg;
int getsubopt(char * *, char * const *, char * *);



int fgetattrlist(int,void*,void *,size_t __attrBufSize,unsigned int) __attribute__((availability(macosx,introduced=10.6)));
int fsetattrlist(int,void*,void *,size_t __attrBufSize,unsigned int) __attribute__((availability(macosx,introduced=10.6)));
int getattrlist(const char*,void*,void *,size_t __attrBufSize,unsigned int) __asm("_" "getattrlist" );
int setattrlist(const char*,void*,void *,size_t __attrBufSize,unsigned int) __asm("_" "setattrlist" );
int exchangedata(const char*,const char*,unsigned int) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));
int getdirentriesattr(int,void*,void *,size_t __attrBufSize,unsigned int*,unsigned int*,unsigned int*,unsigned int) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));
# 771 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/unistd.h" 3 4
struct fssearchblock;
struct searchstate;

int searchfs(const char *, struct fssearchblock *, unsigned long *, unsigned int, unsigned int, struct searchstate *) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));
int fsctl(const char *,unsigned long,void *,unsigned int);
int ffsctl(int,unsigned long,void *,unsigned int) __attribute__((availability(macosx,introduced=10.6)));




int fsync_volume_np(int, int) __attribute__((availability(macosx,introduced=10.8)));
int sync_volume_np(const char *, int) __attribute__((availability(macosx,introduced=10.8)));

extern int optreset;
# 15 "/Users/kevin/projects/lattice/salt-front/runtime.c" 2
# 38 "/Users/kevin/projects/lattice/salt-front/runtime.c"
int32_t salt_get_argc() {

  return (int32_t)*_NSGetArgc();



}

const char *salt_get_argv(int32_t idx) {

  int argc = *_NSGetArgc();
  char **argv = *_NSGetArgv();




  if (idx < 0 || idx >= argc || argv == ((void*)0))
    return ((void*)0);
  return argv[idx];
}

int64_t salt_get_argv_len(int32_t idx) {

  int argc = *_NSGetArgc();
  char **argv = *_NSGetArgv();




  if (idx < 0 || idx >= argc || argv == ((void*)0))
    return 0;
  return (int64_t)strlen(argv[idx]);
}

const char *salt_getenv(const char *name) { return getenv(name); }

int64_t salt_strlen(const char *s) {
  if (!s)
    return 0;
  return (int64_t)strlen(s);
}


int64_t sys_open(const char *path, int64_t flags, int64_t mode) {
  return open(path, (int)flags, (int)mode);
}


int64_t sys_write(int32_t fd, const void *buf, int64_t len) {
  return (int64_t)write(fd, buf, (size_t)len);
}


int64_t sys_read(int32_t fd, void *buf, int64_t len) {
  return (int64_t)read(fd, buf, (size_t)len);
}


int32_t sys_close(int32_t fd) { return (int32_t)close(fd); }


int64_t sys_mmap(int64_t addr, int64_t len, int64_t prot, int64_t flags,
                 int64_t fd, int64_t offset) {
  void *result = mmap((void *)addr, (size_t)len, (int)prot, (int)flags, (int)fd,
                      (off_t)offset);
  return (int64_t)result;
}


int32_t sys_munmap(int64_t addr, int64_t length) {
  return (int32_t)munmap((void *)(uintptr_t)addr, (size_t)length);
}





int64_t salt_memcpy_impl(int64_t dst, int64_t src,
                         int64_t len) __asm__("_memcpy");
int64_t salt_memcpy_impl(int64_t dst, int64_t src, int64_t len) {
  unsigned char *d = (unsigned char *)(uintptr_t)dst;
  const unsigned char *s = (const unsigned char *)(uintptr_t)src;
  if (len <= 0)
    return dst;


  if (d > s && (size_t)(d - s) < (size_t)len) {
    for (int64_t i = len - 1; i >= 0; i--) {
      d[i] = s[i];
    }
  } else {
    for (int64_t i = 0; i < len; i++) {
      d[i] = s[i];
    }
  }
  return dst;
}



int64_t salt_mmap(int64_t size) {
  void *result = mmap(((void*)0), (size_t)size, 0x01 | 0x02,
                      0x0002 | 0x1000, -1, 0);
  return (result == ((void *)-1)) ? 0 : (int64_t)result;
}

int64_t salt_munmap(int64_t addr, int64_t size) {
  return (int64_t)munmap((void *)addr, (size_t)size);
}







static int64_t arena_base = 0;
static int64_t arena_current = 0;
static int64_t arena_end = 0;


static void arena_init() {
  if (arena_base == 0) {
    void *result = mmap(((void*)0), (256 * 1024 * 1024), 0x01 | 0x02,
                        0x0002 | 0x1000, -1, 0);
    if (result == ((void *)-1)) {
      arena_base = 0;
    } else {
      arena_base = (int64_t)result;
      arena_current = arena_base;
      arena_end = arena_base + (256 * 1024 * 1024);
    }
  }
}



int64_t salt_arena_alloc(int64_t size) {
  arena_init();

  int64_t aligned_addr = (arena_current + 7) & ~7;
  int64_t next = aligned_addr + size;
  if (next > arena_end)
    return 0;
  arena_current = next;
  return aligned_addr;
}


int64_t salt_arena_mark() {
  arena_init();
  return arena_current;
}



void salt_arena_reset_to(int64_t mark) {
# 205 "/Users/kevin/projects/lattice/salt-front/runtime.c"
  arena_current = mark;
}


int64_t salt_clock_now() {

  static mach_timebase_info_data_t timebase_info;
  static int timebase_initialized = 0;
  if (!timebase_initialized) {
    mach_timebase_info(&timebase_info);
    timebase_initialized = 1;
  }
  uint64_t ticks = mach_absolute_time();
  return (int64_t)(ticks * timebase_info.numer / timebase_info.denom);





}


int64_t rdtsc() { return salt_clock_now(); }








#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
int64_t syscall6(int64_t number, int64_t arg1, int64_t arg2, int64_t arg3,
                 int64_t arg4, int64_t arg5, int64_t arg6) {

  return syscall(number, arg1, arg2, arg3, arg4, arg5, arg6);
}
#pragma clang diagnostic pop

void ___salt_yield_check() {

}

void __salt_yield_check() {}


void debug_print_ptr(void *p, const char *label) {
  printf("[SALT-DEBUG] %s: %p\n", label, p);
}



void __salt_contract_violation() {
  fprintf(__stderrp,
          "FATAL: Salt contract violation (requires/ensures/invariant)\n");
  abort();
}


void __salt_panic(const char *message) {
  fprintf(__stderrp, "CRITICAL: Salt Runtime Panic: %s\n", message);
  __builtin_trap();
}


void printf_shim(const char *fmt, int64_t val) { printf(fmt, val); }





void __salt_print_literal(const char *str, int64_t len) {
  fwrite(str, 1, (size_t)len, __stdoutp);
}

void __salt_print_i64(int64_t val) { printf("%lld", val); }

void __salt_print_u64(int64_t val) { printf("%llu", (unsigned long long)val); }

void __salt_print_f64(double val) { printf("%g", val); }

void __salt_print_bool(int8_t val) { printf("%s", val ? "true" : "false"); }

void __salt_print_ptr(int64_t val) {
  printf("0x%llx", (unsigned long long)val);
}
# 301 "/Users/kevin/projects/lattice/salt-front/runtime.c"
int64_t __salt_fmt_f64_to_buf(char *buf, double val, int64_t precision) {


  char fmt[8];
  __builtin___snprintf_chk (fmt, sizeof(fmt), 0, __builtin_object_size (fmt, 2 > 1 ? 1 : 0), "%%.%lldf", (long long)precision);
  int written = __builtin___snprintf_chk (buf, 64, 0, __builtin_object_size (buf, 2 > 1 ? 1 : 0), fmt, val);
  return (int64_t)(written > 0 ? written : 0);
}
# 319 "/Users/kevin/projects/lattice/salt-front/runtime.c"
void *salt_sys_alloc(int64_t size, int64_t align) {
  if (size <= 0)
    return ((void*)0);

  void *ptr = ((void*)0);

  size_t actual_align = (size_t)align;
  if (actual_align < sizeof(void *)) {
    actual_align = sizeof(void *);
  }

  if (posix_memalign(&ptr, actual_align, (size_t)size) != 0) {

    fprintf(__stderrp, "FATAL: Salt runtime failed to allocate %lld bytes\n",
            (long long)size);
    return ((void*)0);
  }
  return ptr;
}





void salt_sys_dealloc(void *ptr, int64_t size, int64_t align) {
  (void)size;
  (void)align;
  free(ptr);
}





void *simple_alloc(int64_t size) {
  return salt_sys_alloc(size, 8);
}

void simple_dealloc(void *ptr, int64_t size) { salt_sys_dealloc(ptr, size, 8); }





void *salt_sys_realloc(void *ptr, int64_t new_size) {
  return realloc(ptr, (size_t)new_size);
}





void salt_sys_free(void *ptr) { free(ptr); }




# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread.h" 1 3 4
# 56 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread/sched.h" 1 3 4
# 28 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread/sched.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread/pthread_impl.h" 1 3 4
# 33 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread/pthread_impl.h" 3 4
#pragma clang assume_nonnull begin
# 33 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread/pthread_impl.h" 3 4
#pragma clang assume_nonnull end
# 29 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread/sched.h" 2 3 4






struct sched_param { int sched_priority; char __opaque[4]; };




extern int sched_yield(void);
extern int sched_get_priority_min(int);
extern int sched_get_priority_max(int);
# 57 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/time.h" 1 3 4
# 63 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/time.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_time.h" 1 3 4
# 69 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_time.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/Availability.h" 1 3 4
# 70 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_time.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_clock_t.h" 1 3 4
# 31 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_clock_t.h" 3 4
typedef __darwin_clock_t clock_t;
# 71 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_time.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_null.h" 1 3 4
# 72 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_time.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_size_t.h" 1 3 4
# 73 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_time.h" 2 3 4





struct tm {
 int tm_sec;
 int tm_min;
 int tm_hour;
 int tm_mday;
 int tm_mon;
 int tm_year;
 int tm_wday;
 int tm_yday;
 int tm_isdst;
 long tm_gmtoff;
 char * tm_zone;
};
# 101 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_time.h" 3 4
extern char * tzname[];


extern int getdate_err;

extern long timezone __asm("_" "timezone" );

extern int daylight;


char * asctime(const struct tm *);
clock_t clock(void) __asm("_" "clock" );
char * ctime(const time_t *);
double difftime(time_t, time_t);
struct tm *getdate(const char *);
struct tm *gmtime(const time_t *);
struct tm *localtime(const time_t *);
time_t mktime(struct tm *) __asm("_" "mktime" );
size_t strftime(char * restrict, size_t __maxsize, const char * restrict, const struct tm * restrict) __asm("_" "strftime" );
char * strptime(const char * restrict, const char * restrict, struct tm * restrict) __asm("_" "strptime" );
time_t time(time_t *);


void tzset(void);



char * asctime_r(const struct tm * restrict, char * restrict );
char * ctime_r(const time_t *, char *);
struct tm *gmtime_r(const time_t * restrict, struct tm * restrict);
struct tm *localtime_r(const time_t * restrict, struct tm * restrict);


time_t posix2time(time_t);



void tzsetwall(void);
time_t time2posix(time_t);
time_t timelocal(struct tm * const);
time_t timegm(struct tm * const);



int nanosleep(const struct timespec *__rqtp, struct timespec *__rmtp) __asm("_" "nanosleep" );
# 156 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_time.h" 3 4
typedef enum {
_CLOCK_REALTIME __attribute__((availability(macosx,introduced=10.12))) __attribute__((availability(ios,introduced=10.0))) __attribute__((availability(tvos,introduced=10.0))) __attribute__((availability(watchos,introduced=3.0))) = 0,

_CLOCK_MONOTONIC __attribute__((availability(macosx,introduced=10.12))) __attribute__((availability(ios,introduced=10.0))) __attribute__((availability(tvos,introduced=10.0))) __attribute__((availability(watchos,introduced=3.0))) = 6,


_CLOCK_MONOTONIC_RAW __attribute__((availability(macosx,introduced=10.12))) __attribute__((availability(ios,introduced=10.0))) __attribute__((availability(tvos,introduced=10.0))) __attribute__((availability(watchos,introduced=3.0))) = 4,

_CLOCK_MONOTONIC_RAW_APPROX __attribute__((availability(macosx,introduced=10.12))) __attribute__((availability(ios,introduced=10.0))) __attribute__((availability(tvos,introduced=10.0))) __attribute__((availability(watchos,introduced=3.0))) = 5,

_CLOCK_UPTIME_RAW __attribute__((availability(macosx,introduced=10.12))) __attribute__((availability(ios,introduced=10.0))) __attribute__((availability(tvos,introduced=10.0))) __attribute__((availability(watchos,introduced=3.0))) = 8,

_CLOCK_UPTIME_RAW_APPROX __attribute__((availability(macosx,introduced=10.12))) __attribute__((availability(ios,introduced=10.0))) __attribute__((availability(tvos,introduced=10.0))) __attribute__((availability(watchos,introduced=3.0))) = 9,


_CLOCK_PROCESS_CPUTIME_ID __attribute__((availability(macosx,introduced=10.12))) __attribute__((availability(ios,introduced=10.0))) __attribute__((availability(tvos,introduced=10.0))) __attribute__((availability(watchos,introduced=3.0))) = 12,

_CLOCK_THREAD_CPUTIME_ID __attribute__((availability(macosx,introduced=10.12))) __attribute__((availability(ios,introduced=10.0))) __attribute__((availability(tvos,introduced=10.0))) __attribute__((availability(watchos,introduced=3.0))) = 16

} clockid_t;

__attribute__((availability(macosx,introduced=10.12))) __attribute__((availability(ios,introduced=10.0))) __attribute__((availability(tvos,introduced=10.0))) __attribute__((availability(watchos,introduced=3.0)))
int clock_getres(clockid_t __clock_id, struct timespec *__res);

__attribute__((availability(macosx,introduced=10.12))) __attribute__((availability(ios,introduced=10.0))) __attribute__((availability(tvos,introduced=10.0))) __attribute__((availability(watchos,introduced=3.0)))
int clock_gettime(clockid_t __clock_id, struct timespec *__tp);


__attribute__((availability(macosx,introduced=10.12))) __attribute__((availability(ios,introduced=10.0))) __attribute__((availability(tvos,introduced=10.0))) __attribute__((availability(watchos,introduced=3.0)))
__uint64_t clock_gettime_nsec_np(clockid_t __clock_id);


__attribute__((availability(macosx,introduced=10.12))) __attribute__((availability(ios,unavailable)))
__attribute__((availability(tvos,unavailable))) __attribute__((availability(watchos,unavailable)))
int clock_settime(clockid_t __clock_id, const struct timespec *__tp);
# 201 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/_time.h" 3 4
__attribute__((availability(macos,introduced=10.15))) __attribute__((availability(ios,introduced=13.0))) __attribute__((availability(tvos,introduced=13.0))) __attribute__((availability(watchos,introduced=6.0)))
int timespec_get(struct timespec *ts, int base);
# 64 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/time.h" 2 3 4
# 58 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread.h" 2 3 4


# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_pthread/_pthread_cond_t.h" 1 3 4
# 31 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_pthread/_pthread_cond_t.h" 3 4
typedef __darwin_pthread_cond_t pthread_cond_t;
# 61 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_pthread/_pthread_condattr_t.h" 1 3 4
# 31 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_pthread/_pthread_condattr_t.h" 3 4
typedef __darwin_pthread_condattr_t pthread_condattr_t;
# 62 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_pthread/_pthread_key_t.h" 1 3 4
# 31 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_pthread/_pthread_key_t.h" 3 4
typedef __darwin_pthread_key_t pthread_key_t;
# 63 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_pthread/_pthread_mutex_t.h" 1 3 4
# 31 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_pthread/_pthread_mutex_t.h" 3 4
typedef __darwin_pthread_mutex_t pthread_mutex_t;
# 64 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_pthread/_pthread_mutexattr_t.h" 1 3 4
# 31 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_pthread/_pthread_mutexattr_t.h" 3 4
typedef __darwin_pthread_mutexattr_t pthread_mutexattr_t;
# 65 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_pthread/_pthread_once_t.h" 1 3 4
# 31 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_pthread/_pthread_once_t.h" 3 4
typedef __darwin_pthread_once_t pthread_once_t;
# 66 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_pthread/_pthread_rwlock_t.h" 1 3 4
# 31 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_pthread/_pthread_rwlock_t.h" 3 4
typedef __darwin_pthread_rwlock_t pthread_rwlock_t;
# 67 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_pthread/_pthread_rwlockattr_t.h" 1 3 4
# 31 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_pthread/_pthread_rwlockattr_t.h" 3 4
typedef __darwin_pthread_rwlockattr_t pthread_rwlockattr_t;
# 68 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread.h" 2 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_pthread/_pthread_t.h" 1 3 4
# 31 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_pthread/_pthread_t.h" 3 4
typedef __darwin_pthread_t pthread_t;
# 69 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread.h" 2 3 4

# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread/qos.h" 1 3 4
# 30 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread/qos.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/Availability.h" 1 3 4
# 31 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread/qos.h" 2 3 4



# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/qos.h" 1 3 4
# 28 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/qos.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/Availability.h" 1 3 4
# 29 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/qos.h" 2 3 4
# 130 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/qos.h" 3 4
enum { QOS_CLASS_USER_INTERACTIVE __attribute__((availability(macos,introduced=10.10))) __attribute__((availability(ios,introduced=8.0))) = 0x21, QOS_CLASS_USER_INITIATED __attribute__((availability(macos,introduced=10.10))) __attribute__((availability(ios,introduced=8.0))) = 0x19, QOS_CLASS_DEFAULT __attribute__((availability(macos,introduced=10.10))) __attribute__((availability(ios,introduced=8.0))) = 0x15, QOS_CLASS_UTILITY __attribute__((availability(macos,introduced=10.10))) __attribute__((availability(ios,introduced=8.0))) = 0x11, QOS_CLASS_BACKGROUND __attribute__((availability(macos,introduced=10.10))) __attribute__((availability(ios,introduced=8.0))) = 0x09, QOS_CLASS_UNSPECIFIED __attribute__((availability(macos,introduced=10.10))) __attribute__((availability(ios,introduced=8.0))) = 0x00, }; typedef unsigned int qos_class_t;
# 170 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/qos.h" 3 4
__attribute__((availability(macos,introduced=10.10))) __attribute__((availability(ios,introduced=8.0)))
qos_class_t
qos_class_self(void);
# 192 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/qos.h" 3 4
__attribute__((availability(macos,introduced=10.10))) __attribute__((availability(ios,introduced=8.0)))
qos_class_t
qos_class_main(void);
# 35 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread/qos.h" 2 3 4




#pragma clang assume_nonnull begin
# 81 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread/qos.h" 3 4
__attribute__((availability(macos,introduced=10.10))) __attribute__((availability(ios,introduced=8.0)))
int
pthread_attr_set_qos_class_np(pthread_attr_t *__attr,
  qos_class_t __qos_class, int __relative_priority);
# 112 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread/qos.h" 3 4
__attribute__((availability(macos,introduced=10.10))) __attribute__((availability(ios,introduced=8.0)))
int
pthread_attr_get_qos_class_np(pthread_attr_t * restrict __attr,
  qos_class_t * _Nullable restrict __qos_class,
  int * _Nullable restrict __relative_priority);
# 153 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread/qos.h" 3 4
__attribute__((availability(macos,introduced=10.10))) __attribute__((availability(ios,introduced=8.0)))
int
pthread_set_qos_class_self_np(qos_class_t __qos_class,
  int __relative_priority);
# 184 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread/qos.h" 3 4
__attribute__((availability(macos,introduced=10.10))) __attribute__((availability(ios,introduced=8.0)))
int
pthread_get_qos_class_np(pthread_t __pthread,
  qos_class_t * _Nullable restrict __qos_class,
  int * _Nullable restrict __relative_priority);
# 211 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread/qos.h" 3 4
typedef struct pthread_override_s* pthread_override_t;
# 263 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread/qos.h" 3 4
__attribute__((availability(macos,introduced=10.10))) __attribute__((availability(ios,introduced=8.0)))
pthread_override_t
pthread_override_qos_class_start_np(pthread_t __pthread,
  qos_class_t __qos_class, int __relative_priority);
# 291 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread/qos.h" 3 4
__attribute__((availability(macos,introduced=10.10))) __attribute__((availability(ios,introduced=8.0)))
int
pthread_override_qos_class_end_np(pthread_override_t __override);
# 293 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread/qos.h" 3 4
#pragma clang assume_nonnull end
# 71 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread.h" 2 3 4
# 100 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/Availability.h" 1 3 4
# 101 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread.h" 2 3 4


#pragma clang assume_nonnull begin
# 225 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread.h" 3 4
__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_atfork(void (* _Nullable)(void), void (* _Nullable)(void),
  void (* _Nullable)(void));

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_attr_destroy(pthread_attr_t *);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_attr_getdetachstate(const pthread_attr_t *, int *);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_attr_getguardsize(const pthread_attr_t * restrict, size_t * restrict);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_attr_getinheritsched(const pthread_attr_t * restrict, int * restrict);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_attr_getschedparam(const pthread_attr_t * restrict,
  struct sched_param * restrict);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_attr_getschedpolicy(const pthread_attr_t * restrict, int * restrict);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_attr_getscope(const pthread_attr_t * restrict, int * restrict);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_attr_getstack(const pthread_attr_t * restrict,
  void * _Nullable * _Nonnull restrict, size_t * restrict);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_attr_getstackaddr(const pthread_attr_t * restrict,
  void * _Nullable * _Nonnull restrict);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_attr_getstacksize(const pthread_attr_t * restrict, size_t * restrict);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_attr_init(pthread_attr_t *);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_attr_setdetachstate(pthread_attr_t *, int);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_attr_setguardsize(pthread_attr_t *, size_t);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_attr_setinheritsched(pthread_attr_t *, int);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_attr_setschedparam(pthread_attr_t * restrict,
  const struct sched_param * restrict);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_attr_setschedpolicy(pthread_attr_t *, int);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_attr_setscope(pthread_attr_t *, int);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_attr_setstack(pthread_attr_t *, void *, size_t);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_attr_setstackaddr(pthread_attr_t *, void *);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_attr_setstacksize(pthread_attr_t *, size_t);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_cancel(pthread_t) __asm("_" "pthread_cancel" );

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_cond_broadcast(pthread_cond_t *);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_cond_destroy(pthread_cond_t *);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_cond_init(
  pthread_cond_t * restrict,
  const pthread_condattr_t * _Nullable restrict)
  __asm("_" "pthread_cond_init" );

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_cond_signal(pthread_cond_t *);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
__attribute__((__swift_attr__("@_unavailableFromAsync(message: \"" "Use an asynchronous wait instead of a synchronous wait" "\")")))
int pthread_cond_timedwait(
  pthread_cond_t * restrict, pthread_mutex_t * restrict,
  const struct timespec * _Nullable restrict)
  __asm("_" "pthread_cond_timedwait" );

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
__attribute__((__swift_attr__("@_unavailableFromAsync(message: \"" "Use an asynchronous wait instead of a synchronous wait" "\")")))
int pthread_cond_wait(pthread_cond_t * restrict,
  pthread_mutex_t * restrict) __asm("_" "pthread_cond_wait" );

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_condattr_destroy(pthread_condattr_t *);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_condattr_init(pthread_condattr_t *);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_condattr_getpshared(const pthread_condattr_t * restrict,
  int * restrict);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_condattr_setpshared(pthread_condattr_t *, int);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))

int pthread_create(pthread_t _Nullable * _Nonnull restrict,
  const pthread_attr_t * _Nullable restrict,
  void * _Nullable (* _Nonnull)(void * _Nullable),
  void * _Nullable restrict);






__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_detach(pthread_t);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_equal(pthread_t _Nullable, pthread_t _Nullable);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
__attribute__((__swift_attr__("@_unavailableFromAsync(message: \"" "Thread lifecycle is owned by Swift Concurrency runtime" "\")")))
void pthread_exit(void * _Nullable) __attribute__((__noreturn__));

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_getconcurrency(void);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_getschedparam(pthread_t , int * _Nullable restrict,
  struct sched_param * _Nullable restrict);

__attribute__((__swift_attr__("@_unavailableFromAsync(message: \"" "Use Task Local Values instead" "\")")))
__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
void* _Nullable pthread_getspecific(pthread_key_t);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
__attribute__((__swift_attr__("@_unavailableFromAsync(message: \"" "Use an asynchronous wait instead of a synchronous wait" "\")")))
int pthread_join(pthread_t , void * _Nullable * _Nullable)
  __asm("_" "pthread_join" );

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_key_create(pthread_key_t *, void (* _Nullable)(void *));

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_key_delete(pthread_key_t);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_mutex_destroy(pthread_mutex_t *);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_mutex_getprioceiling(const pthread_mutex_t * restrict,
  int * restrict);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_mutex_init(pthread_mutex_t * restrict,
  const pthread_mutexattr_t * _Nullable restrict);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
__attribute__((__swift_attr__("@_unavailableFromAsync(message: \"" "Use OSAllocatedUnfairLock's withLock or NSLock for async-safe scoped locking" "\")")))
int pthread_mutex_lock(pthread_mutex_t *);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_mutex_setprioceiling(pthread_mutex_t * restrict, int,
  int * restrict);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
__attribute__((__swift_attr__("@_unavailableFromAsync(message: \"" "Use OSAllocatedUnfairLock's withLockIfAvailable or NSLock for async-safe scoped locking" "\")")))
int pthread_mutex_trylock(pthread_mutex_t *);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
__attribute__((__swift_attr__("@_unavailableFromAsync(message: \"" "Use OSAllocatedUnfairLock's withLock or NSLock for async-safe scoped locking" "\")")))
int pthread_mutex_unlock(pthread_mutex_t *);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_mutexattr_destroy(pthread_mutexattr_t *) __asm("_" "pthread_mutexattr_destroy" );

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_mutexattr_getprioceiling(const pthread_mutexattr_t * restrict,
  int * restrict);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_mutexattr_getprotocol(const pthread_mutexattr_t * restrict,
  int * restrict);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_mutexattr_getpshared(const pthread_mutexattr_t * restrict,
  int * restrict);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_mutexattr_gettype(const pthread_mutexattr_t * restrict,
  int * restrict);

__attribute__((availability(macos,introduced=10.13.4))) __attribute__((availability(ios,introduced=11.3))) __attribute__((availability(watchos,introduced=4.3))) __attribute__((availability(tvos,introduced=11.3)))
int pthread_mutexattr_getpolicy_np(const pthread_mutexattr_t * restrict,
  int * restrict);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_mutexattr_init(pthread_mutexattr_t *);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_mutexattr_setprioceiling(pthread_mutexattr_t *, int);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_mutexattr_setprotocol(pthread_mutexattr_t *, int);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_mutexattr_setpshared(pthread_mutexattr_t *, int);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_mutexattr_settype(pthread_mutexattr_t *, int);

__attribute__((availability(macos,introduced=10.7))) __attribute__((availability(ios,introduced=5.0)))
int pthread_mutexattr_setpolicy_np(pthread_mutexattr_t *, int);

__attribute__((availability(swift,unavailable,message="Use lazily initialized globals instead")))
__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_once(pthread_once_t *, void (* _Nonnull)(void));

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_rwlock_destroy(pthread_rwlock_t * ) __asm("_" "pthread_rwlock_destroy" );

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_rwlock_init(pthread_rwlock_t * restrict,
  const pthread_rwlockattr_t * _Nullable restrict)
  __asm("_" "pthread_rwlock_init" );

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
__attribute__((__swift_attr__("@_unavailableFromAsync(message: \"" "Use async-safe scoped locking instead" "\")")))
int pthread_rwlock_rdlock(pthread_rwlock_t *) __asm("_" "pthread_rwlock_rdlock" );

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
__attribute__((__swift_attr__("@_unavailableFromAsync(message: \"" "Use async-safe scoped locking instead" "\")")))
int pthread_rwlock_tryrdlock(pthread_rwlock_t *) __asm("_" "pthread_rwlock_tryrdlock" );

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
__attribute__((__swift_attr__("@_unavailableFromAsync(message: \"" "Use async-safe scoped locking instead" "\")")))
int pthread_rwlock_trywrlock(pthread_rwlock_t *) __asm("_" "pthread_rwlock_trywrlock" );

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
__attribute__((__swift_attr__("@_unavailableFromAsync(message: \"" "Use async-safe scoped locking instead" "\")")))
int pthread_rwlock_wrlock(pthread_rwlock_t *) __asm("_" "pthread_rwlock_wrlock" );

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
__attribute__((__swift_attr__("@_unavailableFromAsync(message: \"" "Use async-safe scoped locking instead" "\")")))
int pthread_rwlock_unlock(pthread_rwlock_t *) __asm("_" "pthread_rwlock_unlock" );

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_rwlockattr_destroy(pthread_rwlockattr_t *);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_rwlockattr_getpshared(const pthread_rwlockattr_t * restrict,
  int * restrict);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_rwlockattr_init(pthread_rwlockattr_t *);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_rwlockattr_setpshared(pthread_rwlockattr_t *, int);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
pthread_t pthread_self(void);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
__attribute__((__swift_attr__("@_unavailableFromAsync(message: \"" "Use Task cancellation instead" "\")")))
int pthread_setcancelstate(int , int * _Nullable)
  __asm("_" "pthread_setcancelstate" );

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
__attribute__((__swift_attr__("@_unavailableFromAsync(message: \"" "Use Task cancellation instead" "\")")))
int pthread_setcanceltype(int , int * _Nullable)
  __asm("_" "pthread_setcanceltype" );

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_setconcurrency(int);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_setschedparam(pthread_t, int, const struct sched_param *);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
__attribute__((__swift_attr__("@_unavailableFromAsync(message: \"" "Use Task Local Values instead" "\")")))
int pthread_setspecific(pthread_key_t , const void * _Nullable);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
__attribute__((__swift_attr__("@_unavailableFromAsync(message: \"" "Use Task cancellation instead" "\")")))
void pthread_testcancel(void) __asm("_" "pthread_testcancel" );




__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_is_threaded_np(void);

__attribute__((availability(macos,introduced=10.6))) __attribute__((availability(ios,introduced=3.2)))
int pthread_threadid_np(pthread_t _Nullable,__uint64_t* _Nullable);


__attribute__((availability(macos,introduced=10.6))) __attribute__((availability(ios,introduced=3.2)))
int pthread_getname_np(pthread_t,char*,size_t);

__attribute__((availability(macos,introduced=10.6))) __attribute__((availability(ios,introduced=3.2)))
__attribute__((__swift_attr__("@_unavailableFromAsync(message: \"" "Thread lifecycle is owned by Swift Concurrency runtime" "\")")))
int pthread_setname_np(const char*);


__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_main_np(void);


__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
mach_port_t pthread_mach_thread_np(pthread_t);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
size_t pthread_get_stacksize_np(pthread_t);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
void* pthread_get_stackaddr_np(pthread_t);


__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_cond_signal_thread_np(pthread_cond_t *, pthread_t _Nullable);


__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
__attribute__((__swift_attr__("@_unavailableFromAsync(message: \"" "Use an asynchronous wait instead of a synchronous wait" "\")")))
int pthread_cond_timedwait_relative_np(pthread_cond_t *, pthread_mutex_t *,
  const struct timespec * _Nullable);


__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))

int pthread_create_suspended_np(
  pthread_t _Nullable * _Nonnull, const pthread_attr_t * _Nullable,
  void * _Nullable (* _Nonnull)(void * _Nullable), void * _Nullable);





__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_kill(pthread_t, int);

__attribute__((availability(macos,introduced=10.5))) __attribute__((availability(ios,introduced=2.0)))
_Nullable pthread_t pthread_from_mach_thread_np(mach_port_t);

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
int pthread_sigmask(int, const sigset_t * _Nullable, sigset_t * _Nullable)
  __asm("_" "pthread_sigmask" );

__attribute__((availability(macos,introduced=10.4))) __attribute__((availability(ios,introduced=2.0)))
__attribute__((__swift_attr__("@_unavailableFromAsync(message: \"" "Use Task.yield(), or await a condition instead of spinning" "\")")))
void pthread_yield_np(void);

__attribute__((availability(macos,introduced=11.0)))
__attribute__((availability(ios,unavailable))) __attribute__((availability(tvos,unavailable))) __attribute__((availability(watchos,unavailable))) __attribute__((availability(driverkit,unavailable)))
void pthread_jit_write_protect_np(int enabled);

__attribute__((availability(macos,introduced=11.0))) __attribute__((availability(ios,introduced=17.4)))
__attribute__((availability(tvos,unavailable))) __attribute__((availability(watchos,unavailable))) __attribute__((availability(driverkit,unavailable))) __attribute__((availability(visionos,unavailable)))
int pthread_jit_write_protect_supported_np(void);
# 608 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread.h" 3 4
typedef int (*pthread_jit_write_callback_t)(void * _Nullable ctx);
# 694 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread.h" 3 4
__attribute__((availability(macos,introduced=11.4))) __attribute__((availability(ios,introduced=17.4)))
__attribute__((availability(tvos,unavailable))) __attribute__((availability(watchos,unavailable))) __attribute__((availability(driverkit,unavailable))) __attribute__((availability(visionos,unavailable)))
__attribute__((availability(swift,unavailable,message="This interface cannot be safely used from Swift")))
int pthread_jit_write_with_callback_np(
  pthread_jit_write_callback_t _Nonnull callback, void * _Nullable ctx);
# 725 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread.h" 3 4
__attribute__((availability(macos,introduced=12.1))) __attribute__((availability(ios,introduced=17.4)))
__attribute__((availability(tvos,unavailable))) __attribute__((availability(watchos,unavailable))) __attribute__((availability(driverkit,unavailable))) __attribute__((availability(visionos,unavailable)))
void pthread_jit_write_freeze_callbacks_np(void);
# 744 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread.h" 3 4
__attribute__((availability(macos,introduced=11.0))) __attribute__((availability(ios,introduced=14.2))) __attribute__((availability(tvos,introduced=14.2))) __attribute__((availability(watchos,introduced=7.1)))
int
pthread_cpu_number_np(size_t *cpu_number_out);
# 746 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/pthread.h" 3 4
#pragma clang assume_nonnull end
# 377 "/Users/kevin/projects/lattice/salt-front/runtime.c" 2



typedef void (*salt_thread_fn)(void);

struct salt_thread_trampoline_ctx {
  salt_thread_fn fn;
};

static void *salt_thread_trampoline(void *arg) {
  struct salt_thread_trampoline_ctx *ctx =
      (struct salt_thread_trampoline_ctx *)arg;
  salt_thread_fn fn = ctx->fn;
  free(ctx);
  fn();
  return ((void*)0);
}



int64_t salt_thread_spawn(int64_t fn_addr) {
  pthread_t tid;
  struct salt_thread_trampoline_ctx *ctx = malloc(sizeof(*ctx));
  ctx->fn = (salt_thread_fn)fn_addr;
  int ret = pthread_create(&tid, ((void*)0), salt_thread_trampoline, ctx);
  if (ret != 0) {
    free(ctx);
    return 0;
  }
  return (int64_t)tid;
}


int32_t salt_thread_join(int64_t handle) {
  return (int32_t)pthread_join((pthread_t)handle, ((void*)0));
}






int64_t salt_mutex_create(void) {
  pthread_mutex_t *m = (pthread_mutex_t *)malloc(sizeof(pthread_mutex_t));
  pthread_mutex_init(m, ((void*)0));
  return (int64_t)m;
}

void salt_mutex_lock(int64_t handle) {
  pthread_mutex_lock((pthread_mutex_t *)handle);
}

void salt_mutex_unlock(int64_t handle) {
  pthread_mutex_unlock((pthread_mutex_t *)handle);
}

void salt_mutex_destroy(int64_t handle) {
  pthread_mutex_destroy((pthread_mutex_t *)handle);
  free((void *)handle);
}


int64_t salt_atomic_load_i64(int64_t *ptr) {
  return __atomic_load_n(ptr, 5);
}

void salt_atomic_store_i64(int64_t *ptr, int64_t val) {
  __atomic_store_n(ptr, val, 5);
}

int64_t salt_atomic_add_i64(int64_t *ptr, int64_t val) {
  return __atomic_fetch_add(ptr, val, 5);
}

int64_t salt_atomic_cas_i64(int64_t *ptr, int64_t expected, int64_t desired) {
  __atomic_compare_exchange_n(ptr, &expected, desired, 0, 5,
                              5);
  return expected;
}




# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/spawn.h" 1 3 4
# 34 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/spawn.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/spawn.h" 1 3 4
# 35 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/spawn.h" 2 3 4

# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/Availability.h" 1 3 4
# 37 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/spawn.h" 2 3 4
# 51 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/spawn.h" 3 4
typedef void *posix_spawnattr_t;
typedef void *posix_spawn_file_actions_t;







int posix_spawn(pid_t * restrict, const char * restrict,
    const posix_spawn_file_actions_t *,
    const posix_spawnattr_t * restrict,
    char *const __argv[restrict],
    char *const __envp[restrict]) __attribute__((availability(macos,introduced=10.5))) __attribute__((availability(ios,introduced=2.0))) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));

int posix_spawnp(pid_t * restrict, const char * restrict,
    const posix_spawn_file_actions_t *,
    const posix_spawnattr_t * restrict,
    char *const __argv[restrict],
    char *const __envp[restrict]) __attribute__((availability(macos,introduced=10.5))) __attribute__((availability(ios,introduced=2.0)));

int posix_spawn_file_actions_addclose(posix_spawn_file_actions_t *, int) __attribute__((availability(macos,introduced=10.5))) __attribute__((availability(ios,introduced=2.0))) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));

int posix_spawn_file_actions_adddup2(posix_spawn_file_actions_t *, int,
    int) __attribute__((availability(macos,introduced=10.5))) __attribute__((availability(ios,introduced=2.0))) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));

int posix_spawn_file_actions_addopen(
 posix_spawn_file_actions_t * restrict, int,
 const char * restrict, int, mode_t) __attribute__((availability(macos,introduced=10.5))) __attribute__((availability(ios,introduced=2.0))) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));

int posix_spawn_file_actions_destroy(posix_spawn_file_actions_t *) __attribute__((availability(macos,introduced=10.5))) __attribute__((availability(ios,introduced=2.0))) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));

int posix_spawn_file_actions_init(posix_spawn_file_actions_t *) __attribute__((availability(macos,introduced=10.5))) __attribute__((availability(ios,introduced=2.0))) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));

int posix_spawnattr_destroy(posix_spawnattr_t *) __attribute__((availability(macos,introduced=10.5))) __attribute__((availability(ios,introduced=2.0))) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));

int posix_spawnattr_getsigdefault(const posix_spawnattr_t * restrict,
    sigset_t * restrict) __attribute__((availability(macos,introduced=10.5))) __attribute__((availability(ios,introduced=2.0))) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));

int posix_spawnattr_getflags(const posix_spawnattr_t * restrict,
    short * restrict) __attribute__((availability(macos,introduced=10.5))) __attribute__((availability(ios,introduced=2.0))) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));

int posix_spawnattr_getpgroup(const posix_spawnattr_t * restrict,
    pid_t * restrict) __attribute__((availability(macos,introduced=10.5))) __attribute__((availability(ios,introduced=2.0))) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));

int posix_spawnattr_getsigmask(const posix_spawnattr_t * restrict,
    sigset_t * restrict) __attribute__((availability(macos,introduced=10.5))) __attribute__((availability(ios,introduced=2.0))) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));

int posix_spawnattr_init(posix_spawnattr_t *) __attribute__((availability(macos,introduced=10.5))) __attribute__((availability(ios,introduced=2.0))) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));

int posix_spawnattr_setsigdefault(posix_spawnattr_t * restrict,
    const sigset_t * restrict) __attribute__((availability(macos,introduced=10.5))) __attribute__((availability(ios,introduced=2.0))) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));

int posix_spawnattr_setflags(posix_spawnattr_t *, short) __attribute__((availability(macos,introduced=10.5))) __attribute__((availability(ios,introduced=2.0))) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));

int posix_spawnattr_setpgroup(posix_spawnattr_t *, pid_t) __attribute__((availability(macos,introduced=10.5))) __attribute__((availability(ios,introduced=2.0))) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));

int posix_spawnattr_setsigmask(posix_spawnattr_t * restrict,
    const sigset_t * restrict) __attribute__((availability(macos,introduced=10.5))) __attribute__((availability(ios,introduced=2.0))) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));
# 131 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/spawn.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_size_t.h" 1 3 4
# 132 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/spawn.h" 2 3 4



int posix_spawnattr_getbinpref_np(const posix_spawnattr_t * restrict,
    size_t, cpu_type_t *restrict, size_t *restrict) __attribute__((availability(macos,introduced=10.5))) __attribute__((availability(ios,introduced=2.0))) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));

int posix_spawnattr_getarchpref_np(const posix_spawnattr_t * restrict,
    size_t, cpu_type_t *restrict, cpu_subtype_t *restrict, size_t *restrict) __attribute__((availability(macos,introduced=11.0))) __attribute__((availability(ios,introduced=14.0))) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));

int posix_spawnattr_setauditsessionport_np(posix_spawnattr_t * restrict,
    mach_port_t) __attribute__((availability(macos,introduced=10.6))) __attribute__((availability(ios,introduced=3.2)));

int posix_spawnattr_setbinpref_np(posix_spawnattr_t * restrict,
    size_t, cpu_type_t *restrict, size_t *restrict) __attribute__((availability(macos,introduced=10.5))) __attribute__((availability(ios,introduced=2.0))) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));

int posix_spawnattr_setarchpref_np(posix_spawnattr_t * restrict,
    size_t, cpu_type_t *restrict, cpu_subtype_t *restrict, size_t *restrict) __attribute__((availability(macos,introduced=11.0))) __attribute__((availability(ios,introduced=14.0))) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));

int posix_spawnattr_setexceptionports_np(posix_spawnattr_t * restrict,
    exception_mask_t, mach_port_t,
    exception_behavior_t, thread_state_flavor_t) __attribute__((availability(macos,introduced=10.5))) __attribute__((availability(ios,introduced=2.0))) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));

int posix_spawnattr_setspecialport_np(posix_spawnattr_t * restrict,
    mach_port_t, int) __attribute__((availability(macos,introduced=10.5))) __attribute__((availability(ios,introduced=2.0))) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));

int posix_spawnattr_setnosmt_np(const posix_spawnattr_t * restrict attr) __attribute__((availability(macos,introduced=11.0)));





int posix_spawnattr_set_csm_np(const posix_spawnattr_t * restrict attr, uint32_t flags) __attribute__((availability(macos,introduced=11.0)));
# 173 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/spawn.h" 3 4
int posix_spawn_file_actions_addinherit_np(posix_spawn_file_actions_t *,
    int) __attribute__((availability(macos,introduced=10.7))) __attribute__((availability(ios,introduced=4.3))) __attribute__((availability(watchos,unavailable))) __attribute__((availability(tvos,unavailable)));

int posix_spawn_file_actions_addchdir_np(posix_spawn_file_actions_t *,
    const char * restrict) __attribute__((availability(macos,introduced=10.15))) __attribute__((availability(ios,unavailable))) __attribute__((availability(tvos,unavailable))) __attribute__((availability(watchos,unavailable)));

int posix_spawn_file_actions_addfchdir_np(posix_spawn_file_actions_t *,
    int) __attribute__((availability(macos,introduced=10.15))) __attribute__((availability(ios,unavailable))) __attribute__((availability(tvos,unavailable))) __attribute__((availability(watchos,unavailable)));
# 461 "/Users/kevin/projects/lattice/salt-front/runtime.c" 2




int32_t salt_process_exec(const char *program, const char *arg1,
                          const char *arg2, const char *arg3) {
  pid_t pid;
  char *argv[5];
  argv[0] = (char *)program;
  argv[1] = (char *)arg1;
  argv[2] = (char *)arg2;
  argv[3] = (char *)arg3;
  argv[4] = ((void*)0);


  int argc = 1;
  if (arg1)
    argc++;
  if (arg2)
    argc++;
  if (arg3)
    argc++;
  argv[argc] = ((void*)0);

  extern char **environ;
  int status;
  int ret = posix_spawn(&pid, program, ((void*)0), ((void*)0), argv, environ);
  if (ret != 0)
    return -1;

  waitpid(pid, &status, 0);
  if ((((*(int *)&(status)) & 0177) == 0))
    return (((*(int *)&(status)) >> 8) & 0x000000ff);
  return -1;
}



int64_t salt_process_exec_capture(const char *program, const char *arg1,
                                  char *out_buf, int64_t buf_size) {



  char cmd[1024];
  int cmd_len;
  if (arg1) {
    cmd_len = __builtin___snprintf_chk (cmd, sizeof(cmd), 0, __builtin_object_size (cmd, 2 > 1 ? 1 : 0), "%s %s", program, arg1);
  } else {
    cmd_len = __builtin___snprintf_chk (cmd, sizeof(cmd), 0, __builtin_object_size (cmd, 2 > 1 ? 1 : 0), "%s", program);
  }
  if (cmd_len < 0 || cmd_len >= (int)sizeof(cmd)) {

    return -1;
  }

  FILE *fp = popen(cmd, "r");
  if (!fp)
    return -1;

  int64_t total = 0;
  while (total < buf_size - 1) {
    int ch = fgetc(fp);
    if (ch == (-1))
      break;
    out_buf[total++] = (char)ch;
  }
  out_buf[total] = '\0';

  pclose(fp);
  return total;
}




# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/dirent.h" 1 3 4
# 66 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/dirent.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/dirent.h" 1 3 4
# 81 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/dirent.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_ino_t.h" 1 3 4
# 31 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/_types/_ino_t.h" 3 4
typedef __darwin_ino_t ino_t;
# 82 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/dirent.h" 2 3 4




#pragma pack(4)
# 98 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/dirent.h" 3 4
#pragma pack()
# 112 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/dirent.h" 3 4
struct dirent { __uint64_t d_ino; __uint64_t d_seekoff; __uint16_t d_reclen; __uint16_t d_namlen; __uint8_t d_type; char d_name[1024]; };
# 67 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/dirent.h" 2 3 4

# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/Availability.h" 1 3 4
# 69 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/dirent.h" 2 3 4




struct _telldir;


typedef struct {
 int __dd_fd;
 long __dd_loc;
 long __dd_size;
 char * __dd_buf;
 int __dd_len;
 long __dd_seek;
 __attribute__((__unused__)) long __padding;
 int __dd_flags;
 __darwin_pthread_mutex_t __dd_lock;
 struct _telldir *__dd_td;
} DIR;
# 108 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/dirent.h" 3 4
int closedir(DIR *) __asm("_" "closedir" );

DIR *opendir(const char *) __asm("_" "opendir" );

struct dirent *readdir(DIR *) __asm("_" "readdir" );
int readdir_r(DIR *, struct dirent *, struct dirent **) __asm("_" "readdir_r" );

void rewinddir(DIR *) __asm("_" "rewinddir" );

void seekdir(DIR *, long) __asm("_" "seekdir" );

long telldir(DIR *) __asm("_" "telldir" );
# 131 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/dirent.h" 3 4
__attribute__((availability(macosx,introduced=10.10)))
DIR *fdopendir(int) __asm("_" "fdopendir" );

int alphasort(const struct dirent **, const struct dirent **) __asm("_" "alphasort" );
# 149 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/dirent.h" 3 4
int dirfd(DIR *dirp) __attribute__((availability(macosx,introduced=10.8)));


int scandir(const char *, struct dirent ***,
    int (*)(const struct dirent *), int (*)(const struct dirent **, const struct dirent **)) __asm("_" "scandir" );







int scandir_b(const char *, struct dirent ***,
    int (^)(const struct dirent *) __attribute__((__noescape__)),
    int (^)(const struct dirent **, const struct dirent **) __attribute__((__noescape__)))
    __asm("_" "scandir_b" ) __attribute__((availability(macosx,introduced=10.6)));
# 174 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/dirent.h" 3 4
int getdirentries(int, char *, int __nbytes, long *)






      __asm("_getdirentries_is_not_available_when_64_bit_inodes_are_in_effect")



;

DIR *__opendir2(const char *, int) __asm("_" "__opendir2" );
# 537 "/Users/kevin/projects/lattice/salt-front/runtime.c" 2
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/errno.h" 1 3 4
# 23 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/errno.h" 3 4
# 1 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/errno.h" 1 3 4
# 80 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/sys/errno.h" 3 4
extern int * __error(void);
# 24 "/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk/usr/include/errno.h" 2 3 4
# 538 "/Users/kevin/projects/lattice/salt-front/runtime.c" 2



int32_t salt_errno() { return (int32_t)(*__error()); }



int64_t salt_opendir(const char *path) {
  DIR *dir = opendir(path);
  return (int64_t)dir;
}




int32_t salt_readdir(int64_t handle, char *name_buf, int64_t buf_cap,
                     int64_t *out_name_len, int32_t *out_is_dir) {
  DIR *dir = (DIR *)handle;
  struct dirent *entry;

  while ((entry = readdir(dir)) != ((void*)0)) {

    if (entry->d_name[0] == '.') {
      if (entry->d_name[1] == '\0')
        continue;
      if (entry->d_name[1] == '.' && entry->d_name[2] == '\0')
        continue;
    }

    int64_t name_len = (int64_t)strlen(entry->d_name);
    if (name_len >= buf_cap)
      name_len = buf_cap - 1;


    for (int64_t i = 0; i < name_len; i++) {
      name_buf[i] = entry->d_name[i];
    }
    name_buf[name_len] = '\0';

    *out_name_len = name_len;
    *out_is_dir = (entry->d_type == 4) ? 1 : 0;
    return 1;
  }

  *out_name_len = 0;
  *out_is_dir = 0;
  return 0;
}


void salt_closedir(int64_t handle) {
  if (handle != 0) {
    closedir((DIR *)handle);
  }
}
# 601 "/Users/kevin/projects/lattice/salt-front/runtime.c"
int64_t salt_get_prompt_count(void) { return 0; }
int64_t salt_get_prompt_token(int64_t idx) {
  (void)idx;
  return 0;
}


static void *__basalt_engine_ptr = ((void*)0);
static int64_t __basalt_pos = 0;
static int64_t __basalt_token = 1;
static int32_t __basalt_initialized = 0;

void wasm_set_engine_ptr(void *ptr) { __basalt_engine_ptr = ptr; }
void *wasm_get_engine_ptr(void) { return __basalt_engine_ptr; }
void wasm_set_pos(int64_t pos) { __basalt_pos = pos; }
int64_t wasm_get_pos(void) { return __basalt_pos; }
void wasm_set_token(int64_t tok) { __basalt_token = tok; }
int64_t wasm_get_token(void) { return __basalt_token; }
void wasm_set_initialized(int32_t v) { __basalt_initialized = v; }
int32_t wasm_get_initialized(void) { return __basalt_initialized; }
