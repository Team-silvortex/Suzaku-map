/* Test-only barrier for one owned host's preference staging or directory sync. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

int fsync(int fd) {
    int (*real_fsync)(int) = dlsym(RTLD_NEXT, "fsync");
    if (real_fsync == NULL) { errno = ENOSYS; return -1; }
    const char *root = getenv("XDG_RUNTIME_DIR");
    const char *marker = getenv("SUZAKU_PREFERENCE_NATIVE_QA");
    const char *address = getenv("IBUS_ADDRESS");
    const char *prefix = "/tmp/suzaku-preference-native.";
    const char *display = getenv("DISPLAY"), *wayland = getenv("WAYLAND_DISPLAY");
    struct stat info;
    if (root == NULL || marker == NULL || strcmp(marker, "1") != 0 || address == NULL ||
        strncmp(root, prefix, strlen(prefix)) != 0 || strchr(root + strlen(prefix), '/') != NULL ||
        (display != NULL && *display) || (wayland != NULL && *wayland) ||
        lstat(root, &info) != 0 || !S_ISDIR(info.st_mode) || info.st_uid != getuid() || (info.st_mode & 0077)) {
        return real_fsync(fd);
    }
    char expected[PATH_MAX], link[64], path[PATH_MAX];
    snprintf(expected, sizeof(expected), "unix:path=%s/ibus.sock", root);
    if (strcmp(address, expected) != 0) { return real_fsync(fd); }
    snprintf(link, sizeof(link), "/proc/self/fd/%d", fd);
    ssize_t length = readlink(link, path, sizeof(path) - 1);
    if (length < 0) { return real_fsync(fd); }
    path[length] = '\0';
    snprintf(expected, sizeof(expected), "%s/candidate-preferences/.suzaku-", root);
    int save = strncmp(path, expected, strlen(expected)) == 0;
    snprintf(expected, sizeof(expected), "%s/candidate-preferences", root);
    int clear = strcmp(path, expected) == 0;
    if (!save && !clear) { return real_fsync(fd); }
    const char *kind = save ? "save" : "clear";
    char armed[PATH_MAX], entered[PATH_MAX], released[PATH_MAX], finished[PATH_MAX], failed[PATH_MAX];
    snprintf(armed, sizeof(armed), "%s/preference-%s.arm", root, kind);
    snprintf(entered, sizeof(entered), "%s/preference-%s.entered", root, kind);
    snprintf(released, sizeof(released), "%s/preference-%s.release", root, kind);
    snprintf(finished, sizeof(finished), "%s/preference-%s.finished", root, kind);
    snprintf(failed, sizeof(failed), "%s/preference-%s.fail", root, kind);
    if (rename(armed, entered) != 0) { return real_fsync(fd); }
    struct timespec pause = {0, 10000000};
    for (int attempt = 0; attempt < 300 && access(released, F_OK) != 0; attempt++) { nanosleep(&pause, NULL); }
    int result;
    if (access(failed, F_OK) == 0) { errno = EIO; result = -1; }
    else { result = real_fsync(fd); }
    int saved_errno = errno;
    int done = open(finished, O_WRONLY | O_CREAT | O_EXCL | O_CLOEXEC, 0600);
    if (done >= 0) { close(done); }
    errno = saved_errno;
    return result;
}
