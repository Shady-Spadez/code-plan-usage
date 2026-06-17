# Coding Plan Widget — 需求文档

> 本文档记录所有已实现和计划中的功能需求。每次新增或修改功能时，需先阅读本文档，确保不破坏已有功能。

---

## 1. Widget 悬浮显示

**Date**: 2026-06-14
**Status**: ✅ Implemented

### Description
一个透明、无边框、置顶的悬浮 widget，显示 Coding Plan 的用量百分比（圆形进度条）。Widget 尺寸固定为 60×60，百分比数字居中显示在圆形内。

### Acceptance Criteria
- [x] 窗口透明、无边框、始终置顶
- [x] 显示圆形进度条，表示当月用量百分比
- [x] 固定 60×60 尺寸（曾规划小/中/大三档，已收敛为单一固定尺寸）
- [x] 支持暗色/亮色主题
- [x] 百分比数字居中显示在圆形内（总是显示，不可关闭）
- [x] 进度条颜色随百分比变化（绿→黄→橙→红）
- [x] 进度条有平滑动画过渡（指数衰减，帧率无关）

### Implementation Notes
- `src/widget.rs`: WidgetApp, update(), widget rendering
- `src/theme.rs`: `widget_config()` 返回固定 SizeConfig（60×60, radius=25, stroke=3.5）
- 窗口创建参数: `with_transparent(true)`, `with_decorations(false)`, `with_always_on_top()`
- 动画: `speed = 1 - exp(-8.0 * dt)`，snap 阈值 0.05

---

## 2. 系统托盘

**Date**: 2026-06-14
**Status**: ✅ Implemented

### Description
系统托盘图标，支持右键菜单操作。

### Acceptance Criteria
- [x] 托盘图标显示绿色圆点
- [x] 左键点击切换 widget 显示/隐藏
- [x] 右键菜单：显示/隐藏、刷新、设置、退出
- [x] 退出菜单项直接清理并退出进程

### Implementation Notes
- `src/tray.rs`: 完整的托盘实现
- 使用 `tray-icon` crate

---

## 3. 设置面板

**Date**: 2026-06-14
**Status**: ✅ Implemented

### Description
独立的设置面板，通过 `show_viewport_immediate` 作为独立 popup window 弹出（与 widget 窗口分离），widget 窗口在后台继续渲染但被设置窗口遮盖。

### Acceptance Criteria
- [x] 从托盘菜单"设置"打开
- [x] 设置作为独立 popup window 显示（`show_viewport_immediate`），与 widget 窗口分离
- [x] 设置窗口有标题栏（"⚙ 设置"，可拖动）
- [x] 设置窗口大小为 640×720，不可缩放
- [x] 关闭设置后自动恢复 widget 交互（重新触发刷新）
- [x] 点击 OS 关闭按钮返回 widget 模式而非退出应用
- [x] 深色主题 UI
- [x] 配置项：Cookie、CSRF Token、区域代码、刷新间隔、通知阈值、主题、开机自启（曾规划的"窗口大小"和"显示百分比"已移除）
- [x] 保存按钮（手动保存，无自动保存）
- [x] X 按钮关闭设置（有未保存更改时弹出确认对话框：保存并退出 / 不保存）
- [x] 设置面板分为两个签页：通用（区域/刷新/通知/外观/其他）和 Cookie（Cookie/CSRF Token）
- [x] 外观设置（主题）实时同步到 widget
- [x] 点击 widget 圆形区域打开设置面板

### Implementation Notes
- `src/widget.rs`: render_settings_viewport(), update() 中的 `show_viewport_immediate` 调用
- 设置通过 `SettingsViewportState`（Rc<RefCell<>>）在主窗口与 popup 间共享
- 关闭时若 `saved` 则克隆回 `self.settings` 并保存，否则回退到 `original_settings`
- OS 关闭按钮拦截: `ctx.input(|i| i.viewport().close_requested())`，有未保存更改时弹确认对话框

---

## 4. 用量数据获取

**Date**: 2026-06-14
**Status**: ✅ Implemented

### Description
从火山引擎 API 获取 Coding Plan 用量数据。

### Acceptance Criteria
- [x] 调用 `GetCodingPlanUsage` API
- [x] 支持配置区域代码
- [x] 后台线程异步刷新，不阻塞 UI
- [x] 定时刷新（可配置间隔，默认 5 分钟）
- [x] 悬停时触发刷新（冷却时间 30 秒）
- [x] 手动刷新（托盘菜单）
- [x] 错误处理和显示

### Implementation Notes
- `src/widget.rs`: start_refresh(), check_refresh_result(), apply_refresh_result()
- `src/api.rs`: fetch_usage(), console_url()
- API URL: `https://console.volcengine.com/api/top/ark/{region}/2024-01-01/GetCodingPlanUsage`

---

## 5. 悬停提示框

**Date**: 2026-06-14
**Status**: ✅ Implemented

### Description
鼠标悬停在 widget 圆形区域时，展开显示各层级的用量详情。提示框根据屏幕可用区域自适应选择展示方位，避免被屏幕边缘/任务栏裁剪。

### Acceptance Criteria
- [x] 显示各层级的用量百分比和重置倒计时
- [x] 窗口自动扩展以容纳提示框
- [x] 拖拽时隐藏提示框
- [x] 颜色与百分比对应
- [x] 提示框方位按优先级自适应：右下 → 左下 → 左上 → 右上（取第一个能完整放下的角）
- [x] 屏幕工作区（排除任务栏）通过 Win32 `MonitorFromPoint` + `GetMonitorInfoW` 获取，多显示器取窗口所在显示器
- [x] 无论提示框在上方还是下方出现，widget 本体在屏幕上的位置保持不变（不跳动）
- [x] 提示框出现/消失时有至多 1 帧的延迟（视口命令延迟），不影响交互
- [x] 提示框宽度按内容自适应；当 widget 本体已超出屏幕边缘、四个角都放不下时，提示框水平位置钳制到工作区内，确保始终完整可见（不再往右出屏）
- [x] 窗口宽度按需加宽以容纳比 widget 更宽的提示框，widget 屏幕位置不变

### Implementation Notes
- `src/widget.rs`: 提示框尺寸/方位计算、窗口几何与绘制（update() 中）
- `src/screen.rs`: `work_area_for_point()` 获取指定屏幕点所在显示器的工作区
- 方位判断：依据 widget 屏幕位置 + 工作区，按 右下/左下/左上/右上 优先级取首个可放下者
- 兜底：四个角都放不下时按左右余量选边，并把提示框 x 钳制到 `[area.min.x, area.max.x - tooltip_w]`
- 窗口覆盖 widget + 提示框的并集区域；widget 与提示框的本地绘制坐标用 `home - cur` 做 1 帧滞后补偿
- 上方放置时窗口上移（`OuterPosition`）并把 widget 绘制在窗口底部，保持 widget 屏幕位置稳定
- 悬停检测用屏幕坐标，且仅在真实 `PointerMoved` 事件帧更新鼠标屏幕位置（避免窗口被自身命令移动时本地指针过期导致的悬停抖动/死循环）

---

## 6. Widget 拖拽

**Date**: 2026-06-14
**Status**: ✅ Implemented

### Description
通过拖拽圆形区域移动 widget 位置。

### Acceptance Criteria
- [x] 仅在圆形区域可发起拖拽
- [x] 拖拽时直接跟踪鼠标（基于 widget 屏幕位置 home，1 帧延迟由绘制偏移补偿）
- [x] 拖拽结束后保存窗口位置
- [x] 拖拽时隐藏提示框
- [x] 从任意提示框状态（含上方放置）发起拖拽不会卡住、widget 不跳动
- [x] 纯点击（无位移）不触发多余的磁盘保存，且不保存错误位置
- [x] widget 本体不能拖出屏幕，也不能拖到任务栏上（水平方向可贴屏幕左右边，垂直方向钳制到工作区避免压任务栏）

### Implementation Notes
- `src/widget.rs`: 拖拽基于 `drag_anchor`（鼠标相对 widget 屏幕 home 的偏移），每帧 `home = mouse - anchor`，与 OS 窗口瞬时位置解耦
- 拖拽开始即把窗口恢复为 `widget_size`（避免放大的提示框窗口在屏幕边缘被 OS 夹住导致卡住）
- 每帧拖拽位置经 `clamp_home_to_work_area` 钳制：水平用显示器全屏矩形（可贴左右屏幕边），垂直用工作区（排除任务栏），多显示器取 `home` 所在屏
- `src/screen.rs`: `screen_info_for_point()` 同时返回 `monitor`（全屏）与 `work_area`（排除任务栏）矩形
- 释放时仅当位置实际变化才 `settings.save()`

---

## 7. 点击打开控制台

**Date**: 2026-06-14
**Status**: ❌ Removed (合并至功能 3)

### Description
~~点击 widget 圆形区域打开火山引擎控制台。~~

> **收敛说明 (2026-06-17)**: 此功能已被移除。点击 widget 圆形区域的实际行为是打开设置面板（见功能 3），由 commit `7fc0588` ("fix: 点击 widget 打开设置失败") 确立。控制台 URL 的打开方式改为在设置面板 Cookie 签页中通过"🌐 打开控制台"按钮触发 WebView2 登录流程（见功能 10）。

### Acceptance Criteria
- [x] ~~点击圆形区域打开浏览器~~ → 已改为点击圆形区域打开设置面板（功能 3）
- [x] ~~URL 根据区域代码动态生成~~ → `console_url(region)` 仍存在于 `src/api.rs`，仅用作 API 请求的 Referer 头；WebView2 登录使用硬编码 URL（见功能 10 已知限制）

### Implementation Notes
- 点击行为: `src/widget.rs` `if button_clicked && hovered { self.show_settings = true; }`
- `console_url(region)`: `src/api.rs`，用于 API Referer 头

---

## 8. 用量通知

**Date**: 2026-06-14
**Status**: ✅ Implemented

### Description
当用量超过阈值时发送通知。

### Acceptance Criteria
- [x] 可配置通知阈值（0-100%）
- [x] 设为 0 禁用通知
- [x] 每个阈值周期只通知一次

### Implementation Notes
- `src/main.rs`: show_usage_notification() (PowerShell NotifyIcon)
- `src/widget.rs`: notification_sent 字段, apply_refresh_result() 中的阈值检查

---

## 9. 开机自启

**Date**: 2026-06-14
**Status**: ✅ Implemented

### Description
可配置是否开机自动启动。

### Acceptance Criteria
- [x] 设置中可开关
- [x] 通过 Windows 注册表实现

### Implementation Notes
- `src/main.rs`: apply_auto_start()

---

## 10. 浏览器 Cookie 自动提取

**Date**: 2026-06-14
**Status**: ✅ Implemented

### Description
凭证获取支持两条路径：(1) 启动时自动从 cookie 文件读取；(2) 在设置面板手动触发 WebView2 登录流程获取凭证。原 `browser_cookies.rs` 模块（DPAPI 浏览器 cookie 提取）已被 WebView2 登录流程替代（commit `af7a46f`）。

### Acceptance Criteria
- [x] 启动时自动从 `console.volcengine.com_cookies.txt` cookie 文件读取凭证
- [x] 设置面板"🌐 打开控制台"按钮触发 WebView2 登录窗口，登录后自动提取 Cookie/CSRF Token
- [x] 提取后自动保存到设置文件
- [x] 已有凭证时启动自动提取不覆盖（手动 WebView2 流程会覆盖，因为是用户主动操作）
- [x] 设置面板提供"🗑 清理 Cookie"按钮，清空凭证并清理 WebView2 cookies

### Implementation Notes
- `src/settings.rs`: `try_load_from_cookie_file()` 解析 Netscape 格式 cookie 文件，`Settings::load()` 在未配置时调用
- `src/webview_login.rs`: `try_extract_credentials()` 启动 WebView2 登录窗口，通过 channel 异步返回 `BrowserCredentials`
- `src/widget.rs`: 设置面板中轮询 webview_receiver，收到凭证后更新 settings 并保存

### Known Limitations
- WebView2 登录 URL (`TARGET_URL`) 硬编码为 `cn-beijing` 区域，未根据 `settings.region` 动态生成

---

## 11. 设置与 Widget 完全分离

**Date**: 2026-06-14
**Status**: ✅ Implemented

### Description
设置面板作为独立 popup window 显示（`show_viewport_immediate`），与 widget 窗口分离。widget 窗口在后台继续渲染（透明背景，无黑色背景问题），但设置窗口会遮盖 widget。关闭设置后恢复 widget 交互。

### Acceptance Criteria
- [x] 打开设置时无黑色背景（`clear_color` 返回 `[0,0,0,0]`，每帧 GL clear）
- [x] 设置窗口有标题栏可拖动（独立 OS 窗口，带 "⚙ 设置" 标题）
- [x] 设置作为独立 popup window 显示，与 widget 窗口分离
- [x] 关闭设置自动恢复 widget 交互（重新触发刷新）

### Implementation Notes
- `src/widget.rs`: `ctx.show_viewport_immediate(viewport_id, builder, ...)` 创建独立设置窗口
- `clear_color()` 覆盖返回透明，避免子 viewport 的 visuals 污染 widget 窗口
- `SettingsViewportState` 在主窗口与 popup 间共享状态（Rc<RefCell<>>）
- 关闭时 `self.show_settings = false` 并 `self.start_refresh()`

---

## 12. 未配置状态特殊样式

**Date**: 2026-06-15
**Status**: ❌ Removed (简化为错误提示)

### Description
~~当没有本地配置时，widget 使用特殊的视觉样式来区分未配置状态。~~

> **收敛说明 (2026-06-17)**: 此功能已被简化。未配置状态下 widget 仍然显示，但不再使用特殊视觉样式（虚线圆环、"? "图标、"点击配置"提示），改为在圆形右侧显示红色"未配置凭证"错误文字。`ThemeColors` 中的 `unconfigured_*` 字段已移除。

### Acceptance Criteria
- [x] 未配置时 widget 仍然显示（不隐藏）
- [x] 未配置时圆形右侧显示红色"未配置凭证"错误提示（`start_refresh()` 设置 `self.error`）
- [x] 点击圆形区域仍然打开设置面板
- [x] 支持拖拽移动
- [x] ~~圆形区域使用暗淡的背景色~~ → 未实现（使用常规 `circle_bg`）
- [x] ~~显示虚线圆环代替实心进度条~~ → 未实现
- [x] ~~圆形中心显示 "?" 图标~~ → 未实现
- [x] ~~圆形右侧显示 "点击配置" 提示文字（中性色）~~ → 改为红色错误提示
- [x] ~~暗色/亮色主题下均有对应的未配置配色~~ → 未实现

### Implementation Notes
- `src/widget.rs`: `start_refresh()` 在 `!is_configured()` 时设置 `self.error = Some("未配置凭证")`
- `src/widget.rs`: update() 渲染分支：有 error 时在圆形右侧显示红色文字，否则居中显示百分比
- `src/theme.rs`: `ThemeColors` 仅含 `bg_fill`, `circle_bg`, `widget_fg`（无 `unconfigured_*` 字段）
