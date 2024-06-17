/// reference:
/// https://stackoverflow.com/questions/57982213/how-to-add-route-programmatically-in-c-using-ioctl
/// https://zhuanlan.zhihu.com/p/618934877

#include <net/if.h>
#include <netinet/in.h>

typedef struct routing_item_t
{
    in_addr_t dest;
    in_addr_t mask;
    in_addr_t nexthop;
    char ifname[IF_NAMESIZE];
} routing_item_t;

/// @brief 添加路由表
/// @param  路由表项
/// @return 通常而言 -1 表示失败
int add_route(const routing_item_t *);

/// @brief 删除路由表
/// @param  路由表项（nexthop可以不填）
/// @return 通常而言 -1 表示失败
int delete_route(const routing_item_t *);

/// @brief 获取当前路由表（目前版本可能只支持Ubuntu）
/// @param  路由表项数组
/// @param  数组大小
/// @return 路由表项实际数量，-1 表示失败
int get_route_table(routing_item_t *, int);
