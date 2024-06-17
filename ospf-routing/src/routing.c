#include "routing.h"

#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include <net/route.h>
#include <sys/ioctl.h>
#include <arpa/inet.h>
#include <netinet/in.h>

int add_route(const routing_item_t *_r)
{
    int fd = socket(PF_INET, SOCK_DGRAM, IPPROTO_IP);
    if (fd < 0) return -1;

    struct rtentry route;
    struct sockaddr_in *addr;
    memset(&route, 0, sizeof(route));

    addr = (struct sockaddr_in *)&route.rt_dst;
    addr->sin_family = AF_INET;
    addr->sin_addr.s_addr = _r->dest;

    addr = (struct sockaddr_in *)&route.rt_gateway;
    addr->sin_family = AF_INET;
    addr->sin_addr.s_addr = _r->nexthop;

    addr = (struct sockaddr_in *)&route.rt_genmask;
    addr->sin_family = AF_INET;
    addr->sin_addr.s_addr = _r->mask;

    route.rt_flags = RTF_UP | RTF_GATEWAY;
    route.rt_dev = (char *)_r->ifname;

    int ret = ioctl(fd, SIOCADDRT, &route);
    close(fd);
    return ret;
}

int delete_route(const routing_item_t *_r)
{
    int fd = socket(PF_INET, SOCK_DGRAM, IPPROTO_IP);
    if (fd < 0) return -1;

    struct rtentry route;
    struct sockaddr_in *addr;
    memset(&route, 0, sizeof(route));

    addr = (struct sockaddr_in *)&route.rt_dst;
    addr->sin_family = AF_INET;
    addr->sin_addr.s_addr = _r->dest;

    addr = (struct sockaddr_in *)&route.rt_genmask;
    addr->sin_family = AF_INET;
    addr->sin_addr.s_addr = _r->mask;

    route.rt_flags = RTF_UP | RTF_GATEWAY;
    route.rt_dev = (char *)_r->ifname;
 
    int ret = ioctl(fd, SIOCDELRT, &route);
    close(fd);
    return ret;
}

int get_route_table(routing_item_t *_arr, int _size)
{
    FILE *fp;
    int sz = 0, r, f;

    fp = fopen("/proc/net/route", "r");
    if (fp == NULL) return -1;
    /* Skip the first line. */
    r = fscanf(fp, "%*[^\n]\n");
    if (r < 0)
    {
        /* Empty line, read error, or EOF. Yes, if routing table
         * is completely empty, /proc/net/route has no header.
         */
        fclose(fp);
        return 0;
    }

    while (sz < _size)
    {
        r = fscanf(fp, "%15s%x%x%x%*d%*d%*d%x%*d%*d%*d\n",
                   _arr->ifname, &_arr->dest, &_arr->nexthop, &f, &_arr->mask);
        if ((r < 0) && feof(fp))
        /* EOF with no (nonspace) chars read. */
            break;
        if (f & RTF_UP) _arr++, sz++;
    }
    fclose(fp);
    return sz;
}
