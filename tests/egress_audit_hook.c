/* Destination-only observation for an owned, temporary LD_PRELOAD fixture.
 * No packet payload, URL, file path, environment dump or credential is logged.
 * This is not a syscall firewall: static binaries, direct syscalls, hidden libc
 * calls, send/sendmsg/write, and processes that drop LD_PRELOAD can bypass it.
 */
#define _GNU_SOURCE
#include <arpa/inet.h>
#include <dlfcn.h>
#include <errno.h>
#include <netdb.h>
#include <pthread.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/syscall.h>
#include <unistd.h>
#include <fcntl.h>

static char audit_path[4096];
static pthread_once_t symbols_once = PTHREAD_ONCE_INIT;
static pthread_once_t path_once = PTHREAD_ONCE_INIT;
static __thread bool resolving;
static int (*next_socket)(int, int, int);
static int (*next_connect)(int, const struct sockaddr *, socklen_t);
static ssize_t (*next_sendto)(int, const void *, size_t, int, const struct sockaddr *, socklen_t);
static int (*next_getaddrinfo)(const char *, const char *, const struct addrinfo *, struct addrinfo **);

static void symbols(void) {
    resolving = true;
    *(void **)(&next_socket) = dlsym(RTLD_NEXT, "socket");
    *(void **)(&next_connect) = dlsym(RTLD_NEXT, "connect");
    *(void **)(&next_sendto) = dlsym(RTLD_NEXT, "sendto");
    *(void **)(&next_getaddrinfo) = dlsym(RTLD_NEXT, "getaddrinfo");
    resolving = false;
}

static void initialize_path(void) {
    const char *path = getenv("EGRESS_AUDIT_LOG");
    if (path && path[0] && strlen(path) < sizeof audit_path)
        memcpy(audit_path, path, strlen(path) + 1);
}

/* One bounded append write per event, including across exec/fork descendants.
 * Logging failures do not change the application's socket results. The Python
 * fixture requires a startup marker and positive interception self-tests.
 */
static void record(const char *event, int family, const char *protocol,
                   const char *address, unsigned port, const char *hostname) {
    pthread_once(&path_once, initialize_path);
    if (!audit_path[0]) return;
    int saved_errno = errno;
    char line[1024];
    int size = snprintf(line, sizeof line,
        "{\"event\":\"%s\",\"pid\":%ld,\"family\":%d,\"protocol\":\"%s\","
        "\"address\":\"%s\",\"port\":%u,\"hostname\":\"%s\"}\n",
        event, (long)getpid(), family, protocol, address, port, hostname);
    if (size > 0 && (size_t)size < sizeof line) {
        int fd = (int)syscall(SYS_openat, AT_FDCWD, audit_path,
            O_WRONLY | O_APPEND | O_CREAT | O_CLOEXEC | O_NOFOLLOW, 0600);
        if (fd >= 0) {
            ssize_t written;
            do { written = syscall(SYS_write, fd, line, (size_t)size); }
            while (written < 0 && errno == EINTR);
            (void)syscall(SYS_close, fd);
        }
    }
    errno = saved_errno;
}

__attribute__((constructor)) static void audit_loaded(void) {
    record("loaded", 0, "none", "", 0, "");
}

static const char *protocol_for(int type, int protocol) {
    if (protocol == IPPROTO_TCP) return "tcp";
    if (protocol == IPPROTO_UDP) return "udp";
    if ((type & 0xf) == SOCK_STREAM) return "tcp";
    if ((type & 0xf) == SOCK_DGRAM) return "udp";
    return "other";
}

static void destination(const char *event, int fd,
                        const struct sockaddr *address, socklen_t length) {
    if (!address || length < sizeof(sa_family_t)) return;
    char text[INET6_ADDRSTRLEN] = "";
    unsigned port = 0;
    int type = 0;
    socklen_t type_length = sizeof type;
    int saved_errno = errno;
    if (address->sa_family == AF_INET && length >= sizeof(struct sockaddr_in)) {
        const struct sockaddr_in *v4 = (const struct sockaddr_in *)address;
        if (!inet_ntop(AF_INET, &v4->sin_addr, text, sizeof text)) { errno = saved_errno; return; }
        port = ntohs(v4->sin_port);
    } else if (address->sa_family == AF_INET6 && length >= sizeof(struct sockaddr_in6)) {
        const struct sockaddr_in6 *v6 = (const struct sockaddr_in6 *)address;
        if (!inet_ntop(AF_INET6, &v6->sin6_addr, text, sizeof text)) { errno = saved_errno; return; }
        port = ntohs(v6->sin6_port);
    } else { errno = saved_errno; return; } /* Do not log Unix paths. */
    (void)getsockopt(fd, SOL_SOCKET, SO_TYPE, &type, &type_length);
    record(event, address->sa_family, protocol_for(type, 0), text, port, "");
    errno = saved_errno;
}

int socket(int domain, int type, int protocol) {
    if (resolving) return (int)syscall(SYS_socket, domain, type, protocol);
    pthread_once(&symbols_once, symbols);
    if (domain == AF_INET || domain == AF_INET6)
        record("socket", domain, protocol_for(type, protocol), "", 0, "");
    if (next_socket) return next_socket(domain, type, protocol);
    errno = ENOSYS; return -1;
}

int connect(int fd, const struct sockaddr *address, socklen_t length) {
    if (resolving) return (int)syscall(SYS_connect, fd, address, length);
    pthread_once(&symbols_once, symbols);
    destination("connect", fd, address, length);
    if (next_connect) return next_connect(fd, address, length);
    errno = ENOSYS; return -1;
}

ssize_t sendto(int fd, const void *buffer, size_t length, int flags,
               const struct sockaddr *address, socklen_t address_length) {
    if (resolving) return syscall(SYS_sendto, fd, buffer, length, flags, address, address_length);
    pthread_once(&symbols_once, symbols);
    destination("sendto", fd, address, address_length);
    if (next_sendto) return next_sendto(fd, buffer, length, flags, address, address_length);
    errno = ENOSYS; return -1;
}

/* Resolver input can be arbitrary. Retain only a bounded DNS/numeric hostname;
 * invalid/userinfo-like strings become a fixed marker, never raw input.
 */
static const char *safe_hostname(const char *node) {
    if (!node || !node[0]) return "";
    size_t size = strnlen(node, 254);
    if (size > 253) return "invalid-hostname";
    if (strchr(node, ':')) {
        struct in6_addr v6;
        return inet_pton(AF_INET6, node, &v6) == 1 ? node : "invalid-hostname";
    }
    for (size_t index = 0; index < size; index++) {
        unsigned char ch = (unsigned char)node[index];
        if (!((ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z') ||
              (ch >= '0' && ch <= '9') || ch == '.' || ch == '-' || ch == '_'))
            return "invalid-hostname";
    }
    return node;
}

int getaddrinfo(const char *node, const char *service,
                const struct addrinfo *hints, struct addrinfo **result) {
    if (resolving) { errno = ENOSYS; return EAI_SYSTEM; }
    pthread_once(&symbols_once, symbols);
    unsigned port = 0;
    if (service && service[0]) {
        const char *cursor = service;
        unsigned candidate = 0;
        while (*cursor >= '0' && *cursor <= '9' && candidate <= 65535)
            candidate = candidate * 10 + (unsigned)(*cursor++ - '0');
        if (!*cursor && candidate <= 65535) port = candidate;
    }
    record("getaddrinfo", hints ? hints->ai_family : AF_UNSPEC,
        protocol_for(hints ? hints->ai_socktype : 0, hints ? hints->ai_protocol : 0),
        "", port, safe_hostname(node));
    if (next_getaddrinfo) return next_getaddrinfo(node, service, hints, result);
    errno = ENOSYS; return EAI_SYSTEM;
}
