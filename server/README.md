# GNS3 调试指南

GNS3 服务器安装指南见官网。我使用的是 Hyper-V 虚拟机，不过使用 vmware 或其他方式也是可以的。

服务器启动后，将本文件下的 `Dockerfile` 和 `sync.sh` scp 上传到 GNS3 服务器上。
运行 `docker build . -t <name:tag>` 构建镜像，然后在 GNS3 上添加这个镜像模板。（具体见官网）

Docker 虚拟机模板配置方式也可以从官网找到（设置网络设备，ip等等，可以用 ping 测试连通性）。

连接组网正常进行即可。

因为该镜像在服务器上，所以上传本项目的 ospfd 可执行文件需要两步：
1. 在项目根目录使用 `./server/upload.sh <GNS3-server-ip>` 将可执行文件上传 (debug + release)
2. 在 GNS3 服务器上使用 `sudo ./sync.sh` 将可执行文件同步到 docker 容器中（如果容器已经直接启动 ospfd 服务，则需要先关停）

注意：`sync.sh` 很简陋，所以如果 GNS3 服务器上有很多 Volume 大概会传错地方，所以请自行修改。

## wsl 连接 hyper-v 虚拟机指南

https://automatingops.com/allowing-windows-subsystem-for-linux-to-communicate-with-hyper-v-vms

省流：打开交换机转发
