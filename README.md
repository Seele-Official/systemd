# 简介
一个简单的 linux systemd like 的 windows 用户态服务管理工具。


## 如何设置一个开机自启动服务

1. 定位到你的 `systemd` 工具所在目录
2. 新建配置 `configs/example.toml`， 添加以下内容
    ```toml
    [unit]
    name = "example"                 # 服务标识名称

    [service]
    type = "Startup"                 # 设置为自启动
    path = "D:\\example.exe"         # 需要执行的文件目录
    args = ["-e", "exapmle"]         # 启动参数
    env = { ENV = "example"}         # 环境变量
    stdout_path = "D:\\stdout.log"   # 重定向的 stdout 文件路径
    stderr_path = "D:\\stderr.log"   # 重定向的 stderr 文件路径
    ```
3. 选择以下任一方式注册 `systemd` 为自启动
    ```shell
    # 方式一：注册到开机自启动注册表
    systemd.exe setting --register

    # 方式二：安装为系统服务
    systemd.exe setting --install
    ```
4. 重启系统后，`example` 服务将在 `systemd` 启动时自动运行。

> 为什么这里提供两种注册方式呢？
> 考虑到 windows 系统服务都是运行在 Session 0 环境，无法用户图形界面交互。
> 因此我们同时提供了注册表启动选项，便于您管理 GUI 应用程序。

更详细的操作请参阅 [TUTORIAL](TUTORIAL.md) 
