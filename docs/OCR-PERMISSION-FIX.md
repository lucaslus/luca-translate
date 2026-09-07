# macOS OCR 权限误判修复（2026-09-04）

## 原因和证据

应用原先直接依赖 `core-graphics 0.24`。它将
`CGPreflightScreenCaptureAccess` / `CGRequestScreenCaptureAccess` 的返回值
声明成 `boolean_t`（32 位），再与整数 1 比较；Apple SDK 实际声明为 C `bool`。
在 x86_64 调用约定下，高位寄存器内容不能用于判断这个布尔返回值，可能造成
已经授权却返回 false，且同一个进程里表现不一致。

- [上游 Intel Mac 误判报告 #677](https://github.com/servo/core-foundation-rs/issues/677)
- [上游返回类型修复 #698](https://github.com/servo/core-foundation-rs/pull/698)
- 本地 `screen_capture_abi` 测试在 0.24 下复现“已授权被读成 false”，升级至
  0.25 后通过。测试模拟系统函数返回值，不访问或修改真实 TCC 权限。

本轮检查发现已安装应用的代码签名有效。安装更新时核对新旧 designated
requirement 一致；未重置任何系统权限。

## 改动

- 升级至 `core-graphics 0.25`，移除历史成功权限缓存，每次读取真实权限。
- 截图失败时重新检查权限，普通截图错误不再统一显示“无权限”。
- 截图显式使用 `-T0`；OCR 直接读取内存 PNG，主窗口在截图后、OCR 完成前显示进度。
- 修正 Vision `minimumTextHeight` 的单位：它是图片高度比例，不是像素。
- 复用隐藏的选区窗口，等待前端监听器注册后初始化；提交后阻止重复提交或取消，
  修正选区绘制的局部坐标与显示器缩放。
- 权限卡片保持“打开设置”“重启应用”两个按钮，缩短说明，不再默认要求清空授权。

## 验证

```bash
cd app/src-tauri
cargo test --bin lucas-translate --test screen_capture_abi
cd ../..
node --test tests/overlay.test.cjs
```

通过 5 个 Rust 测试和 3 个前端状态测试；权限 ABI 的 2 个回归测试也通过了
Release 优化模式验证。另用用户提供的图片实际运行 Vision，
成功识别出 12 行文字。

通过 LaunchServices 启动 `/Applications/lucas-translate.app` 的诊断入口，
在同一应用进程中对显式指定区域连续截图、OCR，未调用翻译服务或写入剪贴板。

| 次数 | 截图前/后权限 | 截图耗时 | OCR 耗时 | 识别字符数 |
| --- | --- | --- | --- | --- |
| 1 | true / true | 156 ms | 610 ms | 160 |
| 2 | true / true | 227 ms | 324 ms | 160 |
| 3 | true / true | 105 ms | 312 ms | 160 |

随后在正常运行模式收到两次实际截图操作，均读到 `preflight=true`：

| 操作 | 选区后截图耗时 | OCR 耗时 | 识别字符数 | 非空翻译结果记录 |
| --- | --- | --- | --- | --- |
| 1 | 224 ms | 569 ms | 67 | 16:01:41，271 字符 |
| 2 | 220 ms | 223 ms | 41 | 16:01:50，184 字符 |

翻译记录使用只读 SQLite 查询核对时间及字符数，未读取或输出正文。正常入口的
两次截图、OCR 和自动翻译均有运行证据，第二次没有权限误判。界面的最终视觉效果
未由自动化验证（自动注入的快捷键未触发应用）。不要把命令行辅助进程的权限状态
当成已安装应用的权限状态。

诊断用法：先退出应用，将待测试区域显式填入 `x,y,w,h`，然后运行：

```bash
open /Applications/lucas-translate.app --stdout /tmp/lucas-ocr.log --stderr /tmp/lucas-ocr-error.log --args --diagnose-ocr 100,150,620,230
```

诊断只执行三次；输出路径、权限、耗时、行数和字符数，不输出识别文本或图片。
结束后正常打开应用即可。
