#include <unistd.h>
#include <sys/wait.h>

int main() {
    int fds1[2];
    int fds2[2];
    pipe(fds1);
    pipe(fds2);
    pid_t pid = fork();
    if (pid == 0) {
        dup2(fds1[0], STDIN_FILENO);
        dup2(fds2[1], STDOUT_FILENO);
        close(fds1[0]);
        close(fds1[1]);
        close(fds2[0]);
        close(fds2[1]);
        sleep(2);
        return 0;
    }
    close(fds1[0]);
    close(fds2[1]);
    waitpid(pid, NULL, 0);
    return 0;
}
