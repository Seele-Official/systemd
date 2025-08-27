# 使用手册（Windows 用户态服务管理器）

本工具提供一个类似 Linux systemd 的轻量级服务管理方式：
- 主程序常驻运行（仅单实例），负责加载配置、启动/停止被管控的进程，并通过命名管道处理命令。
- 命令分为两类：
  1) 服务管理命令：对“被管控服务”进行 start/stop/status/reload-config；
  2) 主程序管理命令：通过 `setting` 子命令安装/卸载为系统服务、设置为用户登录自启、启动/停止主程序本身。

> 重要：服务管理命令必须在主程序运行时才会生效；否则会提示 “Service is not running.”。


## 快速上手

1) 将可执行文件 `systemd.exe` 与目录 `configs/` 放在同一目录。
2) 在 `configs/` 下为每个被管控服务创建一个 `.toml` 配置文件（见下文模板）。
3) 启动主程序（任选其一）：
   - 注册为当前用户登录自启：`systemd.exe setting --register`（推荐管理 GUI 程序）。
   - 安装为系统服务：`systemd.exe setting --install`（适合纯后台程序）。
4) 重启（或手动启动主程序），然后使用服务管理命令：`start/stop/status/reload-config`。


## 配置文件（configs/*.toml）

将每个服务配置为一个独立的 TOML 文件，示例：

```toml
[unit]
name = "example"                 # 唯一服务名（用于命令行）
# description = "示例服务，可选"

[service]
# Simple: 手动启动；Startup: 主程序启动时自动拉起
type = "Startup"
# 需要运行的可执行文件完整路径
path = "D:\\example.exe"
# 启动参数（可选）
args = ["-e", "example"]
# 环境变量（可选）
env = { ENV = "example" }
# 标准输出/错误重定向文件（可选；不配置则写入内置 log 目录）
stdout_path = "D:\\stdout.log"
stderr_path = "D:\\stderr.log"
```

字段说明：
- unit.name：必填，服务唯一名；后续通过该名称进行 start/stop/status。
- unit.description：可选，仅用于展示。
- service.type：`"Simple"` 或 `"Startup"`；`Startup` 会在主程序启动时自动拉起。
- service.path：必填，目标可执行文件路径。
- service.args：可选，启动参数数组。
- service.env：可选，环境变量字典。
- service.stdout_path / service.stderr_path：可选，如果不设置，日志将默认写入：
  - `<systemd.exe 所在目录>\log\<name>-stdout.log`
  - `<systemd.exe 所在目录>\log\<name>-stderr.log`

注意：
- 主程序加载配置时会读取 `systemd.exe` 同级目录下的 `configs/` 中的所有文件；请确保目录内均为有效 TOML，否则整批加载会失败。


## 主程序管理（setting 子命令）
用于安装/卸载/启动/停止主程序本身，或配置为登录自启。大部分操作需要在有权限的 PowerShell 中执行。

- 安装为系统服务（开机自启，运行于 Session 0；不适合直接交互 GUI）：
  - 安装：
    ```powershell
    .\systemd.exe setting --install
    ```
  - 启动已安装的系统服务：
    ```powershell
    .\systemd.exe setting --start-service
    ```
  - 停止已安装的系统服务：
    ```powershell
    .\systemd.exe setting --stop-service
    ```
  - 卸载：
    ```powershell
    .\systemd.exe setting --uninstall
    ```
  说明：安装后服务以 LocalSystem 账户运行，内部会以参数 `setting --run-as-service` 启动主程序（内部选项，无需手动调用）。

- 注册为“当前用户登录自启”（运行于当前用户会话，适合管理 GUI 程序）：
  - 注册：
    ```powershell
    .\systemd.exe setting --register
    ```
  - 取消注册：
    ```powershell
    .\systemd.exe setting --unregister
    ```
  说明：注册会在 HKCU\Software\Microsoft\Windows\CurrentVersion\Run 写入启动项，登录后以隐藏窗口启动主程序，对应内部参数 `setting --run-as-user`（无需手动调用）。

- 优雅停止正在运行的主程序（无论以哪种方式启动）：
  ```powershell
  .\systemd.exe setting --stop
  ```

权限与定位：
- 安装/卸载/启动/停止系统服务通常需要以管理员身份打开 PowerShell。
- 注册/取消用户登录自启不需要管理员权限。
- 主程序日志默认写入 `<systemd.exe 同级目录>\Systemd.log`。


## 服务管理命令（需要主程序已运行）
当主程序正在运行时，可通过以下命令管理具体服务：

- 启动服务：
  ```powershell
  .\systemd.exe start <name>
  ```
  返回：`Service '<name>' started successfully.` 或错误信息。

- 停止服务：
  ```powershell
  .\systemd.exe stop <name>
  ```

- 查看状态：
  ```powershell
  .\systemd.exe status <name>
  ```
  典型输出示例：
  ```
  example - 示例服务

  Type   :Startup 
  Status :Running
  ```
  当未运行或异常时，Status 可能显示 `ProcessNotFound`、`ProcessExited(<code>)` 或 `IoError(...)`。

- 重新加载全部配置：
  ```powershell
  .\systemd.exe reload-config
  ```
  说明：重新扫描并加载 `configs/` 下的全部配置文件；成功后新/改配置即可被后续命令使用。


## 目录与文件约定
- `systemd.exe`：主程序入口，仅单实例运行（命名互斥体保证）。
- `configs/`：服务配置目录，放置若干 `.toml` 文件。
- `log/`：被管控服务的默认日志目录（按服务名分文件）。
- `Systemd.log`：主程序运行日志（与可执行文件同级）。


## 进阶说明
- 单实例机制：主程序使用命名互斥体保证同一台机器仅有一个实例常驻；其他命令行调用会通过命名管道与之通信。
- 生命周期：`Startup` 类型的服务会在主程序启动后自动拉起；`Simple` 类型仅在显式执行 `start <name>` 时启动。
- 停止语义：`stop <name>` 会向被管控进程发送终止（Kill）并等待退出。


## 常见问题（FAQ）
- 运行服务命令时提示 “Service is not running.”？
  - 说明主程序未在当前机器上运行。请先通过注册表自启或安装为系统服务的方式启动主程序。
- GUI 程序无法显示窗口？
  - 若主程序以“系统服务”方式运行，运行环境是 Session 0，不具备桌面交互能力。请改用“注册为当前用户登录自启”。
- 修改了配置不生效？
  - 确认配置文件是合法 TOML，且全部文件均合法；然后执行 `reload-config` 重新加载。


## 命令速查表

- 服务管理（需主程序已运行）：
  - `start <name>`
  - `stop <name>`
  - `status <name>`
  - `reload-config`

- 主程序管理（setting 子命令）：
  - `setting --install | --uninstall`
  - `setting --start-service | --stop-service`
  - `setting --register | --unregister`
  - `setting --stop`
  - 内部：`setting --run-as-service`、`setting --run-as-user`（一般无需手动执行）

