# Coding Plan Widget — Features

> 本文档记录所有已验证、已实现的功能。由 acceptance-tester 技能自动维护。

---

## 1. Widget 悬浮显示

**Verified**: 2026-06-15

### Summary
一个透明、无边框、置顶的悬浮 widget，显示 Coding Plan 的用量百分比（圆形进度条），支持多尺寸、多主题，进度条颜色随百分比变化并有平滑动画。

### Key Files
- `src/main.rs` — WidgetApp, update(), widget rendering
- `src/widget.rs` — Widget 渲染逻辑

---

## 2. 系统托盘

**Verified**: 2026-06-15

### Summary
系统托盘图标，支持左键切换显示/隐藏、右键菜单（显示/隐藏、刷新、设置、退出）。

### Key Files
- `src/tray.rs` — 完整的托盘实现

---

## 3. 设置面板

**Verified**: 2026-06-15

### Summary
独立的设置面板，与 widget 互斥显示（同一窗口切换模式），支持 Cookie/CSRF Token/区域代码/刷新间隔/通知阈值/主题/窗口大小/开机自启/显示百分比等配置，分为通用和 Cookie 两个签页。

### Key Files
- `src/main.rs` — render_settings_viewport(), 设置模式切换
- `src/settings.rs` — 设置数据结构与持久化

---

## 4. 用量数据获取

**Verified**: 2026-06-15

### Summary
从火山引擎 API 获取 Coding Plan 用量数据，支持后台异步刷新、定时刷新、悬停刷新、手动刷新。

### Key Files
- `src/api.rs` — API 调用
- `src/main.rs` — fetch_usage(), start_refresh(), check_refresh_result()

---

## 5. 悬停提示框

**Verified**: 2026-06-15

### Summary
鼠标悬停在 widget 圆形区域时，展开显示各层级的用量详情和重置倒计时，拖拽时自动隐藏。

### Key Files
- `src/main.rs` — format_level_line(), tooltip rendering

---

## 6. Widget 拖拽

**Verified**: 2026-06-15

### Summary
通过拖拽圆形区域移动 widget 位置，使用 lerp 平滑移动，拖拽结束后保存窗口位置。

### Key Files
- `src/main.rs` — 自定义拖拽逻辑

---

## 7. 点击打开控制台

**Verified**: 2026-06-15

### Summary
点击 widget 圆形区域打开火山引擎控制台，URL 根据区域代码动态生成。

### Key Files
- `src/main.rs` — open_url(), console_url()

---

## 8. 用量通知

**Verified**: 2026-06-15

### Summary
当用量超过可配置阈值时发送通知，每个阈值周期只通知一次，设为 0 禁用通知。

### Key Files
- `src/main.rs` — show_usage_notification()

---

## 9. 开机自启

**Verified**: 2026-06-15

### Summary
可配置是否开机自动启动，通过 Windows 注册表实现。

### Key Files
- `src/main.rs` — apply_auto_start()

---

## 10. 浏览器 Cookie 自动提取

**Verified**: 2026-06-15

### Summary
首次启动时自动从浏览器或 cookie 文件提取凭证，提取后自动保存到设置文件，已有凭证时不覆盖。

### Key Files
- `src/browser_cookies.rs` — try_extract_credentials()
- `src/main.rs` — Settings::load()

---

## 11. 设置与 Widget 完全分离

**Verified**: 2026-06-15

### Summary
设置面板和 widget 不能同时显示，打开设置时 widget 切换为设置模式，关闭设置后恢复 widget，模式切换时动态改变窗口装饰和大小。

### Key Files
- `src/main.rs` — was_in_settings_mode 追踪模式切换

---

## 12. 未配置状态特殊样式

**Verified**: 2026-06-15

### Summary
当没有本地配置时，widget 显示特殊视觉样式（暗淡背景、虚线圆环、"?" 图标、"点击配置" 提示），支持拖拽和点击打开设置。

### Key Files
- `src/widget.rs` — is_configured() 分支渲染
- `src/theme.rs` — ThemeColors 未配置配色字段
