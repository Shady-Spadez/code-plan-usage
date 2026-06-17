# Coding Plan Widget — Features

> 本文档记录所有已验证、已实现的功能。由 acceptance-tester 技能自动维护。
> 最后收敛日期: 2026-06-17

---

## 1. Widget 悬浮显示

**Verified**: 2026-06-17

### Summary
透明、无边框、置顶的悬浮 widget，显示 Coding Plan 当月用量百分比（圆形进度条）。固定 60×60 尺寸，百分比数字居中显示在圆形内，进度条颜色随百分比变化（绿→黄→橙→红），使用帧率无关的指数衰减平滑动画。

### Key Files
- `src/widget.rs` — WidgetApp, update(), widget rendering
- `src/theme.rs` — widget_config() 固定尺寸, percent_color()
- `src/main.rs` — 窗口创建（with_transparent/with_decorations/with_always_on_top）

---

## 2. 系统托盘

**Verified**: 2026-06-17

### Summary
系统托盘图标，支持左键切换显示/隐藏、右键菜单（显示/隐藏、刷新、设置、退出）。

### Key Files
- `src/tray.rs` — 完整的托盘实现，SETTINGS_REQUESTED 原子标志

---

## 3. 设置面板

**Verified**: 2026-06-17

### Summary
通过 `show_viewport_immediate` 弹出的独立 popup window（640×720，带标题栏，不可缩放），分为"通用"和"Cookie"两个签页。通用签页包含刷新间隔、通知阈值、主题、开机自启；Cookie 签页包含区域代码、Cookie、CSRF Token，以及"打开控制台"和"清理 Cookie"按钮。支持手动保存、未保存更改确认对话框，主题实时同步到 widget。点击 widget 圆形区域或托盘菜单"设置"均可打开。

### Key Files
- `src/widget.rs` — render_settings_viewport(), show_viewport_immediate 调用, SettingsViewportState
- `src/settings.rs` — Settings 数据结构与持久化

---

## 4. 用量数据获取

**Verified**: 2026-06-17

### Summary
从火山引擎 `GetCodingPlanUsage` API 获取用量数据，支持配置区域代码。后台线程异步刷新不阻塞 UI，支持定时刷新（默认 5 分钟）、悬停刷新（冷却 30 秒）、托盘手动刷新。

### Key Files
- `src/api.rs` — fetch_usage(), console_url(), format_level_line()
- `src/widget.rs` — start_refresh(), check_refresh_result(), apply_refresh_result()

---

## 5. 悬停提示框

**Verified**: 2026-06-17

### Summary
鼠标悬停在 widget 圆形区域时，窗口自动扩展显示各层级的用量百分比和重置倒计时。提示框方位按优先级自适应（右下→左下→左上→右上），通过 Win32 `MonitorFromPoint`+`GetMonitorInfoW` 获取屏幕工作区；四个角都放不下时水平钳制到工作区内。拖拽时隐藏提示框。widget 本体屏幕位置始终稳定不跳动。

### Key Files
- `src/widget.rs` — 提示框尺寸/方位计算、窗口几何与绘制（update() 中）
- `src/screen.rs` — screen_info_for_point(), work_area_for_point()

---

## 6. Widget 拖拽

**Verified**: 2026-06-17

### Summary
通过拖拽圆形区域移动 widget 位置。基于 `drag_anchor`（鼠标相对 widget 屏幕 home 的偏移）直接跟踪鼠标，与 OS 窗口命令延迟解耦。拖拽位置经 `clamp_home_to_work_area` 钳制（水平用显示器全屏矩形可贴边，垂直用工作区避开任务栏）。纯点击（位移 <4px）不触发磁盘保存。

### Key Files
- `src/widget.rs` — drag_anchor, press_pos, DRAG_THRESHOLD, clamp_home_to_work_area()
- `src/screen.rs` — screen_info_for_point() 返回 monitor + work_area

---

## 7. 用量通知

**Verified**: 2026-06-17

### Summary
当用量超过可配置阈值时发送 Windows 通知（PowerShell NotifyIcon），每个阈值周期只通知一次，设为 0 禁用通知。

### Key Files
- `src/main.rs` — show_usage_notification()
- `src/widget.rs` — notification_sent 字段, apply_refresh_result() 阈值检查

---

## 8. 开机自启

**Verified**: 2026-06-17

### Summary
可配置是否开机自动启动，通过 Windows 注册表 `HKCU\...\Run` 实现。

### Key Files
- `src/main.rs` — apply_auto_start()

---

## 9. 凭证获取

**Verified**: 2026-06-17

### Summary
凭证获取支持两条路径：(1) 启动时自动从 `console.volcengine.com_cookies.txt` cookie 文件读取（Netscape 格式）；(2) 设置面板"🌐 打开控制台"按钮触发 WebView2 登录窗口，登录后自动提取 Cookie/CSRF Token。已有凭证时启动自动提取不覆盖。设置面板提供"🗑 清理 Cookie"按钮。

### Key Files
- `src/settings.rs` — try_load_from_cookie_file(), Settings::load()
- `src/webview_login.rs` — try_extract_credentials() WebView2 登录流程
- `src/widget.rs` — 设置面板中轮询 webview_receiver

### Known Limitations
- WebView2 登录 URL 硬编码为 `cn-beijing` 区域，未根据 `settings.region` 动态生成

---

## 10. 设置与 Widget 分离

**Verified**: 2026-06-17

### Summary
设置面板作为独立 popup window 显示（`show_viewport_immediate`），与 widget 窗口分离。widget 窗口透明背景（`clear_color` 返回 `[0,0,0,0]`，无黑色背景），设置窗口遮盖 widget。关闭设置后恢复 widget 交互并重新刷新。

### Key Files
- `src/widget.rs` — show_viewport_immediate, clear_color() 覆盖, SettingsViewportState

---

## 已移除功能

以下功能曾计划/实现，现已移除或简化（详见 requirements.md 对应条目的收敛说明）：

- **点击打开控制台**（原功能 7）— 点击 widget 圆形区域的行为已改为打开设置面板（commit `7fc0588`）。控制台访问改为通过设置面板的 WebView2 登录按钮。
- **未配置状态特殊样式**（原功能 12）— 已简化。未配置时不再显示虚线圆环/"? "图标/"点击配置"提示，改为在圆形右侧显示红色"未配置凭证"错误文字。`ThemeColors` 的 `unconfigured_*` 字段已移除。
- **小/中/大三种尺寸**（原功能 1 子项）— 已收敛为固定 60×60 尺寸（commit `84aeb76`）。
- **可选显示百分比开关**（原功能 1/3 子项）— 已移除，百分比现在总是居中显示。
- **DPAPI 浏览器 cookie 提取**（原功能 10）— `browser_cookies.rs` 已被 `webview_login.rs`（WebView2 登录流程）替代（commit `af7a46f`）。
