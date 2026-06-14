# Coding Plan Widget — 需求文档

> 本文档记录所有已实现和计划中的功能需求。每次新增或修改功能时，需先阅读本文档，确保不破坏已有功能。

---

## 1. Widget 悬浮显示

**Date**: 2026-06-14
**Status**: ✅ Implemented

### Description
一个透明、无边框、置顶的悬浮 widget，显示 Coding Plan 的用量百分比（圆形进度条）。

### Acceptance Criteria
- [x] 窗口透明、无边框、始终置顶
- [x] 显示圆形进度条，表示当月用量百分比
- [x] 支持小/中/大三种尺寸
- [x] 支持暗色/亮色主题
- [x] 可选显示百分比数字
- [x] 进度条颜色随百分比变化（绿→黄→橙→红）
- [x] 进度条有平滑动画过渡

### Implementation Notes
- `src/main.rs`: WidgetApp, update(), widget rendering
- 窗口创建参数: `with_transparent(true)`, `with_decorations(false)`, `with_always_on_top()`

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
独立的设置面板，与 widget 互斥显示（同一窗口切换模式）。

### Acceptance Criteria
- [x] 从托盘菜单"设置"打开
- [x] 设置和 widget 共享同一窗口，互斥显示
- [x] 设置模式下窗口有标题栏（可拖动）
- [x] 设置模式下窗口大小为 640×720
- [x] 关闭设置后自动切换回 widget 模式
- [x] 点击 OS 关闭按钮返回 widget 模式而非退出应用
- [x] 深色主题 UI
- [x] 配置项：Cookie、CSRF Token、区域代码、刷新间隔、通知阈值、主题、窗口大小、开机自启、显示百分比
- [x] 保存按钮（手动保存，无自动保存）
- [x] X 按钮关闭设置（有未保存更改时弹出确认对话框：保存并退出 / 不保存）
- [x] 设置面板分为两个签页：通用（区域/刷新/通知/外观/其他）和 Cookie（Cookie/CSRF Token）
- [x] 外观设置（主题、窗口大小、显示百分比）实时同步到 widget
- [x] 点击 widget 图标打开设置面板

### Implementation Notes
- `src/main.rs`: render_settings_viewport(), update() 中的设置模式切换
- 设置和 widget 通过 `was_in_settings_mode` 字段追踪模式切换
- 模式切换时发送 `ViewportCommand::Decorations` 和 `ViewportCommand::InnerSize`

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
- `src/main.rs`: fetch_usage(), start_refresh(), check_refresh_result()
- API URL: `https://console.volcengine.com/api/top/ark/{region}/2024-01-01/GetCodingPlanUsage`

---

## 5. 悬停提示框

**Date**: 2026-06-14
**Status**: ✅ Implemented

### Description
鼠标悬停在 widget 圆形区域时，展开显示各层级的用量详情。

### Acceptance Criteria
- [x] 显示各层级的用量百分比和重置倒计时
- [x] 窗口自动扩展以容纳提示框
- [x] 拖拽时隐藏提示框
- [x] 颜色与百分比对应

### Implementation Notes
- `src/main.rs`: format_level_line(), tooltip rendering in update()

---

## 6. Widget 拖拽

**Date**: 2026-06-14
**Status**: ✅ Implemented

### Description
通过拖拽圆形区域移动 widget 位置。

### Acceptance Criteria
- [x] 仅在圆形区域可发起拖拽
- [x] 拖拽时使用 lerp 平滑移动
- [x] 拖拽结束后保存窗口位置
- [x] 拖拽时隐藏提示框

### Implementation Notes
- `src/main.rs`: 自定义拖拽逻辑，使用屏幕坐标追踪

---

## 7. 点击打开控制台

**Date**: 2026-06-14
**Status**: ✅ Implemented

### Description
点击 widget 圆形区域打开火山引擎控制台。

### Acceptance Criteria
- [x] 点击圆形区域打开浏览器
- [x] URL 根据区域代码动态生成

### Implementation Notes
- `src/main.rs`: open_url(), console_url()

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
- `src/main.rs`: show_usage_notification(), notification_sent 字段

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
首次启动时自动从浏览器或 cookie 文件提取凭证。

### Acceptance Criteria
- [x] 自动检测浏览器 cookie
- [x] 支持从 cookie 文件读取
- [x] 提取后自动保存到设置文件
- [x] 已有凭证时不覆盖

### Implementation Notes
- `src/browser_cookies.rs`: try_extract_credentials()
- `src/main.rs`: Settings::load()

---

## 11. 设置与 Widget 完全分离

**Date**: 2026-06-14
**Status**: ✅ Implemented

### Description
设置面板和 widget 不能同时显示，打开设置时 widget 切换为设置模式，关闭设置后恢复 widget。

### Acceptance Criteria
- [x] 打开设置时不显示 widget（无黑色背景）
- [x] 设置窗口有标题栏可拖动
- [x] 设置和 widget 互斥显示
- [x] 关闭设置自动恢复 widget

### Implementation Notes
- 通过 `was_in_settings_mode` 追踪模式切换
- 模式切换时动态改变窗口装饰和大小
