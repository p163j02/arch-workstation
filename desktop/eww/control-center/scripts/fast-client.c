/* Native shebang dispatcher: fast local IPC, Python fallback before delivery only. */
#define _GNU_SOURCE
#include <sys/socket.h>
#include <sys/un.h>
#include <sys/time.h>
#include <unistd.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>

static void fallback(int argc, char **argv) {
    char **args = calloc(argc + 1, sizeof(char *));
    args[0] = "python3";
    for (int i=1;i<argc;i++) args[i]=argv[i];
    execvp(args[0], args);
    perror("python3"); exit(127);
}
static int quote(char *out, size_t cap, size_t *n, const char *s) {
    if (*n+2>=cap) return -1;
    out[(*n)++]='"';
    for (;*s;s++) {
        unsigned char c=*s;
        if (c<32) return -1;
        if (*n+3>=cap) return -1;
        if (c=='"' || c=='\\') out[(*n)++]='\\';
        out[(*n)++]=c;
    }
    out[(*n)++]='"'; return 0;
}
int main(int argc, char **argv) {
    if (argc<2) return 2;
    const char *base=strrchr(argv[1],'/'); base=base?base+1:argv[1];
    int popup=!strcmp(base,"popup-manager.py");
    int action=!strcmp(base,"backend.py") && argc>=4 && !strcmp(argv[2],"action");
    if ((!popup && !action) || (popup && argc>2 && !strcmp(argv[2],"cleanup")) || getenv("CC_DIRECT")) fallback(argc,argv);
    struct sockaddr_un addr={.sun_family=AF_UNIX};
    const char *runtime=getenv("XDG_RUNTIME_DIR"); char default_dir[64];
    if (!runtime) { snprintf(default_dir,sizeof(default_dir),"/run/user/%u",getuid()); runtime=default_dir; }
    if (snprintf(addr.sun_path,sizeof(addr.sun_path),"%s/waybar-control-center/dispatch.sock",runtime)>=(int)sizeof(addr.sun_path)) fallback(argc,argv);
    int fd=socket(AF_UNIX,SOCK_STREAM|SOCK_CLOEXEC,0);
    if (fd<0 || connect(fd,(struct sockaddr*)&addr,sizeof(addr))) { if(fd>=0)close(fd); fallback(argc,argv); }
    struct timeval timeout={.tv_sec=15};
    setsockopt(fd,SOL_SOCKET,SO_RCVTIMEO,&timeout,sizeof(timeout));
    setsockopt(fd,SOL_SOCKET,SO_SNDTIMEO,&timeout,sizeof(timeout));
    char request[4096]; size_t n=0;
    n=snprintf(request,sizeof(request),"{\"kind\":\"%s\",\"args\":[",popup?"popup":"action");
    int start=popup?2:3;
    for(int i=start;i<argc;i++) {
        if(i>start)request[n++]=',';
        if(quote(request,sizeof(request),&n,argv[i])) { close(fd); fprintf(stderr,"Invalid control-center arguments\n");return 2; }
    }
    if(n+4>=sizeof(request)){close(fd);return 2;}
    memcpy(request+n,"]}\n",3);n+=3;
    size_t sent=0;
    while(sent<n) { ssize_t k=send(fd,request+sent,n-sent,MSG_NOSIGNAL); if(k<=0){perror("control-center send");close(fd);return 1;}sent+=k; }
    /* Never replay an accepted request: that could toggle twice. */
    char response[4096]; size_t used=0;
    while(used<sizeof(response)-1) {
        ssize_t k=read(fd,response+used,sizeof(response)-1-used);
        if(k<=0)break;
        used+=k;if(memchr(response,'\n',used))break;
    }
    close(fd);response[used]=0;
    if(!strncmp(response,"OK\n",3)) return 0;
    fprintf(stderr,"Control Center: %s\n",used?response:"service did not acknowledge request");return 1;
}
