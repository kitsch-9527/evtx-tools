# 二进制部署与跨机使用说明

> 目标：把 evet 的两个可执行文件带到**其他机器**上使用。
> 部署清单极简——两个 exe + 一个消息库，无运行时依赖（Rust 静态链接、SQLite bundled）。
> 生成日期：2026-10-01（对应构建：R26–R30 终态，schema v9）。

---

## 1. 部署清单（拷这三个文件）

| 文件 | 作用 | 部署到目标机 |
|---|---|---|
| `evtx-parity-test/tools/evtx-tools.exe` | **离线解析器**：EVTX → CSV/SQLite，含消息本地化、typed 渲染、`--profile` 分析 | 任意机器（设计上不依赖 Windows API） |
| `evtx-parity-test/tools/msgdb-dump-x64.exe` | **消息库生成器**：从本机注册表/DLL/清单静态萃取消息库 | 仅 Windows（需要注册表与系统 DLL 读取权限，建议管理员） |
| `evtx-parity-test/msgdb/hotech-msg-*.db` | **消息库本体**（schema v9，约 350 MB） | 跟随解析目标：解析"哪台机器的日志"就用"哪台机器的库" |

> ⚠️ 不要拷贝 `evtx-batch.exe`——旧版化石产物已隔离（`*.stale-*`），源码中已无此工具。

---

## 2. 三种使用场景

### 场景 A：在目标 Windows 机器上解析**它自己**的日志（推荐，准确度最高）

```bat
:: 1. 在目标机上生成它自己的消息库（管理员，~3 分钟）
msgdb-dump-x64.exe -out-dir E:\evet -lang 1033,2052
::    产出 E:\evet\hotech-msg-<系统>-<build>.db

:: 2. 用它解析该机的任意 EVTX（含运行时导出或第三方采集的）
evtx-tools.exe parse C:\Windows\System32\winevt\Logs\System.evtx ^
    -o out\System.csv --format csv --msg-db E:\evet\hotech-msg-*.db --lang zh-CN
```

这是唯一保证**与目标机事件查看器逐字一致**的路径（本项目 PASS 验证即此形态）。
`-lang 1033,2052` 控制萃取哪些语言（1033=en，2052=zh-CN，可加其他 LCID）。

### 场景 B：在任意机器上解析**第三方 EVTX**（取证/分析形态）

用本机（或源机）的消息库解析别处拿来的 EVTX：

```bash
evtx-tools.exe parse evidence.evtx -o out.csv --format csv --msg-db hotech-msg.db --lang zh-CN
```

**覆盖面规则**（务必理解）：
- 消息库的覆盖面 = 生成它的那台机器所注册的 provider。源 EVTX 里的 provider 在库中**有** →
  完整渲染；**没有** → 该事件 message 回退为裸参数（`;;` 连接的 Data 直出）或 N/A。
- 因此：解析"某服务器"的日志，最好用"那台服务器"跑一次场景 A 生成的库。
- 语言：请求语言缺失自动回落 en-US（与 wevtapi 语义一致）。

### 场景 C：目标机不可登录，只有它的 DLL 文件（离线导入）

把目标机的消息 DLL（EventMessageFile 链上的 .dll/.exe/.sys）收集到一个目录：

```bat
msgdb-dump-x64.exe -out-dir E:\evet -lang 1033,2052 -import-dll E:\collected-dlls
```

工具从目录静态萃取消息表并以**文件名派生 provider 名**（`e1dmsg.dll` → provider "e1dmsg"）。
局限：provider 名来自文件名而非注册表，语言块依赖收集到的文件（收集时把对应 MUI 一并带上）。

---

## 3. evtx-tools.exe 命令参考

```
parse   解析 .evtx → CSV/SQLite
info    消息库元数据（schema v9；旧库提示 re-run msgdb-dump）
```

**parse 参数**（完整说明见 `docs/evtx-tools-usage.md`）：

| 参数 | 说明 |
|---|---|
| `-o <路径>` | 输出文件；**目录须先建**；重复运行=覆盖（快照语义，不会累积） |
| `--format` | `db` / `csv` / `both`（both 会抢写同一路径，需要两种格式分两次跑） |
| `--msg-db` / `--msg-dir` | 消息库（目录形态自动探测 `hotech-msg-*.db`） |
| `--lang` | 默认 `zh-CN`，缺失回落 en-US |
| `--with-xml` | 附 raw XML |
| `--profile <名>` | security / persistence / system / remote / process / powershell / mssql / web |
| `--no-stats` | 跳过 stat_* 统计表 |

**info 示例**（验库三要素：schema=9、provider 数、variant 数）：

```
evtx-tools.exe info hotech-msg-....db
  Schema version: 9
  Provider count: 1170
  Variant count:  1553249
```

---

## 4. 已知限制与坑位

| 坑 | 说明 / 处置 |
|---|---|
| 消息库与渲染器版本必须配对 | 库是 schema v9 + 现行 exe（R26–R30）。旧 exe 遇 v9 库直接拒开；`info` 曾停在 v7（R30 已修） |
| `-o` 目录须先建 | 工具不自动 mkdir |
| `--format both` 抢写 | 分两次跑（db 一次、csv 一次） |
| CSV 含重复表头行 | 每 batch 夹带 BOM+表头；下游按 event_id 纯数字过滤 |
| 跨机覆盖面 | 见场景 B 规则——**库不认识的 provider 回退裸参数**，不是工具坏了 |
| msgdb-dump 仅 Windows | 萃取依赖注册表与系统 DLL；Linux/macOS 只能跑 evtx-tools.exe（Rust 静态，理论可跨平台，官方验证以 Windows 为准） |
| 不要跑源码树里的旧 exe | `msgdb-dump/` 下有历史 exe（含 9 月初的旧格式产物）；只用最新构建 |

---

## 5. 验证部署成功（30 秒自检）

```bat
:: 1. 解析器活着
evtx-tools.exe --version

:: 2. 消息库可开且 schema=9
evtx-tools.exe info hotech-msg-*.db

:: 3. 实解析一条
evtx-tools.exe parse <某个.evtx> -o out\demo.csv --format csv --msg-db <库> --lang zh-CN
:: 打开 demo.csv：message 列应有本地化文本而非 ";;" 裸参数
```

若第 3 步出现 `;;` 裸参数：该事件的 provider 不在消息库覆盖面内（场景 B 规则），
或 `--msg-db` 没带上（工具会直接报错退出，不会静默产出）。
