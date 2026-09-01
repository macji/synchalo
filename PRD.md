# SyncHalo 产品需求与技术设计文档（PRD）

| 项目 | 内容 |
| --- | --- |
| 产品名称 | SyncHalo |
| 文档版本 | v0.14 |
| 协议版本 | SyncHalo Protocol v1 |
| 文档状态 | 讨论稿 |
| 更新日期 | 2026-09-01 |
| 首发平台 | macOS、Ubuntu 24.04 ARM64 |

## 1. 文档目的

本文档定义 SyncHalo 桌面端 MVP 的产品范围、用户流程、功能要求和技术实现方案，作为产品、设计、开发和测试的共同基线。

本文档重点解决以下问题：

- 多台设备如何在没有云端账号和中心服务器的情况下互相发现与建立信任。
- A 向 B 同步、B 又向 A 同步时，如何排序、去重和防止无限回环。
- 文本剪贴板和文件为什么采用不同的同步语义。
- 文件如何获得高吞吐、完整性校验和断点续传能力。
- 剪贴板历史、文件历史、密钥、设置和传输进度如何安全地保存在本地。
- macOS 与 Ubuntu ARM64，尤其 Ubuntu Wayland 环境下的能力边界。

## 2. 产品概述

### 2.1 产品定位

SyncHalo 是一个面向个人多设备和小型可信工作组的局域网同步工具。它在同一局域网内自动同步纯文本剪贴板，并通过显式的粘贴或拖放操作向其他设备投递文件。

首版不依赖云端账号、云存储或公网中继，数据只在用户已授权的设备之间传输。

### 2.2 核心价值

- **快速**：复制文本后，其他在线设备几乎立即可粘贴。
- **直观**：文件拖入或粘贴一次，其他设备自动接收。
- **本地优先**：无云端上传，历史、设置和文件都保存在本机。
- **跨平台**：MVP 覆盖 macOS 与 Ubuntu ARM64，架构预留 Windows 和移动端能力。
- **高性能**：传输核心不经过 WebView，使用 Rust 和 QUIC 直接流式传输。
- **可恢复**：文件支持校验、失败重试和断点续传。

### 2.3 产品原则

1. 未经用户明确配对的设备不能接收任何内容。
2. 系统剪贴板中的文件不会被后台自动发送，必须由用户在 App 内显式粘贴或拖入。
3. 首版不把接收目录做成双向镜像目录，不传播删除和重命名。
4. 文本剪贴板强调实时性；文件同步强调可靠性。
5. 时间戳用于排序和展示，事件 ID 用于幂等与防回环，两者不能互相替代。
6. 默认不覆盖同名文件，不因清除历史而删除用户文件。
7. 剪贴板正文属于敏感数据，不写入普通日志，在本地加密保存。

## 3. 产品目标与非目标

### 3.1 MVP 目标

- 在同一二层局域网中自动发现 SyncHalo 设备。
- 通过用户确认的安全流程将设备加入同一同步空间。
- 在在线设备间自动同步纯文本剪贴板。
- 支持在 App 内粘贴或拖放一个或多个文件。
- 自动将文件接收到每台设备配置的接收目录，默认使用系统 Downloads。
- 展示剪贴板历史和文件同步历史。
- 文件支持分块校验、失败重试、断点续传和同名冲突处理。
- 支持托盘驻留、暂停同步和开机启动。
- 支持 macOS 安装包和 Ubuntu ARM64 的 deb/AppImage 安装包。

### 3.2 MVP 非目标

- 不做 Dropbox 式目录双向镜像。
- 不自动传播文件删除、移动或重命名。
- 不同步图片、HTML、RTF 或其他富文本剪贴板格式。
- 不提供公网连接、NAT 穿透、云端中继或云端备份。
- 不提供账号体系、企业组织和权限后台。
- 不保证跨 VLAN 或启用了无线终端隔离的网络可用。
- 不支持移动端；只在架构层预留。
- 不在 MVP 发布 Windows 安装包；Windows 作为后续平台。
- 不在首版实现大量设备间的 P2P 分块协同下载。

### 3.3 后续方向

- Windows 10/11 桌面端。
- Android、iOS 客户端。
- 剪贴板图片和富文本。
- 右键菜单“发送到 SyncHalo”。
- 可选择的文件夹镜像模式。
- 多网络发现、手动地址簿和可信中继。
- 设备间协同分发文件块。
- 企业设备策略和审计。

## 4. 目标用户与典型场景

### 4.1 目标用户

- 同时使用 Mac 与 Ubuntu ARM64 工作站或开发板的开发者。
- 在办公桌面与家庭设备间频繁复制命令、链接和文档的用户。
- 不希望把敏感内容上传云端的个人或小型团队。
- 需要在局域网内快速发送大文件的创作者和工程人员。

### 4.2 核心场景

#### 场景 A：复制文本

用户在 Mac 上复制一段命令，Ubuntu ARM64 设备在保持在线且启用剪贴板同步时自动写入同一文本。用户直接在 Ubuntu 中粘贴即可。

#### 场景 B：投递文件

用户在 Mac 上复制一个文件，然后打开 SyncHalo 按“粘贴文件”；或直接将文件拖入 App。Ubuntu ARM64 自动下载到配置的接收目录；反向投递同样可用。

#### 场景 C：查看历史

用户打开剪贴板历史，找到一小时前从 Linux 同步来的命令，单击后重新写入系统剪贴板。用户也可在文件历史中查看文件来自哪台设备、是否已送达以及保存位置。

#### 场景 D：设备暂时离线

发送文件时若没有勾选目标，App 只发送给当前在线设备并忽略离线设备；若用户明确勾选的目标已经离线或传输中掉线，该目标立即记为失败，设备重新上线后可由用户手动重试。剪贴板旧事件不会在设备重新上线后覆盖当前系统剪贴板。

## 5. 术语

| 术语 | 定义 |
| --- | --- |
| 设备 Device | 一次 SyncHalo 安装实例，拥有独立设备 ID 和身份密钥。 |
| 同步空间 Space | 用户的一组可信设备，MVP 默认每台设备只加入一个空间。 |
| 成员 Member | 已获得空间授权的设备。 |
| 同步码 Pairing Code | 添加设备时生成的一次性 6 位短码，默认 60 秒失效。 |
| 事件 Event | 一次不可变的剪贴板变化、文件投递或成员变更。 |
| 实时事件 Live Event | 在设备在线连接期间直接到达、允许影响系统剪贴板的事件。 |
| 回放事件 Replay Event | 重连或补齐历史时收到的旧事件，只能进入历史，不能覆盖当前剪贴板。 |
| 接收目录 Receive Directory | 接收到的文件最终保存位置，默认是系统 Downloads。 |
| Outbox | 尚未向全部目标完成投递的文件任务及可选缓存。 |
| HLC | Hybrid Logical Clock，混合逻辑时钟，用于跨设备事件排序。 |

## 6. 平台范围与兼容性

### 6.1 首版支持范围

| 平台 | MVP 范围 | 说明 |
| --- | --- | --- |
| macOS | Apple Silicon、Intel | 最低 macOS 13；后台剪贴板使用 NSPasteboard，文件粘贴板读取 file URL。 |
| Ubuntu X11 | Ubuntu 24.04 ARM64 | 使用 X11 clipboard selection，文件粘贴板读取 `text/uri-list`。 |
| Ubuntu Wayland | Ubuntu 24.04 ARM64 | compositor 支持 data-control 时启用后台监听；否则降级为 App 活跃或手动同步。 |

### 6.2 Linux Wayland 限制

Wayland 对后台读取和控制全局剪贴板有更严格的安全限制，且 data-control 协议的支持随 compositor 不同。SyncHalo 必须运行时检测能力，并在设置页显示：

- “完整后台同步”；
- “仅 App 活跃时可用”；
- “需要手动同步”；
- “当前桌面环境不支持”。

不能通过静默失败让用户误以为剪贴板已同步。

## 7. 信息架构与界面

### 7.1 主导航

```text
粘贴板
├── 全部历史
├── 本机产生
├── 其他设备
└── 收藏

同步文件
├── 全部
├── 已发送
├── 已接收
├── 进行中
└── 失败

设置
├── 同步码与添加设备
├── 我的设备（在线/离线）
├── 接收目录
├── 粘贴板与历史
├── 文件与缓存
├── 网络
├── 开机启动
└── 隐私与诊断
```

左侧栏只显示“粘贴板”“同步文件”“设置”三个一级菜单；设备管理不单独占用一级菜单，统一放入设置页。

### 7.2 全局布局

主窗口采用固定左右分栏：

- 左侧栏宽度约 224 px，包含产品状态、三个一级菜单和在线/离线设备数量摘要。
- 右侧内容区展示当前页面标题、页面操作和历史列表或设置内容。
- 粘贴板页默认作为启动首页。
- 同步文件页整个内容区域可接收文件拖放。
- 设置页首屏同时显示同步码和按在线/离线分组的设备列表。
- 默认窗口为 1080 × 720 px，最小窗口为 860 × 560 px；窗口低于 960 px 宽时左侧栏缩至 184 px，但不折叠成图标栏。
- 详细视觉 token、组件状态和无障碍规范见 `UI-DESIGN.md`。

全局字符线框：

```text
┌──────────────────────┬───────────────────────────────────────────────────────┐
│  ◉ SyncHalo          │  当前页面标题                              页面操作    │
│  ● 同步正常 · 2 在线 ├───────────────────────────────────────────────────────┤
│                      │                                                       │
│  ┌────────────────┐  │                                                       │
│  │ ▣  粘贴板      │  │                                                       │
│  └────────────────┘  │                    当前页面内容                       │
│    ⇄  同步文件       │                                                       │
│    ⚙  设置           │                                                       │
│                      │                                                       │
│                      │                                                       │
│  ──────────────────  │                                                       │
│  ● 2 台在线          │                                                       │
│  ○ 1 台离线          │                                                       │
│  [暂停同步]          │                                                       │
└──────────────────────┴───────────────────────────────────────────────────────┘
```

### 7.3 粘贴板页面布局

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│  粘贴板历史                         [搜索历史] [收藏] [清空]               │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  今天                                                                        │
│    cargo test --workspace             [悬停：预览 复制 收藏 删除]         │
│    MacBook Pro · 本机 · 14:32                                               │
│  ──────────────────────────────────────────────────────────────────────────  │
│  https://github.com/example/synchalo                                         │
│  ★ · Studio Ubuntu · 已接收 · 13:08                                         │
│  ──────────────────────────────────────────────────────────────────────────  │
│  会议结论：MVP 先覆盖 macOS 与 Ubuntu ARM64，其他平台后续处理……              │
│  Ubuntu · 已接收 · 10:46                                                      │
│                                                                              │
│  昨天                                                                        │
│  export RUST_LOG=synchalo=debug                                               │
│  MacBook Pro · 本机 · 昨天 21:17                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

- 历史采用列表行和细分隔线，不使用卡片堆叠。
- 单击历史行不执行复制；文本区域只用于阅读和选择文本。
- 预览、复制、收藏、删除四个图标默认隐藏，鼠标悬停或键盘焦点进入该行时显示。
- 预览图标打开只读文本框并显示完整原文。
- 仅已收藏历史在来源设备名前常驻显示“实心星标 ·”；未收藏不显示状态文字或图标。该星标属于内容状态，不属于悬停操作。
- 顶部“收藏”是可切换筛选项：启用时只显示收藏内容，再次点击恢复全部内容。
- 粘贴板页顶部不重复放置暂停/恢复按钮；全局暂停保留在左侧栏，暂停后的页面提示仍提供“恢复”。
- 复制只能通过悬停操作中的复制图标触发，并显示“已复制”提示；若复制内容与最新一条相同，不创建历史副本或广播新同步事件，复制不同的旧内容仍按一次新的本机复制处理。
- 单条删除不弹确认框，提供 5 秒撤销；“清空”必须二次确认。
- 历史每页固定显示 100 条；超过 100 条时显示页码、上一页和下一页。分页区上方不增加装饰分隔线，点击任一分页控件后列表立即回到开头。
- 正文保留换行，预览最多三行；第二行固定展示来源设备、方向和时间。

### 7.4 同步文件页面布局

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│  同步文件                                         [搜索历史] [☆ 收藏]   │
├──────────────────────────────────────────────────────────────────────────────┤
│  ┌───────────────── 我的设备 ────────────────┐ ┌──────── 文件同步 ───────┐ │
│  │ Jason 的 MacBook Air  本机 [显示同步码] │ │ 将同步到全部 2 台在线设备│ │
│  │ ☐ Studio Ubuntu       在线              │ │ 把文件拖入或者直接粘贴文件│ │
│  │ ☐ Desk Pi             在线              │ │ [+ 选择文件]              │ │
│  │ ☐ Office Ubuntu       离线              │ │ 复制文件后按 ⌘V 自动同步 │ │
│  └──────────────────────────────────────────┘ └────────────────────────────┘ │
│                                                                              │
│  同步历史                                                                  │
│  SyncHalo-design.zip     传输中 67%                    [再次同步] [☆] [取消] │
│  notes.pdf                                         [再次同步] [★] [打开] […] │
│  第 1 / 3 页 · 共 205 条 · 每页 100 条                  [‹] [1] [2] [3] [›] │
└──────────────────────────────────────────────────────────────────────────────┘
```

- 页面上部为左右区域：左侧“我的设备”，右侧“文件同步”；下部为横跨内容区的“同步历史”。
- 设备列表第一项始终是本机，显示系统取得的设备名和副文案“本机”；其右侧“显示同步码”打开覆盖整个右侧内容区的居中模态弹层，背景使用 70% 黑色蒙层但不遮挡左侧导航。其他设备行承担本次同步目标选择。
- 未勾选任何目标时，选择、拖入、粘贴和再次同步默认发送给当前全部在线设备；勾选后只发送给指定设备。
- 没有任何在线设备且没有指定目标时，所有文件入口统一提示：“当前没有可同步的在线设备，请至少保持 1 台其他设备在线”。
- 文件选择入口合并到拖入区；页面不显示“粘贴并同步”按钮，在同步文件页直接按 `Ctrl/Cmd + V` 即读取文件粘贴板并自动同步。
- 生产版拖放只使用 Tauri WebView 原生事件提供的绝对路径，不把浏览器 `File.name` 当作本地路径发送给 Rust。
- 进行中任务显示总体进度、速度和剩余时间；多目标任务可展开查看每台设备状态。
- 文件历史显示文件名、大小、方向、设备和时间；进行中与失败任务显示状态，成功任务不重复显示“已完成”；每行右侧常驻“再次同步”和收藏图标。
- 页面右上角“收藏”筛选与粘贴板页一致：点击只显示收藏文件，再次点击恢复全部历史。收藏状态持久化到 SQLite。
- 同步文件页不展示“全部 / 已发送 / 已接收 / 进行中 / 失败”标签筛选。搜索框位于右上角收藏按钮左侧，并与粘贴板搜索框使用相同尺寸、清除操作和 `Ctrl/Cmd + F` 行为。
- 同步历史由 Rust/SQLite 返回真实分页结果，每页固定 100 条；收藏和搜索在后端分页前执行，换页后滚动条回到页面开头。
- 删除历史默认不删除最终文件；删除实际文件必须使用独立危险操作并二次确认。

### 7.5 设置页面布局

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│  设置                                                                        │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  添加设备                                                                    │
│  ┌────────────────────────────────────────────────────────────────────────┐  │
│  │ 同步码                    482 913                     剩余 00:48       │  │
│  │ 在另一台设备输入此码。同步码一次有效。          [复制] [刷新] [加入] │  │
│  └────────────────────────────────────────────────────────────────────────┘  │
│                                                                              │
│  我的设备                                                                    │
│  在线 · 2                                                                    │
│  ● MacBook Pro      本机 · macOS                  当前设备                  │
│  ● Studio Ubuntu    Ubuntu ARM64 · 192.168.1.18   刚刚同步          [更多]  │
│                                                                              │
│  离线 · 1                                                                    │
│  ○ Ubuntu           Linux · 上次在线：昨天 22:14                    [更多]  │
│                                                                              │
│  文件接收                                                                    │
│  接收目录        ~/Downloads                              [更改] [打开]      │
│  自动同步粘贴板                                             [开关]           │
│  保存历史        7 天                                      [选择]           │
└──────────────────────────────────────────────────────────────────────────────┘
```

- 设置首屏必须同时看见同步码和至少两行设备，不把设备列表藏入二级页面。
- 同步码为一次性六位码，默认 60 秒失效，使用、超时或离开配对流程后立即失效。
- “生成同步码/刷新”右侧显示“加入”按钮；点击后使用与“清空历史”相同的模态框壳，在弹窗内输入一次性同步码。设置页不再常驻展示加入输入框。
- 我的设备固定按“在线、离线”分组；本机标记“当前设备”，且不能撤销自身。
- “添加设备”“我的设备”“文件接收”“粘贴板与历史”“启动与后台”“当前设备”等区块只显示标题，不在标题下重复展示辅助说明；标题列保持窄栏，把宽度优先留给右侧控件。
- 设备更多菜单包含重命名、暂停向此设备同步和撤销设备。

### 7.6 主题与交互状态

- Web 前端通过 CSS variables 定义浅色和深色 token，并以 `prefers-color-scheme` 跟随操作系统；系统切换后无需重启。
- 使用系统 UI 字体；正文 14 px、辅助信息 12 px、页面标题 20 px。
- 强调色只用于主操作和进行中状态；在线、完成、失败同时使用图标、文字和颜色表达。
- 图标使用统一线性 SVG，删除图标正常态为次级色，悬停和键盘聚焦时使用危险色。
- 所有图标按钮至少有 32 × 32 px 点击区，并提供 tooltip 与无障碍名称。
- 列表、搜索、弹窗和通知都支持键盘操作；正文与背景对比度目标至少 4.5:1。

### 7.7 系统托盘

托盘菜单包含：

- 打开 SyncHalo。
- 暂停/恢复所有同步。
- 暂停/恢复剪贴板同步。
- 粘贴并发送文件。
- 在线设备数量。
- 退出。

关闭主窗口默认隐藏到托盘；用户选择“退出”才终止后台服务。

- macOS 状态栏图标左键释放后直接显示、还原并聚焦主窗口；右键保留托盘菜单。
- Ubuntu ARM64 使用 AppIndicator。由于该接口不向 Tauri 提供图标点击事件，点击图标打开原生菜单，通过首项“打开 SyncHalo”显示并聚焦主窗口。

## 8. 用户流程

### 8.1 首次启动

1. App 生成随机设备 ID、设备身份密钥和默认设备名称。
2. 创建新的同步空间，或选择加入附近已有空间。
3. 请求必要权限，并解释剪贴板隐私风险。
4. 将接收目录设为系统 Downloads；用户可立即修改。
5. 显示附近设备和“添加设备”入口。
6. 默认启用剪贴板同步与本地历史，默认不开启开机启动，由用户确认后启用。

### 8.2 添加设备

1. 设备 A 在设置页点击“生成同步码”，进入 60 秒配对窗口。
2. A 显示一次性六位同步码，例如 `482 913`。
3. 设备 B 在设置页输入该同步码，并选择发现到的 A；mDNS 不可用时可同时输入 A 的地址。
4. 双方通过基于同步码的密码认证密钥交换建立临时加密连接，短码本身不以明文发送。
5. A 显示 B 的设备名称和平台，用户确认允许加入。
6. A 向 B 安全发送空间信息和成员列表；同步码立即失效。
7. 所有在线成员收到成员变更事件。
8. B 出现在设备列表中，并开始正常同步。

### 8.3 文本同步

1. 剪贴板适配层捕获本机纯文本变化。
2. 同步核心判断该变化是否为远端事件刚刚写入造成。
3. 若为真正的本机复制，则创建唯一剪贴板事件。
4. 事件写入本地历史并立即广播给在线成员。
5. 接收端验证空间、签名、事件 ID、大小和排序信息。
6. 若为实时事件且剪贴板同步开启，则写入系统剪贴板。
7. 接收端保存历史并标记该系统剪贴板变更为远端应用结果。

### 8.4 文件粘贴或拖放

1. 用户点击“粘贴文件”，App 读取系统剪贴板中的文件引用；纯文本不会进入该流程。
2. 或用户将一个或多个文件拖入 App。
3. App 展示文件数、总大小和目标设备范围。
4. 若用户没有勾选目标，发送给当前所有在线设备；若已经勾选，只发送给指定设备。
5. 在线目标立即开始接收；明确指定的离线目标直接记为失败，不创建等待任务。
6. 文件写入临时路径，校验完成后原子移动到接收目录。
7. 各目标状态和最终路径写入文件历史。

## 9. 功能需求

### 9.1 设备发现与连接

| 编号 | 要求 | 优先级 |
| --- | --- | --- |
| FR-DIS-001 | App 启动后自动发布并浏览局域网 SyncHalo 服务。 | P0 |
| FR-DIS-002 | 同一设备通过多个网卡、IPv4、IPv6 被发现时按设备 ID 去重。 | P0 |
| FR-DIS-003 | 用户可以选择允许参与发现的网络接口。 | P1 |
| FR-DIS-004 | mDNS 不可用时支持输入 IP 地址和端口连接。 | P0 |
| FR-DIS-005 | 设备离开、睡眠、切换 IP 后状态能自动更新和重连。 | P0 |
| FR-DIS-006 | 协议版本不兼容时显示明确提示，不尝试传输内容。 | P0 |

### 9.2 配对与空间成员

| 编号 | 要求 | 优先级 |
| --- | --- | --- |
| FR-PAIR-001 | 未配对设备不能发送或接收剪贴板、文件和历史。 | P0 |
| FR-PAIR-002 | 配对必须使用一次性同步码，并由已有成员确认新设备。 | P0 |
| FR-PAIR-003 | 配对窗口默认 60 秒，超时自动关闭。 | P0 |
| FR-PAIR-004 | 用户可以修改设备名称。 | P0 |
| FR-PAIR-005 | 用户可以撤销任意成员；撤销后立即断开并拒绝重连。 | P0 |
| FR-PAIR-006 | MVP 默认最多 10 台设备，超过时给出提示。 | P1 |

### 9.3 剪贴板同步

| 编号 | 要求 | 优先级 |
| --- | --- | --- |
| FR-CLIP-001 | 自动同步纯文本剪贴板。 | P0 |
| FR-CLIP-002 | 远端写入不得再次生成新的本地事件。 | P0 |
| FR-CLIP-003 | 默认最大文本大小为 1 MiB，超限时只记录失败提示。 | P0 |
| FR-CLIP-004 | 空文本不生成同步事件。 | P0 |
| FR-CLIP-005 | 用户可全局暂停，也可针对单台设备关闭接收。 | P0 |
| FR-CLIP-006 | 回放或离线补齐事件不得自动覆盖系统剪贴板。 | P0 |
| FR-CLIP-007 | 同时复制产生冲突时，各设备按同一排序键选出最终事件。 | P0 |
| FR-CLIP-008 | 不自动同步系统剪贴板中的文件列表。 | P0 |

### 9.4 剪贴板历史

| 编号 | 要求 | 优先级 |
| --- | --- | --- |
| FR-CH-001 | 显示内容预览、来源设备、方向和产生时间。 | P0 |
| FR-CH-002 | 历史行本身不触发复制；用户点击行内复制图标后写入系统剪贴板。 | P0 |
| FR-CH-003 | 支持搜索、收藏、删除单条和清空。 | P0 |
| FR-CH-004 | 默认保留 7 天或最多 500 条，以先达到者为准。 | P0 |
| FR-CH-005 | 支持 1 天、7 天、30 天、永久和不保存。 | P1 |
| FR-CH-006 | 收藏项不受自动过期影响。 | P1 |
| FR-CH-007 | 正文在本地加密，普通日志不得包含正文。 | P0 |
| FR-CH-008 | 默认每页 100 条，超过 100 条时由 Rust/SQLite 返回分页结果与总数。 | P0 |

MVP 历史规则：每台设备保存其产生或实际收到的事件。在线设备通常拥有一致的剪贴板历史；离线期间遗漏的剪贴板正文默认不补发。后续可增加“跨设备补齐历史”，但补齐事件只能进入历史，不能写入系统剪贴板。

### 9.5 文件同步

| 编号 | 要求 | 优先级 |
| --- | --- | --- |
| FR-FILE-001 | 支持在 App 内粘贴一个或多个文件。 | P0 |
| FR-FILE-002 | 支持拖放一个或多个文件。 | P0 |
| FR-FILE-003 | 未勾选目标时默认发送给全部在线设备；勾选后只发送给指定设备。 | P0 |
| FR-FILE-004 | 接收端自动保存，不需要再次确认。 | P0 |
| FR-FILE-005 | 支持传输进度、瞬时速度、平均速度和预计剩余时间。 | P0 |
| FR-FILE-006 | 支持失败后手动重试，并在接收端已有有效临时块时断点续传。 | P0 |
| FR-FILE-007 | 文件必须经过分块校验和整文件校验。 | P0 |
| FR-FILE-008 | 文件完成前不得以最终文件名暴露给用户。 | P0 |
| FR-FILE-009 | 同名同内容自动去重；同名不同内容自动生成新名称。 | P0 |
| FR-FILE-010 | 删除、移动或重命名接收文件不传播到其他设备。 | P0 |
| FR-FILE-011 | 首版不解析压缩包，不跟随符号链接。 | P0 |
| FR-FILE-012 | 页面使用左侧设备、右侧发送、下方历史的三分区布局，本机固定为设备列表第一项。 | P0 |
| FR-FILE-013 | 没有在线设备且没有指定目标时，所有文件入口显示统一的“至少保持 1 台其他设备在线”提示。 | P0 |
| FR-FILE-014 | 文件历史支持持久化收藏、顶部收藏筛选和按历史任务再次同步。 | P0 |
| FR-FILE-015 | 本机设备行可生成、显示和复制一次性同步码。 | P0 |
| FR-FILE-016 | 文件历史按每页 100 条进行 Rust/SQLite 后端分页，筛选后重新计算总数和页码。 | P0 |
| FR-FILE-012 | 文件夹投递作为 P1，首版可先限制为文件。 | P1 |

### 9.6 文件历史

| 编号 | 要求 | 优先级 |
| --- | --- | --- |
| FR-FH-001 | 显示文件名、大小、来源、方向、时间和状态。 | P0 |
| FR-FH-002 | 一个文件发往多台设备时显示每台设备的独立状态。 | P0 |
| FR-FH-003 | 支持重试、重新发送、打开文件、显示所在目录和复制路径。 | P0 |
| FR-FH-004 | 删除历史默认不删除文件，删除文件必须单独确认。 | P0 |
| FR-FH-005 | 展示标准化失败原因和建议操作。 | P0 |

### 9.7 接收目录与设置

| 编号 | 要求 | 优先级 |
| --- | --- | --- |
| FR-SET-001 | 默认接收目录为系统 Downloads。 | P0 |
| FR-SET-002 | 用户可以选择任意有写权限的本地目录。 | P0 |
| FR-SET-003 | 目录失效、只读或空间不足时立即停止新任务并提示。 | P0 |
| FR-SET-004 | 修改目录只影响新任务，不自动移动历史文件。 | P0 |
| FR-SET-005 | 支持开机启动和后台驻留；传输完成与错误通知固定开启，不显示设置开关。 | P0 |
| FR-SET-006 | 支持导出脱敏诊断包。 | P1 |

## 10. 同步语义与冲突规则

### 10.1 统一事件模型

所有需要跨设备传播的状态都封装为不可变事件：

```protobuf
message EventEnvelope {
  bytes event_id = 1;             // UUIDv7，128 bit
  bytes space_id = 2;             // 同步空间 ID
  bytes origin_device_id = 3;     // 事件最初产生设备
  uint64 origin_sequence = 4;     // 每个 origin 单调递增
  int64 created_at_utc_ms = 5;    // 展示用物理时间
  HlcTimestamp hlc = 6;           // 排序用混合逻辑时钟
  EventKind kind = 7;
  bytes content_hash = 8;         // 载荷摘要
  uint32 schema_version = 9;
  bytes payload = 10;
  bytes signature = 11;           // 来源设备对上述字段签名
}

message HlcTimestamp {
  int64 physical_ms = 1;
  uint32 logical = 2;
}
```

`received_at_utc_ms`、连接 ID、是否实时到达等属于接收端本地投递信息，不写入签名事件本体。

### 10.2 为什么不能只使用普通时间戳

不同设备可能存在系统时钟偏差；同时复制时也可能产生相同毫秒时间。只用 `created_at` 会导致各设备排序不一致。

SyncHalo 使用 HLC：

- 本机产生事件时，物理部分取本机 UTC 时间和上一 HLC 物理值的较大者。
- 物理值前进时逻辑计数归零；同一物理值上继续产生事件时逻辑计数加一。
- 收到远端事件时，将本地 HLC 与远端 HLC 合并，再处理后续本机事件。
- 如果 B 已经收到 A 的事件，B 之后产生的新事件必然排在 A 之后。

最终稳定排序键为：

```text
(hlc.physical_ms, hlc.logical, origin_device_id, event_id)
```

`created_at_utc_ms` 只用于向用户显示当地时间，不作为唯一冲突依据。检测到设备时钟偏差超过 5 分钟时，App 显示“系统时间可能不准确”，但仍使用 HLC 保持确定性排序。

### 10.3 A ↔ B 防回环机制

时间戳不负责防循环。防回环依赖三层机制：

1. **事件幂等**：数据库中 `event_id` 唯一；见过的事件直接忽略载荷应用。
2. **来源序号**：`(origin_device_id, origin_sequence)` 唯一，用于发现重复事件或序号缺口。
3. **系统剪贴板回声抑制**：写入远端文本前记录 `event_id`、内容哈希和平台剪贴板变更序号；随后的系统变更通知若与之匹配，则不生成新事件。

设备转发时必须转发完全相同的签名事件，不能把收到的内容重新包装为自己的新剪贴板事件。

### 10.4 同时复制冲突

当 A 和 B 几乎同时复制不同文本：

1. 两个事件都进入历史。
2. 各设备按稳定排序键选出较新的事件作为系统剪贴板最终值。
3. 已被用户在本机随后主动复制的新事件，不得被排序更旧的远端事件覆盖。
4. UI 可将未成为最终值的事件标记为“同时发生”，但不丢弃历史。

### 10.5 剪贴板实时与历史语义

- 连接存续期间直接收到的剪贴板事件标记为 `LIVE`，可以写入系统剪贴板。
- 重连补发、历史查询或来自本地数据库的事件标记为 `REPLAY`，只能展示或由用户主动点击复制。
- 暂停剪贴板同步期间收到的事件可进入历史，但不得写入系统剪贴板。
- 用户点击历史项复制时先写入本机系统剪贴板并抑制监听回声；若内容与最新一条相同，不创建历史副本或再次同步，复制不同的旧内容则成为新的本机事件。

### 10.6 文件语义

每次显式粘贴或拖放创建一个文件事件；即使文件名和路径相同，只要内容不同就是新事件。

- 同名且整文件哈希相同：不重复写入，历史标记“已存在”。
- 同名但哈希不同：使用 `名称 (from 设备名, YYYY-MM-DD HHmmss).扩展名`。
- 文件接收完成后再修改，不触发二次同步。
- 文件删除不生成事件。
- 修改接收目录不移动已接收文件。

### 10.7 事件投递与可靠性等级

不同数据类型不能采用同一套重试策略：

| 数据类型 | 本地先落盘 | 在线发送 | 离线补发 | 自动应用规则 |
| --- | --- | --- | --- | --- |
| 剪贴板文本 | 是 | 立即广播 | MVP 默认不补正文 | 只有实时到达事件写入系统剪贴板。 |
| 文件事件 | 是 | 立即 Offer | 否 | 未指定目标时只投递给在线设备；指定目标离线时立即失败。 |
| 文件数据块 | 接收进度落盘 | 流式发送 | 仅手动重试 | 只写临时文件，全部校验后提交。 |
| 成员变更 | 是 | 立即广播 | 是 | 所有有效成员最终必须收敛到同一成员版本。 |
| 本机设置 | 是 | 不发送 | 不适用 | 仅影响本机。 |

通用事件投递流程：

1. 产生事件的设备在同一数据库事务中分配 `origin_sequence`、更新 HLC、写入事件和目标投递记录。
2. 事务成功后才允许向网络发送，避免“已经发出但本地没有记录”。
3. 接收端先验证空间、成员身份、签名、大小限制和协议版本。
4. 接收端在数据库事务中插入事件；若 `event_id` 或来源序号已存在，则按幂等成功处理。
5. 接收端落盘成功后返回 `EventAck(PERSISTED)`，不能在仅收到网络字节时提前确认。
6. 发送端收到 Ack 后更新该目标的投递记录。

剪贴板事件仅在创建时已经在线的成员间短暂重试，实时窗口默认 5 秒；超过窗口后不再自动投递正文，防止旧内容覆盖用户当前剪贴板。文件传输失败后由用户手动重试，成员事件采用持久重试。

MVP 使用空间成员间的直接全连接拓扑。来源设备直接向每个在线成员发送剪贴板和文件事件，不依赖某一台设备长期充当中心节点。成员变更事件可以转发，但转发时必须保留完全相同的事件 ID、来源签名和载荷。

## 11. 技术架构

### 11.1 技术选型

| 层 | 选型 | 原因 |
| --- | --- | --- |
| 桌面框架 | Tauri 2 | 使用系统 WebView 承载静态 Web 前端，并由 Rust 管理窗口、生命周期、托盘、权限和安装包。 |
| Web 前端 | React + TypeScript + Vite | 使用 HTML/CSS 实现左右分栏、虚拟列表、弹窗、拖放反馈和系统明暗主题；构建为本地静态 SPA。 |
| 前后端边界 | Tauri Commands + Events/Channels | 请求使用 command，后台状态与节流后的进度使用 event/channel；只传结构化元数据，不传文件数据面。 |
| 核心运行时 | Rust + Tokio | 跨平台、内存安全，适合大量异步网络和文件 I/O。 |
| 发现 | mDNS/DNS-SD | 同一局域网自动发现，不需要中心服务。 |
| 传输 | QUIC + TLS 1.3 | 加密、多路流、低延迟连接和断线恢复基础能力。 |
| 协议编码 | Protocol Buffers | 稳定字段编号和向前兼容，便于后续移动端实现。 |
| 数据库 | SQLite（本地） | 无服务依赖，适合事件、设置和任务元数据。 |
| 内容校验 | BLAKE3 | 支持快速流式哈希、SIMD 和并行计算。 |
| 密钥存储 | `synchalo.key`（0600）+ SQLite wrapped secrets | Rust 生成随机 KEK 并存入当前用户独占文件；SQLite 保存 wrapped DEK 与加密设备身份，正常启动不访问 Keychain/Secret Service。 |
| 本地加密 | XChaCha20-Poly1305 | 用于剪贴板正文加密、DEK 包装和元数据认证。 |

### 11.2 进程内分层

```text
Tauri Window
└── System WebView
    └── React + TypeScript + CSS
        │ invoke / events / channels
        │ 仅传命令、ViewModel 和节流后的进度
        ▼
Tauri IPC Boundary
└── Application Service（Rust）
    ├── DeviceService
    ├── ClipboardService
    ├── TransferService
    ├── HistoryService
    └── SettingsService
           │
           ▼
Domain Core（Rust）
├── Event / HLC / Dedup
├── Device / Space / Trust
├── File Manifest / Transfer State
└── Policy / Conflict Rules
           │
           ▼
Infrastructure（Rust）
├── mDNS Discovery
├── QUIC Transport
├── SQLite Repository
├── Key Store
├── File System
└── Platform Clipboard Adapters
```

Web 前端不能直接打开监听端口、读取私钥、访问数据库或传送文件字节。所有高权限和大数据操作都由 Rust 后台服务完成；前端只能调用显式注册并授权的 Tauri command 或插件能力，并消费只含展示数据的事件或 channel。

生产包只加载随应用打包的本地静态资源，不加载远程页面，不依赖 SSR 或本地 HTTP 业务服务。CSP 默认拒绝远程脚本和任意网络连接；若后续加入更新检查，必须为对应目标单独开放权限。

### 11.3 代码目录

项目目录去掉 `synchalo-` 前缀：

```text
synchalo/
├── apps/
│   └── desktop/
│       ├── package.json
│       ├── vite.config.ts
│       ├── index.html
│       ├── src/                  # Web 前端
│       │   ├── main.tsx
│       │   ├── App.tsx
│       │   ├── routes.ts
│       │   ├── api/              # 类型化 Tauri command/event 封装
│       │   ├── components/       # Sidebar、Toolbar、Dialog、Toast 等
│       │   ├── pages/            # Clipboard、Files、Settings
│       │   ├── state/            # 仅 UI 与 ViewModel 状态
│       │   └── styles/           # tokens、主题和全局样式
│       └── src-tauri/            # Tauri 壳与 Rust 入口
│           ├── Cargo.toml
│           ├── tauri.conf.json
│           ├── capabilities/
│           │   └── default.json
│           └── src/
│               ├── main.rs
│               ├── lib.rs
│               ├── commands.rs
│               ├── events.rs
│               └── tray.rs
├── crates/
│   ├── core/                    # 事件、HLC、策略、领域模型
│   ├── network/                 # mDNS、QUIC、配对、连接管理
│   ├── transfer/                # 文件分块、校验、续传
│   ├── clipboard/               # 剪贴板抽象与回声抑制
│   ├── storage/                 # SQLite、迁移、加密字段
│   └── platform/                # macOS/Ubuntu Linux 实现
├── protocol/
│   └── synchalo.proto
├── tests/
│   ├── integration/
│   ├── compatibility/
│   └── performance/
├── Cargo.toml
├── PRD.md
└── UI-DESIGN.md
```

### 11.4 主要组件建议

- `tauri` 2.x：窗口、WebView、生命周期、托盘、IPC 和应用打包。
- React + TypeScript + Vite：静态 SPA、组件与类型安全的 UI 开发。
- CSS Grid/Flexbox + CSS variables：左右分栏、列表布局和明暗主题；不引入重型 UI 组件库作为 MVP 前提。
- 虚拟列表库或等效自研组件：只渲染可见的长历史列表。
- Tauri 官方 dialog、opener、autostart、notification 插件：系统选择框、打开路径、开机启动和通知。
- `tokio`：异步任务和 I/O。
- `quinn` + `rustls`：QUIC 与 TLS。
- `mdns-sd`：跨平台 mDNS/DNS-SD。
- `prost`：Protocol Buffers。
- `rusqlite`：SQLite；使用 bundled SQLite 并锁定安全版本。
- `blake3`：文件块与整文件摘要。
- `uuid`：UUIDv7 事件 ID。
- `ed25519-dalek`：设备签名。
- `chacha20poly1305`：历史正文加密。
- `keyring` 或平台原生 API：长期密钥保护。
- `tracing`：结构化日志和脱敏诊断。

最终依赖版本必须在开发初始化时锁定，不能在 PRD 中使用无上限的浮动版本。

### 11.5 WebView 兼容策略

- macOS 使用 WKWebView，Ubuntu 使用 WebKitGTK；MVP 的 CSS 与 JavaScript 基线以两端最低支持版本的交集为准。
- 核心布局只依赖稳定的 CSS Grid/Flexbox，不依赖实验性浏览器特性。
- WebView 差异只能影响展示层，不能改变同步、传输、加密或持久化语义。
- CI 在 macOS ARM64 与 Ubuntu ARM64 原生 runner 分别执行测试和安装包构建；Ubuntu 覆盖目标 WebKitGTK 版本。

## 12. 局域网发现方案

### 12.1 mDNS 服务

每台设备在允许的私有网络接口上发布：

```text
Service Type: _synchalo._udp.local.
Instance:     sh-<device-id-prefix>
Port:         当前 QUIC UDP 端口
```

TXT 记录仅包含非敏感信息：

```text
v=1                       # 发现协议版本
id=<base32-device-id>     # 随机设备 ID
caps=clip,file,resume     # 能力位
pair=0|1                  # 是否处于可配对窗口
space=<short-hmac-hint>   # 已配对设备识别空间，不暴露 space_id
```

不广播：

- 剪贴板内容；
- 文件名；
- 用户账号；
- 完整公钥；
- 本机真实用户名；
- 接收目录路径。

### 12.2 网络接口策略

- 默认启用标记为 Private/Home 的 Wi-Fi 和以太网。
- 默认忽略 loopback、Docker、虚拟机、VPN、点对点和公网接口。
- 设置页允许用户手动启用或禁用接口。
- IPv4 和 IPv6 地址都可发现，同一 `device_id` 合并为一个设备，并维护多个候选地址。
- 地址连接采用 Happy Eyeballs 风格的短延迟竞速，但只能保留一个逻辑连接。

### 12.3 端口策略

- mDNS 使用标准 UDP 5353。
- QUIC 默认尝试 UDP 53317；占用时依次尝试 53318–53327，并通过 SRV 记录广播实际端口。
- Ubuntu 打包说明列出 mDNS 与 QUIC 所需的本机防火墙端口；应用不自动修改系统防火墙规则。
- 用户可在高级设置中固定 QUIC 端口。

### 12.4 连接去重

两个设备可能同时向对方拨号。使用设备 ID 确定连接发起者：字节序较小的一方优先主动连接；另一方等待。若仍产生两条连接，比较双方声明的连接 nonce，只保留确定性选中的一条。

### 12.5 发现降级

当 mDNS 无结果时：

1. 提供 `IP:Port` 手动连接。
2. 检测本机是否有可用私有接口。
3. 检测 UDP 端口绑定和防火墙错误。
4. 提示企业 Wi-Fi、访客网络或 AP Isolation 可能阻止设备互访。
5. MVP 不自动回退到任何公网服务。

## 13. 配对、身份与连接安全

### 13.1 设备身份

首次启动生成：

- 128 bit 随机 `device_id`；
- Ed25519 长期身份密钥；
- 自签名 TLS 身份证书或与设备公钥绑定的证书材料；
- 单调递增的本机事件序号。

设备证书私钥和 Ed25519 签名密钥使用当前 DEK 加密后保存到 SQLite `local_secrets`；KEK 只存在当前用户权限为 `0600` 的 `synchalo.key` 中。密钥和私钥不进入 WebView。

### 13.2 同步空间

首次创建空间时生成：

- 随机 `space_id`；
- 256 bit `space_secret`；
- 初始成员列表。

新设备通过已验证的配对连接取得空间信息。正常连接需要同时满足：

1. TLS 连接加密成功；
2. 设备身份公钥与成员记录匹配；
3. 能证明持有当前空间密钥；
4. 设备未被撤销；
5. 协议版本兼容。

### 13.3 配对校验

六位同步码由已有成员在本地使用安全随机数生成，只能使用一次，默认 60 秒失效。同步码不能作为普通字符串直接发送给对方，也不能直接派生长期密钥。

配对使用经过审计的密码认证密钥交换（PAKE，例如符合标准的 SPAKE2+ 实现），并将双方临时握手 transcript、长期身份公钥和随机 nonce 绑定到验证结果。PAKE 成功且已有成员确认设备信息之前，不得传输空间密钥。

配对接口需要：

- 60 秒有效期；
- 成功一次后立即失效；
- 单 IP 和全局频率限制；
- 同时最多一个待确认请求；
- 失败后指数退避；
- 用户可见的请求设备名称和系统类型。

### 13.4 撤销

撤销成员后：

- 立即关闭该设备现有连接；
- 标记为 `REVOKED`；
- 后续握手拒绝；
- 生成成员变更事件；
- 轮换空间密钥，并安全分发给其余在线成员；
- 离线但仍有效的成员通过已固定的身份连接取得新密钥。

MVP 不支持远程擦除被撤销设备上已经保存的历史和文件。

## 14. QUIC 应用协议

### 14.1 连接参数

```text
ALPN: synchalo/1
Transport: QUIC over UDP
Security: TLS 1.3
Idle timeout: 30 s
Keepalive: 10 s（仅已配对连接）
```

不启用携带副作用的 0-RTT 消息，避免重放导致文件任务或剪贴板事件被重复应用。恢复连接可以使用会话恢复，但应用事件仍必须经过 `event_id` 幂等检查。

### 14.2 连接内流

| 流 | 类型 | 内容 |
| --- | --- | --- |
| Control | 长期双向流 | Hello、成员、剪贴板事件、文件 Offer、Ack、错误和心跳。 |
| File Data | 单向或双向独立流 | 文件块和块校验。每个文件可使用多个受限并行流。 |
| History/Sync | 短期双向流 | 序号摘要、缺口查询和可选历史补齐。 |

控制流使用长度前缀的 Protobuf 帧。每帧设置最大长度，解析前先检查类型与长度，未知字段忽略，未知必需能力则返回 `UNSUPPORTED_VERSION`。

### 14.3 控制消息

MVP 至少包含：

```text
Hello
HelloAck
PairRequest
PairChallenge
PairConfirm
MemberUpdate
ClipboardEvent
FileOffer
FileAccept
ChunkRequest
TransferProgress
TransferComplete
EventAck
Ping / Pong
Error
```

### 14.4 连接状态机

```text
DISCOVERED
  → CONNECTING
  → TLS_ESTABLISHED
  → AUTHENTICATING
  → ONLINE
  → DEGRADED
  → RECONNECTING
  → OFFLINE

未配对分支：
TLS_ESTABLISHED → PAIRING → USER_CONFIRM → ONLINE

拒绝分支：
AUTHENTICATING → REVOKED / INCOMPATIBLE / REJECTED
```

## 15. 剪贴板实现

### 15.1 跨平台抽象

```rust
trait ClipboardBackend {
    fn capabilities(&self) -> ClipboardCapabilities;
    fn start_watch(&self) -> Result<ClipboardEventStream>;
    fn read_text(&self) -> Result<Option<String>>;
    fn write_text(&self, text: &str) -> Result<WriteReceipt>;
    fn read_file_refs(&self) -> Result<Vec<PathBuf>>;
}
```

`read_file_refs` 只在用户点击“粘贴文件”或触发 App 内快捷键时调用，不在后台监听器中自动调用。

### 15.2 平台实现

- **macOS**：轮询 `NSPasteboard.changeCount`，变化后读取 `public.utf8-plain-text`；文件使用 file URL/pasteboard types。
- **Ubuntu X11**：监听 CLIPBOARD selection，读取 UTF-8 与 `text/uri-list`。
- **Ubuntu Wayland**：优先使用 compositor 支持的 data-control；文件粘贴读取 `text/uri-list`，不支持时使用明确的降级模式。

macOS 轮询建议默认 150 ms，窗口隐藏时可以放宽到 250 ms；读取和哈希必须在 Rust 线程中执行，不触发 UI 重渲染。

### 15.3 回声抑制状态

每个平台适配器维护：

```text
last_seen_platform_sequence
last_applied_event_id
last_applied_content_hash
last_applied_at_monotonic
suppression_deadline
```

抑制条件必须综合平台序号、内容哈希和短时间窗口，不能仅依赖文本相等；用户稍后主动复制同样文本时仍应生成新事件。

### 15.4 敏感内容

操作系统通常不能可靠判断复制内容是否来自密码管理器。首次启用必须提示：“复制的所有纯文本都可能被发送到可信设备并进入本地历史。”

提供以下控制：

- 一键暂停；
- 本地历史关闭；
- 自动清理时间；
- 每台设备接收开关；
- 可选的应用排除列表作为后续能力，不在 MVP 承诺所有平台可用。

## 16. 文件传输机制

### 16.1 文件状态机

```text
CREATED
  → QUEUED
  → OFFERED
  → ACCEPTED
  → TRANSFERRING
  → VERIFYING
  → COMMITTING
  → COMPLETED

可恢复：
TRANSFERRING → PAUSED / WAITING_PEER → TRANSFERRING

失败：
任意状态 → FAILED → RETRYING

取消：
QUEUED / TRANSFERRING → CANCELED
```

### 16.2 文件 Offer

```protobuf
message FileOffer {
  bytes event_id = 1;
  string file_name = 2;
  uint64 file_size = 3;
  int64 modified_at_utc_ms = 4;
  uint32 chunk_size = 5;
  uint64 total_chunks = 6;
  string media_type = 7;
  bytes source_fingerprint = 8;
}
```

文件名按 UTF-8 协议传输；接收端负责转换并净化为本平台合法名称。不得接受绝对路径、`..`、设备名、Windows 保留名或指向接收目录外的路径。

### 16.3 分块与校验

- 默认块大小 4 MiB。
- 小于 4 MiB 的文件使用一个块。
- 每块携带 `chunk_index`、偏移、长度和 BLAKE3 哈希。
- 发送端边读取边计算块哈希并发送，不要求先完整扫描文件。
- 发送结束后发送整文件 BLAKE3 和完整 manifest 摘要。
- 接收端先校验块，再在完成阶段校验整文件。
- 分块大小可以通过协议能力协商，但同一任务开始后不可改变。

### 16.4 传输并发与背压

默认限制：

- 每个目标设备最多 4 条活跃文件数据流。
- 全局最多 8 条文件数据流。
- 全局缓冲预算 128 MiB。
- 控制消息使用独立高优先级队列。
- 慢设备使用独立任务和有界队列，不阻塞其他设备。
- 不对文件内容默认压缩；常见媒体和压缩包再次压缩只会消耗 CPU。

实际参数根据性能测试调整，并允许在高级设置中选择“节能、均衡、极速”。

### 16.5 断点续传

接收端持久化已校验块 bitmap。重新连接后：

1. 发送端重新发送 `FileOffer`。
2. 接收端返回已拥有块 bitmap 或缺失区间。
3. 发送端只发送缺失块。
4. 最终重新执行整文件校验。

bitmap 使用压缩位图或连续区间编码，不为每个块频繁创建单独数据库事务。进度按时间或数据量批量落盘，例如每 1 秒或每 64 MiB 一次。

### 16.6 源文件变化

MVP 使用原文件作为待发送内容源。发送开始时记录：

- 文件大小；
- 修改时间；
- 平台文件标识；
- 已计算块哈希。

续传前若源文件被删除或元数据变化，任务进入 `SOURCE_MISSING` 或 `SOURCE_CHANGED`，不继续发送混合内容。用户可重新选择文件创建新事件。

后续 P1 可增加受配额控制的 CAS Outbox，在离线目标存在时缓存文件块，从而允许源文件被移动或删除。

### 16.7 接收与原子提交

- 临时文件放在接收目录内的隐藏临时区域，保证最终重命名位于同一文件系统。
- 临时名称：`.synchalo-<event-id>.part`。
- 校验完成后刷新文件数据和必要元数据，再原子重命名。
- App 崩溃后扫描数据库和临时文件，匹配任务后恢复；无法匹配的孤儿临时文件在宽限期后清理。
- 磁盘空间预检查至少要求 `file_size + safety_margin`；稀疏文件支持不在 MVP 范围内。

## 17. 本地存储方案

### 17.1 文件布局

本地应用数据目录使用操作系统标准 App Data 位置：

```text
<app-data>/SyncHalo/
├── synchalo.db
├── synchalo.key                  # 256 bit KEK，文件权限 0600
├── manifests/
│   └── <event-id>.manifest
├── outbox/                      # P1 或待发送缓存
├── logs/
│   └── synchalo.log
└── diagnostics/
```

KEK 由 Rust 安全随机生成并保存在 `synchalo.key`；SQLite 只保存带 AEAD 认证标签的 wrapped DEK 和加密设备身份，不保存明文 DEK/私钥。若攻击者同时取得数据库与密钥文件，则能够离线解密，因此该方案主要防护数据库单独泄露，不等同于硬件或系统安全存储。接收中的 `.part` 文件位于用户接收目录内的隐藏临时目录。

数据库严禁放在网络共享、用户配置的接收目录或会被其他同步工具实时同步的位置。

### 17.2 SQLite 配置

推荐配置：

```sql
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;
PRAGMA temp_store = MEMORY;
```

存储访问采用单写入 actor，UI 查询通过受控只读连接或同一存储服务完成。长查询不得保持事务跨越 UI await。

由于 2026 年披露的 SQLite WAL reset 问题，必须使用已修复版本：SQLite 3.51.3+，或官方提供修复的 3.50.7/3.44.6 回移版本。构建采用 bundled SQLite，并在 CI 和启动诊断中断言运行时版本，不能依赖系统自带的未知版本。

数据库只存元数据和小型加密文本，不把文件字节写入 SQLite BLOB。

### 17.3 数据表

#### `settings`

```text
key TEXT PRIMARY KEY
value_json TEXT NOT NULL
updated_at_ms INTEGER NOT NULL
```

保存接收目录、历史保留、剪贴板开关、并发数、开机启动等。所有设置变更需经过类型校验。

#### `local_state`

```text
device_id BLOB PRIMARY KEY
next_origin_sequence INTEGER NOT NULL
hlc_physical_ms INTEGER NOT NULL
hlc_logical INTEGER NOT NULL
updated_at_ms INTEGER NOT NULL
```

本机事件序号、HLC 状态和事件本体必须在同一事务中推进，保证进程崩溃或并发事件不会复用来源序号。

#### `crypto_metadata`、`wrapped_data_keys` 与 `local_secrets`

```text
crypto_metadata:
  singleton INTEGER PRIMARY KEY
  database_id TEXT NOT NULL UNIQUE

wrapped_data_keys:
  key_id TEXT PRIMARY KEY
  purpose TEXT NOT NULL
  algorithm TEXT NOT NULL
  wrap_algorithm TEXT NOT NULL
  wrap_nonce BLOB NOT NULL
  wrapped_key BLOB NOT NULL
  created_at_ms INTEGER NOT NULL
  status TEXT NOT NULL              # active / retired

local_secrets:
  secret_id TEXT PRIMARY KEY
  key_id TEXT NOT NULL
  crypto_version INTEGER NOT NULL
  nonce BLOB NOT NULL
  ciphertext BLOB NOT NULL
  updated_at_ms INTEGER NOT NULL
```

每个数据库生成独立 `database_id`。KEK 使用 XChaCha20-Poly1305 包装随机 256 bit DEK，AAD 绑定 `database_id`、`key_id`、用途和算法版本。剪贴板记录保存 `key_id` 与 `crypto_version`；设备传输身份保存在 `local_secrets`，AAD 绑定 `secret_id` 和 `key_id`。

旧版本升级采用两阶段迁移：先最后读取一次 Keychain/Secret Service，将 KEK 写入 `.synchalo.key.pending`，再把传输身份加密写入 SQLite 并回读验证；随后生成包含加密身份的 `synchalo.keychain-migration-backup.db`，全部成功后原子改名为 `synchalo.key` 并删除旧系统钥匙串项。存在最终密钥文件时，启动流程完全绕过 Keychain API。

#### `spaces`

```text
space_id BLOB PRIMARY KEY
display_name TEXT NOT NULL
created_at_ms INTEGER NOT NULL
key_version INTEGER NOT NULL
secret_key_ref TEXT NOT NULL
```

`secret_key_ref` 指向 SQLite 中由本机 DEK 加密的 secret 记录，不是密钥正文。

#### `devices`

```text
device_id BLOB PRIMARY KEY
space_id BLOB NOT NULL
display_name TEXT NOT NULL
platform TEXT NOT NULL
public_key BLOB NOT NULL
fingerprint BLOB NOT NULL
trust_state TEXT NOT NULL
protocol_version INTEGER NOT NULL
capabilities BLOB NOT NULL
trusted_at_ms INTEGER
last_seen_at_ms INTEGER
revoked_at_ms INTEGER
```

索引：`(space_id, trust_state)`、`last_seen_at_ms`。

#### `events`

```text
event_id BLOB PRIMARY KEY
space_id BLOB NOT NULL
origin_device_id BLOB NOT NULL
origin_sequence INTEGER NOT NULL
kind TEXT NOT NULL
created_at_utc_ms INTEGER NOT NULL
hlc_physical_ms INTEGER NOT NULL
hlc_logical INTEGER NOT NULL
content_hash BLOB NOT NULL
schema_version INTEGER NOT NULL
direction TEXT NOT NULL
received_at_utc_ms INTEGER
signature BLOB NOT NULL
UNIQUE(origin_device_id, origin_sequence)
```

索引：

- `(hlc_physical_ms DESC, hlc_logical DESC, origin_device_id DESC)`；
- `(kind, created_at_utc_ms DESC)`；
- `(origin_device_id, origin_sequence)`。

#### `clipboard_items`

```text
event_id BLOB PRIMARY KEY REFERENCES events(event_id) ON DELETE CASCADE
ciphertext BLOB NOT NULL
nonce BLOB NOT NULL
content_length INTEGER NOT NULL
is_pinned INTEGER NOT NULL DEFAULT 0
expires_at_ms INTEGER
hidden_at_ms INTEGER
```

不保存明文预览。历史页面分页读取后，在 Rust 内存中解密并只把当前可见条目传给 UI。搜索默认在受限数量的解密结果中执行，不建立泄露明文的全文索引。

#### `event_deliveries`

```text
event_id BLOB NOT NULL REFERENCES events(event_id) ON DELETE CASCADE
peer_device_id BLOB NOT NULL
delivery_class TEXT NOT NULL     # LIVE / DURABLE / REPLAY
status TEXT NOT NULL             # PENDING / SENT / ACKED / EXPIRED / FAILED
attempt_count INTEGER NOT NULL DEFAULT 0
last_attempt_at_ms INTEGER
acked_at_ms INTEGER
expires_at_ms INTEGER
PRIMARY KEY(event_id, peer_device_id)
```

该表保存控制事件的逐设备投递状态。剪贴板投递记录超过实时窗口后进入 `EXPIRED`；文件 Offer 和成员事件使用 `DURABLE`，持续重试。

#### `file_items`

```text
event_id BLOB PRIMARY KEY REFERENCES events(event_id) ON DELETE CASCADE
file_name TEXT NOT NULL
source_path TEXT
destination_path TEXT
file_size INTEGER NOT NULL
modified_at_utc_ms INTEGER
chunk_size INTEGER NOT NULL
total_chunks INTEGER NOT NULL
manifest_hash BLOB
whole_file_hash BLOB
status TEXT NOT NULL
completed_at_ms INTEGER
```

`source_path` 和 `destination_path` 只在本机数据库出现，不进入跨设备事件签名主体。

#### `transfer_targets`

```text
id INTEGER PRIMARY KEY
event_id BLOB NOT NULL REFERENCES events(event_id) ON DELETE CASCADE
peer_device_id BLOB NOT NULL
direction TEXT NOT NULL
status TEXT NOT NULL
bytes_transferred INTEGER NOT NULL DEFAULT 0
started_at_ms INTEGER
updated_at_ms INTEGER NOT NULL
completed_at_ms INTEGER
retry_count INTEGER NOT NULL DEFAULT 0
last_error_code TEXT
last_error_detail TEXT
UNIQUE(event_id, peer_device_id, direction)
```

索引：`(status, updated_at_ms)`、`(peer_device_id, status)`。

#### `transfer_checkpoints`

```text
event_id BLOB NOT NULL
peer_device_id BLOB NOT NULL
verified_chunks BLOB NOT NULL
manifest_path TEXT
updated_at_ms INTEGER NOT NULL
PRIMARY KEY(event_id, peer_device_id)
```

`verified_chunks` 使用压缩 bitmap。块哈希较多时保存在版本化 manifest sidecar 文件中，数据库只保存路径和摘要。

#### `event_tombstones`

```text
event_id BLOB PRIMARY KEY
reason TEXT NOT NULL
created_at_ms INTEGER NOT NULL
expires_at_ms INTEGER NOT NULL
```

历史过期或本地隐藏后保留短期 tombstone，防止重连期间重复导入已经清理的事件。

### 17.4 事务边界

- 创建事件、具体事件记录和目标任务必须在同一事务中提交。
- 接收事件先验证签名与唯一性，再在事务中插入；唯一冲突视为幂等成功。
- 文件字节写入不占用数据库事务。
- 进度更新批量写入，避免每个数据块一次事务。
- 文件最终重命名成功后再提交 `COMPLETED`；若数据库提交失败，启动恢复流程通过文件哈希修复状态。
- 历史删除采用数据库事务；删除文件是独立的、用户确认后的文件系统操作。

### 17.5 保留与清理

- 剪贴板历史默认 7 天或 500 条；收藏项例外。
- 文件历史默认保留 90 天，只删除记录，不删除文件。
- 已完成任务的 checkpoint 和 manifest 默认 24 小时后清理。
- 失败任务默认保留 7 天。
- 日志默认滚动保存 7 天，总量上限 50 MiB。
- 清理任务在 App 空闲时执行，并使用小批次事务。

## 18. UI 与核心接口

### 18.1 Tauri Commands

```text
get_app_state
list_devices
open_pairing_window
confirm_pairing
revoke_device
paste_files
enqueue_files
pause_transfer
resume_transfer
cancel_transfer
list_clipboard_history
copy_history_item
delete_clipboard_item
clear_clipboard_history
list_file_history
retry_transfer
reveal_file
get_settings
update_settings
select_receive_directory
export_diagnostics
```

前端统一通过类型化 TypeScript API 封装调用，不在页面组件中散落字符串形式的 `invoke`。每个 Tauri command 只负责反序列化、鉴权、参数验证和错误映射，再转换为 Rust 应用命令并通过有界 channel 交给应用服务。

前端传入的路径、设备 ID、事件 ID 和分页游标都视为不可信输入；路径必须由 Rust 后台规范化并重新校验。选择文件或目录时，优先让系统对话框直接向 Rust 返回受控路径，前端只保留用于展示的副本。

### 18.2 Tauri Events 与 Channels

```text
device_discovered
device_state_changed
pairing_requested
clipboard_history_added
clipboard_state_changed
transfer_added
transfer_progress
transfer_state_changed
settings_changed
storage_warning
network_warning
```

离散状态变化使用 Tauri event；需要持续更新的传输进度可以使用 channel，但仍在 Rust 侧合并并限制为每个活跃任务每秒 5–10 次。前端只更新对应任务的 ViewModel，不能因单条进度变化重新渲染整张历史列表。

Command、event 和 channel 的 payload 使用显式 DTO，不直接序列化内部领域对象。错误统一映射为稳定的错误码、用户可见摘要和可选诊断 ID；不得把密钥、堆栈、剪贴板正文或完整敏感路径写入前端日志。

## 19. 性能设计与指标

### 19.1 实现原则

- 文件字节不进入 WebView，也不经过 Tauri IPC；Web 前端只接收进度、状态和展示所需元数据。
- 哈希和网络发送采用流式管线，不先把整个文件读入内存。
- 对每个连接设置流量和内存上限。
- 控制消息与文件数据分流并优先调度。
- SQLite 进度批量写入，历史列表使用游标分页。
- 文件列表和历史列表采用虚拟滚动。
- 多目标传输使用独立读取任务；利用操作系统页缓存，同时隔离慢设备。
- 不默认压缩文件。

### 19.2 验收环境目标

在 SSD、千兆有线局域网、两台性能正常的设备上：

| 指标 | MVP 目标 |
| --- | --- |
| 设备发现 | P95 小于 3 秒 |
| 文本复制到远端可粘贴 | P95 小于 500 ms |
| 1 GB 单文件有效吞吐 | 不低于 700 Mbps，且不低于同环境 iperf 的 70% |
| 大文件内存占用 | 传输 50 GB 文件时 App 总内存低于 256 MiB |
| 控制消息延迟 | 大文件满速传输时剪贴板 P95 仍小于 800 ms |
| 断点续传 | 10 GB 文件中断后只重传缺失块 |
| UI 进度 | 不冻结，进度更新延迟小于 1 秒 |

无线网络指标以链路实际吞吐为基线，不承诺固定 Mbps。

## 20. 安全与隐私

### 20.1 威胁模型

需要防御：

- 同一局域网中的未授权设备窃听或主动连接。
- 中间人替换配对对象。
- 重放旧剪贴板或文件控制消息。
- 恶意文件名进行路径穿越或覆盖系统文件。
- 超大消息、连接洪泛和磁盘耗尽。
- 日志、崩溃报告泄露剪贴板正文或完整文件路径。

MVP 不防御：

- 已配对设备本身被完全控制。
- 用户主动接收并打开的恶意文件。
- 操作系统或管理员级恶意程序读取剪贴板。

### 20.2 安全要求

- 所有正常数据连接使用 TLS 1.3。
- 配对使用可核对的短认证字符串，确认前不发送空间秘密。
- 事件由来源设备签名，转发不改变事件本体。
- `event_id` 幂等，连接级 nonce 和会话状态防止简单重放。
- 单个控制帧、剪贴板正文、文件名和并发数都有硬限制。
- 文件只写入接收目录；路径规范化后再次检查父目录。
- 临时文件使用仅当前用户可访问的权限。
- 剪贴板正文使用本机密钥进行字段级加密。
- `synchalo.key` 以 `0600` 权限保存 KEK；SQLite 只保存 wrapped DEK 与加密身份。正文、关键记录元数据、`secret_id` 和 `key_id` 由 AEAD 一并认证。
- Keychain/Secret Service 仅允许在旧版本一次性迁移路径中调用；最终密钥文件存在时不得查询、创建或更新系统钥匙串项。
- 默认不启用遥测；诊断包必须脱敏并由用户主动导出。
- mDNS 只暴露最少发现信息。
- 撤销设备后轮换空间密钥。
- Tauri capabilities 按窗口和插件采用最小授权；自定义 command 纳入应用权限清单，未使用的 shell、文件系统、HTTP 和进程能力默认不开放。
- WebView CSP 只允许随应用打包的本地脚本、样式、字体和必要的 Tauri IPC，不加载远程脚本或 CDN 资源。
- 所有 Tauri command 参数均按不可信输入处理；涉及文件、设备和事件的操作必须在 Rust 侧重新授权与校验。

## 21. 错误处理与用户提示

标准错误码至少包括：

```text
NETWORK_UNREACHABLE
MDNS_UNAVAILABLE
FIREWALL_BLOCKED
PAIRING_TIMEOUT
PAIRING_REJECTED
AUTH_FAILED
DEVICE_REVOKED
PROTOCOL_INCOMPATIBLE
CLIPBOARD_PERMISSION_DENIED
CLIPBOARD_PLATFORM_LIMITED
TEXT_TOO_LARGE
SOURCE_MISSING
SOURCE_CHANGED
DESTINATION_UNWRITABLE
DISK_FULL
NAME_CONFLICT
HASH_MISMATCH
TRANSFER_TIMEOUT
TRANSFER_CANCELED
DATABASE_BUSY
DATABASE_CORRUPT
KEYSTORE_UNAVAILABLE
```

每个错误需要包含：用户可理解的标题、简短说明、推荐动作和可复制的诊断码。界面不直接显示 Rust 错误堆栈。

## 22. 日志与诊断

### 22.1 日志字段

允许记录：

- 时间、日志级别和模块；
- 截断后的设备 ID、事件 ID、连接 ID；
- 任务状态、字节数、耗时和错误码；
- 网络接口类型和脱敏地址族；
- 协议版本与能力协商结果。

禁止记录：

- 剪贴板正文；
- 历史解密内容；
- 私钥、空间密钥和配对材料；
- 文件内容；
- 默认情况下的完整本地路径和真实用户名。

### 22.2 诊断页

显示：

- App、协议和 SQLite 运行时版本；
- 当前平台剪贴板能力；
- mDNS 发布/浏览状态；
- QUIC 监听端口；
- 活跃网络接口；
- 数据库健康状态；
- 接收目录权限和剩余空间；
- 最近错误码。

## 23. 测试方案

### 23.1 单元测试

- HLC 生成、合并和稳定排序。
- UUID/来源序号幂等。
- 剪贴板回声抑制状态机。
- 文件名净化和路径穿越拦截。
- 同名文件命名策略。
- 分块 bitmap 编解码。
- Protobuf 版本兼容。
- 历史保留和清理策略。
- 加密字段解密失败处理。

### 23.2 集成测试

- 同机启动 2–3 个隔离实例完成发现、认证和事件传播。
- A → B → A 转发不产生新事件。
- A/B 同时复制后所有节点得到相同最终排序。
- 连接中断、进程重启、IP 变化后恢复文件。
- 重复 Offer、Ack 和 Complete 保持幂等。
- 接收目录只读、磁盘满、文件占用和杀毒软件锁定。
- 数据库升级和中途崩溃恢复。

### 23.3 平台矩阵

- macOS Intel ↔ macOS Apple Silicon。
- macOS ↔ Ubuntu 24.04 ARM64 X11。
- macOS ↔ Ubuntu 24.04 ARM64 Wayland。
- Ubuntu ARM64 ↔ Ubuntu ARM64。
- Ubuntu Wayland：GNOME、KDE、wlroots 能力检测和降级。
- IPv4-only、IPv6-only、双栈。
- Wi-Fi、以太网、两块网卡、VPN 同时存在。
- 睡眠/唤醒、锁屏、用户切换。
- Ubuntu ufw 开启、关闭以及局域网端口被拒绝。

### 23.4 文件用例

- 0 B、1 B、4 MiB 边界、4 MiB + 1 B。
- 1 GB、10 GB、50 GB 文件。
- 中文、emoji、组合字符、长文件名。
- 10,000 个小文件作为后续文件夹能力基准。
- 同名同内容、同名不同内容。
- 传输中修改、删除或移动源文件。
- 校验失败、随机丢包、网络抖动和带宽限制。

### 23.5 安全测试

- 未配对连接和伪造设备 ID。
- 配对码暴力尝试和重放。
- 篡改事件签名、块哈希和整文件哈希。
- 超长 Protobuf 帧、非法枚举和未知字段。
- `../`、绝对路径、Windows 保留名和 Unicode 混淆路径。
- 撤销后重连和旧空间密钥使用。

## 24. 发布与运维

### 24.1 构建产物

- macOS：通用或分别构建 arm64/x86_64，DMG，签名并 notarize。
- Ubuntu ARM64：deb 与 AppImage；rpm 和其他架构作为后续补充。

### 24.2 CI

使用原生平台矩阵构建：

- macOS runner；
- Ubuntu 24.04 ARM64 runner；
- Rust 单元测试、Clippy、格式检查；
- TypeScript 类型检查、ESLint、前端单元与组件测试；
- macOS WKWebView 与 Ubuntu WebKitGTK 端到端交互测试和关键页面截图对比；
- 协议兼容测试；
- SQLite 运行时版本断言；
- 安装包签名和发布产物校验。

### 24.3 更新

应用数据同步不依赖互联网；软件更新可选使用签名更新清单。自动更新必须可关闭，更新包必须验证签名，失败不能影响本地同步服务。

## 25. 里程碑

| 里程碑 | 范围 | 预计时间 |
| --- | --- | --- |
| M0：协议与骨架 | Workspace、Tauri 2 + React/TypeScript/Vite、SQLite 迁移、事件模型、HLC | 1 周 |
| M1：发现与配对 | mDNS、QUIC、身份、配对、设备页 | 1–1.5 周 |
| M2：剪贴板 | macOS/Ubuntu ARM64 适配、回声抑制、历史、暂停 | 1–1.5 周 |
| M3：文件传输 | 拖放/粘贴、分块、校验、进度、冲突 | 1.5–2 周 |
| M4：恢复与历史 | 续传、重启恢复、文件历史、清理 | 1 周 |
| M5：硬化发布 | 平台 QA、性能、安全、安装包与签名 | 1–2 周 |

一名熟练工程师完成可发布 MVP 的现实估计为 6–8 周；首个两设备文本同步纵向原型目标为第 2 周末。

## 26. MVP 验收标准

满足以下条件才可判定 MVP 完成：

1. macOS 与 Ubuntu 24.04 ARM64 任意两台设备可在局域网内发现并完成配对。
2. 未配对设备无法读取或注入剪贴板与文件事件。
3. A 复制文本后 B 可在 500 ms 目标延迟内粘贴；B 不会把远端写入再次发送回 A。
4. A/B 同时复制时，两个历史事件均保留，所有在线设备最终选择同一事件。
5. App 可显示、搜索、收藏和清理加密的剪贴板历史。
6. 用户可粘贴或拖入多个文件并发送给多个目标设备。
7. 文件哈希错误不会生成最终文件；断线重连只补发缺失块。
8. 同名同内容不重复写入，同名不同内容不覆盖原文件。
9. App 重启后未完成文件任务能恢复或显示明确不可恢复原因。
10. 接收目录默认 Downloads，可修改，失效时有明确提示。
11. 文件历史可显示每台目标设备状态，并可打开、定位和重试。
12. 50 GB 文件传输期间内存保持有界，UI 和剪贴板控制消息不被阻塞。
13. Linux Wayland 不支持完整后台剪贴板时能正确检测并降级提示。
14. 日志和诊断包不包含剪贴板正文、密钥和文件内容。
15. SQLite 使用已修复 WAL reset 问题的版本，并通过 CI/启动诊断验证。
16. 主窗口在最小尺寸下仍保持左侧菜单、右侧内容的两栏结构，左侧只出现“粘贴板”“同步文件”“设置”。
17. 粘贴板历史行点击不复制；顶部无重复暂停按钮；已收藏行在来源设备名前显示“实心星标 ·”，未收藏不显示状态，悬停或键盘聚焦时显示预览、复制、收藏、删除按钮；复制与最新一条相同的内容不重复同步，删除后可在 5 秒内撤销。
18. 粘贴板历史支持顶部收藏筛选，并以每页 100 条进行真实后端分页；分页上方无装饰线，点击上一页、下一页或数字页码后滚动条回到开头。
19. 同步文件页采用左设备、右发送、下历史三分区；本机固定第一项；同步码弹层在右侧内容区居中并使用 70% 黑色蒙层；文件入口无远端目标时使用统一提示。
20. 文件拖入区同时承担文件选择入口；页面不显示粘贴按钮，按 `Ctrl/Cmd + V` 自动同步文件粘贴板内容。
21. 文件历史每页 100 条并由 Rust/SQLite 真实分页；不显示状态标签筛选，搜索框位于右上角收藏左侧；历史每行提供再次同步和收藏。
22. 设置页首屏同时展示一次性同步码和按在线、离线分组的设备列表；“加入”使用统一模态框，页面无行内加入 UI 和通知开关，通知始终开启。
23. 系统切换浅色或深色模式后，Web 前端无需重启即可更新主题，且所有状态仍可辨识。
24. 生产 WebView 只加载本地静态资源；文件数据面和密钥不经过 Tauri IPC，capabilities 与 CSP 通过安全检查；最终本地密钥存在时启动不访问 Keychain/Secret Service。
25. macOS 点击状态栏图标可直接显示并聚焦主窗口；Ubuntu AppIndicator 菜单首项可完成相同行为。

## 27. 风险与应对

| 风险 | 影响 | 应对 |
| --- | --- | --- |
| Wayland 后台剪贴板协议不统一 | 部分 Linux 环境无法自动同步 | 运行时能力检测、明确降级、优先保证 X11；维护 compositor 兼容矩阵。 |
| 企业 Wi-Fi/AP Isolation | 设备互相不可达 | 手动 IP、网络诊断和明确提示；MVP 不引入云端中继。 |
| 多设备系统时钟偏差 | 历史乱序和剪贴板冲突 | HLC + 设备 ID 稳定排序，普通时间只展示。 |
| 剪贴板回声造成循环 | 事件风暴和反复覆盖 | 事件 ID 幂等、来源序号、平台序号和内容哈希三层抑制。 |
| 大文件占满内存或阻塞 UI | 性能下降、崩溃 | Rust 流式 I/O、有界队列、固定内存预算、UI 只接收节流后的状态。 |
| 源文件在失败后、手动重试前被移动 | 任务无法继续 | 保存指纹并明确失败；P1 引入有配额的 CAS Outbox。 |
| 剪贴板历史包含密码 | 隐私泄露 | 明示风险、字段加密、短期保留、一键暂停和可关闭历史。 |
| SQLite WAL 版本缺陷 | 极低概率数据库损坏 | bundled 固定 SQLite 3.51.3+ 或官方修复回移版本、单写 actor、备份与健康检查。 |
| 数据库与 `synchalo.key` 同时泄露 | 本地加密内容可被离线解密 | 密钥文件强制 `0600`、拒绝符号链接、避免放入共享目录；文档明确其保护边界，后续可选硬件密钥模式。 |
| 多网卡/VPN 产生重复连接 | 状态抖动或重复传输 | 按 device_id 去重、确定性拨号、连接 nonce 选主。 |

## 28. 已确定的产品决策

1. 项目内部 crate 目录使用 `core`、`network`、`transfer` 等名称，不使用 `synchalo-` 前缀。
2. 首版是“实时剪贴板 + 显式文件投递”，不是目录镜像产品。
3. 所有同步事件包含 UTC 时间、HLC、来源设备、来源序号和全局事件 ID。
4. 防回环依靠事件幂等和剪贴板回声抑制，不依靠时间戳。
5. App 展示剪贴板历史和文件同步历史。
6. 剪贴板历史正文默认本地加密保存 7 天或 500 条。
7. 离线旧剪贴板事件不自动覆盖当前剪贴板。
8. 文件同步未指定目标时发送给全部在线设备；指定目标离线或传输掉线时立即失败，可由用户手动重试并断点续传。
9. 接收目录默认 Downloads，且不自动监控目录变化。
10. 桌面端采用 Tauri 2 + React/TypeScript/Vite + Rust 核心；文件数据面不进入 WebView 或 Tauri IPC。

## 29. 待后续评审的决策

以下项目不阻塞 M0/M1，但需要在进入 Beta 前确定：

- 新设备加入空间时，是否允许用户选择同步配对前的剪贴板历史。
- 本地历史默认上限采用“500 条”还是“1,000 条”。
- 文件夹拖放是否进入 MVP，还是严格放在 P1。
- 是否默认启用受配额控制的离线 Outbox 缓存。
- Windows 后续版本采用 MSI、NSIS 或 MSIX 中的哪一种安装格式。
- Ubuntu 22.04 ARM64 是否进入后续兼容范围。

## 30. 技术参考

- [Tauri 架构](https://v2.tauri.app/concept/architecture/)
- [Tauri 前端配置](https://v2.tauri.app/start/frontend/)
- [Tauri WebView 版本与平台差异](https://v2.tauri.app/reference/webview-versions/)
- [Tauri Commands、Events 与 Channels](https://v2.tauri.app/develop/calling-rust/)
- [Tauri Capabilities](https://v2.tauri.app/security/capabilities/)
- [Tauri Content Security Policy](https://v2.tauri.app/security/csp/)
- [Tauri 系统托盘](https://v2.tauri.app/learn/system-tray/)
- [QUIC：RFC 9000](https://datatracker.ietf.org/doc/rfc9000/)
- [mdns-sd Rust 文档](https://docs.rs/crate/mdns-sd/latest)
- [SQLite WAL 官方文档](https://sqlite.org/wal.html)
- [Apple NSPasteboard changeCount](https://developer.apple.com/documentation/appkit/nspasteboard/changecount)
- [Wayland ext-data-control-v1](https://wayland.app/protocols/ext-data-control-v1)
- [BLAKE3 Rust 实现](https://docs.rs/crate/blake3/latest)
