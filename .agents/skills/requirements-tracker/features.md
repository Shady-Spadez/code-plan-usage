# Coding Plan Widget — 功能文档

> 本文档记录已验证、已实现的功能。每个功能经过验收测试确认满足验收标准。

---

## Widget 悬浮显示

**Verified**: 2026-06-22

### Summary
透明、无边框、置顶的悬浮 widget，显示 Coding Plan 用量百分比（圆形进度条），固定 60×60 尺寸。

### Key Files
- `src/widget.rs` — WidgetApp, update(), widget rendering
- `src/theme.rs` — widget_config(), percent_color(), Theme

---

## 系统托盘

**Verified**: 2026-06-22

### Summary
系统托盘图标，左键切换 widget 显示/隐藏，右键菜单支持显示/隐藏、刷新、设置、退出。

### Key Files
- `src/tray.rs` — 完整的托盘实现

---

## 设置面板

**Verified**: 2026-06-22

### Summary
独立 popup window 设置面板（640×720），通过 show_viewport_immediate 与 widget 窗口分离。分为通用和 Cookie 两个签页，支持 Cookie/CSRF Token/区域/刷新间隔/通知阈值/主题/开机自启配置。

### Key Files
- `src/widget.rs` — render_settings_viewport(), SettingsViewportState
- `src/settings.rs` — Settings 结构与持久化

---

## 用量数据获取

**Verified**: 2026-06-22

### Summary
从火山引擎 API 获取 Coding Plan 用量数据，后台线程异步刷新，支持定时刷新（默认 5 分钟）、悬停触发刷新、配额重置后自动刷新。

### Key Files
- `src/api.rs` — fetch_usage(), format_level_line(), is_reset_expired()
- `src/widget.rs` — start_refresh(), apply_refresh_result()

---

## 悬停提示框

**Verified**: 2026-06-22

### Summary
鼠标悬停时展开显示各层级用量详情，提示框方位按优先级自适应（右下→左下→左上→右上），水平位置钳制到工作区确保完整可见。

### Key Files
- `src/widget.rs` — 提示框尺寸/方位计算、窗口几何与绘制
- `src/screen.rs` — work_area_for_point(), screen_info_for_point()

---

## Widget 拖拽

**Verified**: 2026-06-22

### Summary
拖拽圆形区域移动 widget 位置，基于屏幕坐标跟踪鼠标，拖拽结束保存位置。widget 不能拖出屏幕或任务栏。

### Key Files
- `src/widget.rs` — drag_anchor, clamp_home_to_work_area()
- `src/screen.rs` — screen_info_for_point()

---

## 用量通知

**Verified**: 2026-06-22

### Summary
用量超过可配置阈值时发送 Windows 通知，每个阈值周期只通知一次，设为 0 禁用。

### Key Files
- `src/main.rs` — show_usage_notification()
- `src/widget.rs` — notification_sent, apply_refresh_result() 阈值检查

---

## 开机自启

**Verified**: 2026-06-22

### Summary
通过 Windows 注册表实现开机自启，设置面板中可开关。

### Key Files
- `src/main.rs` — apply_auto_start()

---

## 浏览器 Cookie 自动提取

**Verified**: 2026-06-22

### Summary
凭证获取支持两条路径：启动时自动从 cookie 文件读取；设置面板中通过 WebView2 登录窗口手动提取。提供清理 Cookie 按钮。

### Key Files
- `src/settings.rs` — try_load_from_cookie_file()
- `src/webview_login.rs` — try_extract_credentials(), clear_webview_cookies()
- `src/widget.rs` — 设置面板 WebView2 receiver 轮询

### Known Limitations
- WebView2 登录 URL 硬编码为 cn-beijing 区域

---

## 设置与 Widget 完全分离

**Verified**: 2026-06-22

### Summary
设置面板作为独立 popup window 显示，与 widget 窗口分离，无黑色背景问题。关闭设置后恢复 widget 交互。

### Key Files
- `src/widget.rs` — show_viewport_immediate(), clear_color(), SettingsViewportState

---

## Cookie 过期无感自动重试

**Verified**: 2026-06-22

### Summary
当 API 请求因 Cookie 过期而失败时，自动清除旧用量数据，无感（隐藏窗口）尝试从 WebView2 Cookie 存储重新获取凭证。获取到新凭证则自动重试；无法获取则显示"Cookie 过期，请重新登录"。

### Key Files
- `src/webview_login.rs` — try_silent_extract_credentials(), run_silent_extraction(), silent_wndproc(), SILENT_COOKIE_CHECK_SCRIPT
- `src/widget.rs` — apply_refresh_result() 错误分支, start_silent_reauth(), check_silent_reauth_result(), is_likely_cookie_error()

### Known Limitations
- 用户从未使用 WebView2 登录（手动粘贴 Cookie）时，静默提取无法获取凭证
- WebView2 未安装时静默提取失败，显示"Cookie 过期"

---

## Coconut 首次启动磁盘默认文件生成

**Verified**: 2026-06-26

### Summary
首次启动时自动将 CoconutSettings 默认值写入 coconut_settings.json，后续启动从文件读取。写入失败不中断程序。

### Key Files
- `src/bin/coconut/settings.rs` — load() 首次写入逻辑，load_from_path()/save_to_path() 路径参数化
