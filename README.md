# 简介
一个简单的 linux systemd like 的 windows 用户态服务管理工具。


## 使用方式

### 安装 systemd 到 windows service
```shell
systemd.exe --install
```

### 卸载已安装的 windows service
```shell
systemd.exe --uninstall
```

### 启动服务
```shell
systemd.exe start example # 这里是 toml 中设置的 name
```
### 关闭服务
```shell
systemd.exe stop example 
```

### 检查服务状态
```shell
systemd.exe status example 
```

## 如何设置一个开机自启动服务

定位到你的 `systemd` 目录

新建 `configs/example.toml`， 添加以下内容
```toml
[unit]
name = "example"                 # 服务的名称

[service]
type = "Startup"                 # 设置为自启动
path = "D:\\example.exe"         # 需要执行的文件目录
args = ["-e", "exapmle"]         # 启动参数
env = { ENV = "example"}         # 环境变量
stdout_path = "D:\\stdout.log"   # 重定向的 stdout 文件路径
stderr_path = "D:\\stderr.log"   # 重定向的 stderr 文件路径
```

接着安装 `systemd` 到系统
```
systemd.exe --install
```

然后重启，`example` 会在 `systemd` 启动后运行




