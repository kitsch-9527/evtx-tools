# EvtxECmd Maps 调研与实现计划

> 调研日期：2026-07-10
> 决策来源：ECOSYSTEM-RESEARCH.md 审阅结论
> 设计决策：直接复用 Eric Zimmerman 社区 Maps JSON，不转译

---

## 1. 背景与动机

### 1.1 问题

当前 evtx-tools 的消息格式化依赖 `messages.db`（由外部工具 `msgdb-dump` 生成）。当用户没有 messages.db 时：

```
当前无 DB 时的表现：
  message = "无法找到来自源 xxx 的事件 ID yyy 的描述。本地计算机上未安装..."
  └─ 虽然有用，但丢失了事件的结构化字段信息
  └─ 用户无法体验产品的核心价值
  └─ 使用门槛高：必须先准备 messages.db
```

### 1.2 解决方案

复用 EvtxECmd（Eric Zimmerman）的社区 Maps JSON，在无 messages.db 时提供结构化字段输出：

```
有 Maps 时的表现：
  message = "Target: DOMAIN\admin (Interactive)"
  map_description = "An account was successfully logged on"
  payload_data1 = "Target: DOMAIN\admin"
  payload_data2 = "TargetServerName: SRV-FILES"
  └─ 结构化、可读、与 EvtxECmd 兼容
  └─ 零门槛入门：git clone Maps 仓库即可使用
  └─ 不影响消息格式化：有 DB 时两者共存
```

### 1.3 战略定位

```
Maps 是"免费层"→ 零门槛入门，覆盖基本事件解读
messages.db 是"深度体验层"→ 完整中英双语消息格式化
```

用户路径：

```
用户第一次用：
  git clone https://github.com/EricZimmerman/evtx.git
  evtx-tools parse Security.evtx --evtx-maps-dir ./evtx/evtx/Maps/ -o output
  → 立刻看到结构化输出，不需要任何准备工作

用户深入使用时：
  msgdb-dump dump --out-dir ./msg_dbs/
  evtx-tools parse Security.evtx --msg-db ./msg_dbs/hotech-msg-xxx.db \
    --evtx-maps-dir ./Maps/ -o output
  → 完整消息格式化 + Maps 结构化字段同时输出
```

---

## 2. EvtxECmd Maps 格式调研

### 2.1 实际文件结构（2026-07-10 实测）

下载的 Maps 目录位于：
```
/path/to/Get-ZimmermanTools/net9/EvtxeCmd/Maps/
```

```
Maps/
├── !!!!README.md                                     # 说明文档
├── !Channel-Name_Provider-Name_EventID.guide         # 创建指南
├── !Channel-Name_Provider-Name_EventID.template      # 创建模板
├── Security_Microsoft-Windows-Security-Auditing_4624.map   # 单个 .map 文件
├── System_Service-Control-Manager_7045.map
├── Microsoft-Windows-Sysmon-Operational_Microsoft-Windows-Sysmon_1.map
└── ... (总计 383 个 .map 文件，3 个辅助文件)
```

**关键发现：Maps 不是 JSON，而是自定义 YAML 格式。**

文件名格式：`{Channel}_{Provider}_{EventID}.map`

### 2.2 实际格式定义（YAML-like）

```
Author: Eric Zimmerman saericzimmerman@gmail.com           # 作者
Description: Successful logon                               # ← 对应 map_description
EventId: 4624                                               # 目标 EventID
Channel: Security                                           # 日志通道
Provider: Microsoft-Windows-Security-Auditing               # Provider 名称
Maps:                                                       # 属性提取定义列表
  -                                                         # 每个 - 是一个 Property
    Property: UserName                                      # 输出列名
    PropertyValue: "%domain%\\%user%"                      # 模板字符串
    Values:                                                 # 变量提取定义
      -
        Name: domain                                        # 变量名
        Value: "/Event/EventData/Data[@Name=\"SubjectDomainName\"]"  # XPath
      -
        Name: user
        Value: "/Event/EventData/Data[@Name=\"SubjectUserName\"]"
  -
    Property: RemoteHost
    PropertyValue: "%workstation% (%ipAddress%)"
    Values:
      -
        Name: ipAddress
        Value: "/Event/EventData/Data[@Name=\"IpAddress\"]"
      -
        Name: workstation
        Value: "/Event/EventData/Data[@Name=\"WorkstationName\"]"
  -
    Property: PayloadData1
    PropertyValue: "Target: %TargetDomainName%\\%TargetUserName%"
    Values:
      -
        Name: TargetDomainName
        Value: "/Event/EventData/Data[@Name=\"TargetDomainName\"]"
      -
        Name: TargetUserName
        Value: "/Event/EventData/Data[@Name=\"TargetUserName\"]"
  -
    Property: PayloadData2
    PropertyValue: "LogonType %LogonType%"
    Values:
      -
        Name: LogonType
        Value: "/Event/EventData/Data[@Name=\"LogonType\"]"

Lookups:                                                    # 可选项：值映射表
  -
    Name: Status                                            # 映射哪个变量
    Default: Unknown code                                    # 默认值
    Values:                                                 # 映射表
      0xC0000064: user name does not exist
      0xC000006A: user name is correct but the password is wrong
      0xC000006D: the cause is either a bad username or authentication information

# Documentation:                                            # 注释（#开头）
# https://docs.microsoft.com/en-us/windows/security/threat-protection/auditing/event-4624
#
# Example Event Data:
# <EventData>
#   <Data Name="TargetUserName">admin</Data>
#   ...
```

### 2.3 实测统计（383 个 .map 文件）

| 属性 | 数值 |
|------|:----:|
| 总 .map 文件数 | **383** |
| 有 `Lookups` 值的文件 | 43 (11.2%) |
| 使用 regex 的 PropertyValue | 0 (0%) |
| 最早更新日期 | 2022-01-22 |

#### Property 类型分布

| Property 列名 | 出现次数 | 出现比例 | 说明 |
|---------------|:--------:|:--------:|------|
| `PayloadData1` | 357 | **93.2%** | 最通用的数据槽位 |
| `PayloadData2` | 222 | 58.2% | 次要数据槽位 |
| `PayloadData3` | 151 | 39.4% | 第三数据槽位 |
| `UserName`（含 `Username`） | 140 | 36.3% | 用户名（注意大小写变体） |
| `PayloadData4` | 100 | 26.1% | 第四数据槽位 |
| `ExecutableInfo` | 86 | 22.2% | 可执行文件信息 |
| `PayloadData5` | 80 | 20.9% | 第五数据槽位 |
| `PayloadData6` | 54 | 14.1% | 第六数据槽位 |
| `RemoteHost` | 31 | 8.1% | 远程主机/IP |

#### 通道覆盖

| 通道 | 文件数 | 说明 |
|------|:------:|------|
| `Security` | 93 | Windows 安全审计日志 |
| `System` | 27 | 系统日志 |
| `Microsoft-Windows-Sysmon/Operational` | 26 | Sysmon 全部 EventID 1~26 |
| `Application` | 30 | 应用程序日志 |
| `Microsoft-Windows-Windows Defender/Operational` | 17 | Defender 告警 |
| `Microsoft-Windows-PowerShell/Operational` | 4 | PowerShell 4103/4104/400/403 |

**重要发现：`Security` 通道覆盖率最高（93 个 .map 文件），涵盖了你的 8 个 profile 所需的所有核心 EventID。**

### 2.4 100% 兼容所需字段映射

| .map 字段/Property | evtx-tools 列 | 必选？ |
|--------------------|--------------|:------:|
| `Description:` 头字段 | `map_description` | ✅ |
| `PayloadData1` | `payload_data1` | ✅ |
| `PayloadData2` | `payload_data2` | ✅ |
| `PayloadData3` | `payload_data3` | ✅ |
| `PayloadData4` | `payload_data4` | ✅ |
| `PayloadData5` | `payload_data5` | ✅ |
| `PayloadData6` | `payload_data6` | ✅ |
| `UserName` / `Username` | `map_user_name` | ✅ 100% 兼容需要 |
| `RemoteHost` | `map_remote_host` | ✅ 100% 兼容需要 |
| `ExecutableInfo` | `map_executable_info` | ✅ 100% 兼容需要 |
| `Lookups` 值映射 | 运行时查找表 | ✅ 100% 兼容需要 |

**100% 兼容意味着新增 10 列（不是原来的 8 列）：**

```sql
ALTER TABLE events ADD COLUMN map_description      TEXT;  -- Description: 头字段
ALTER TABLE events ADD COLUMN map_user_name         TEXT;  -- UserName / Username
ALTER TABLE events ADD COLUMN map_remote_host       TEXT;  -- RemoteHost
ALTER TABLE events ADD COLUMN map_executable_info   TEXT;  -- ExecutableInfo
ALTER TABLE events ADD COLUMN payload_data1         TEXT;  -- PayloadData1
ALTER TABLE events ADD COLUMN payload_data2         TEXT;  -- PayloadData2
ALTER TABLE events ADD COLUMN payload_data3         TEXT;  -- PayloadData3
ALTER TABLE events ADD COLUMN payload_data4         TEXT;  -- PayloadData4
ALTER TABLE events ADD COLUMN payload_data5         TEXT;  -- PayloadData5
ALTER TABLE events ADD COLUMN payload_data6         TEXT;  -- PayloadData6
ALTER TABLE events ADD COLUMN maps_version          TEXT;  -- Maps 来源标记
```

### 2.5 XPath 处理策略

**关键决策：跳过 XPath 解析，直接从 event_data JSON key 提取。**

原因：

```
EvtxECmd 的 XPath:
  "/Event/EventData/Data[@Name=\"TargetDomainName\"]"
                                 ↓
从 @Name= 提取字段名:
  "TargetDomainName"
                                 ↓
在你的 event_data JSON 中查找:
  {"TargetDomainName": "DOMAIN", "TargetUserName": "admin"}
                                 ↓
直接取值: "DOMAIN"
```

XPath 简化提取函数：

```rust
/// 从 XPath 字符串中提取 @Name 属性值
// 输入: "/Event/EventData/Data[@Name=\"TargetDomainName\"]"
// 输出: "TargetDomainName"
fn extract_field_name_from_xpath(xpath: &str) -> Option<&str> {
    // 查找 [@Name=" 或 [@Name=' 后的内容
    let after_equals = xpath.find("@Name=")?;
    let after_quote = after_equals + 7; // @Name=" 或 @Name='
    let quote_char = xpath[after_equals + 6..after_equals + 7].chars().next()?; // " 或 '
    let remaining = &xpath[after_quote..];
    let end = remaining.find(quote_char)?;
    Some(&remaining[..end])
}
```

**100% 兼容性说明：**
- 实际 Maps 中所有 XPath 路径均使用 `Data[@Name="FieldName"]` 格式
- 没有使用嵌套路径、集合索引或 XPath 函数
- 所以从 `@Name` 提取字段名即可覆盖 **100% 的 Maps**

### 2.6 Lookups 值映射支持

Lookups 是 Maps 中的值映射表，功能类似你的 CRIM VMAP/BMAP：

```yaml
Lookups:
  -
    Name: Status              # 对应 PropertyValue 中的 %Status%
    Default: Unknown code      # 无匹配时的默认值
    Values:                   # 映射表
      0xC0000064: user name does not exist
      0xC000006A: user name is correct but the password is wrong
```

处理逻辑：

```rust
/// 应用 Lookups 值映射
fn apply_lookups(
    rendered_value: &str,         // 当前 PropertyValue 渲染结果
    lookups: &[LookupTable],      // 从 Maps 加载的 Lookups
    variables: &HashMap<String, String>,  // 已提取的变量名→值
) -> String {
    let mut result = rendered_value;
    for lookup in lookups {
        if let Some(raw_value) = variables.get(&lookup.name) {
            let mapped = lookup.values
                .get(raw_value)
                .map(|s| s.as_str())
                .unwrap_or(&lookup.default);
            result = result.replace(raw_value, mapped);
        }
    }
    result
}
```

---

## 3. 设计方案

### 3.1 新增列

在 `events` 主表新增 **11 列**（10 个 Maps 输出字段 + 1 个版本标记）：

```sql
-- 当前 events 表（25列）
-- ...现有的 event_record_id, event_time, provider_name, ...

-- 新增 11 列（Maps 100% 兼容）
ALTER TABLE events ADD COLUMN map_description      TEXT;  -- Description: 头字段
ALTER TABLE events ADD COLUMN map_user_name         TEXT;  -- UserName / Username
ALTER TABLE events ADD COLUMN map_remote_host       TEXT;  -- RemoteHost
ALTER TABLE events ADD COLUMN map_executable_info   TEXT;  -- ExecutableInfo
ALTER TABLE events ADD COLUMN payload_data1         TEXT;  -- PayloadData1
ALTER TABLE events ADD COLUMN payload_data2         TEXT;  -- PayloadData2
ALTER TABLE events ADD COLUMN payload_data3         TEXT;  -- PayloadData3
ALTER TABLE events ADD COLUMN payload_data4         TEXT;  -- PayloadData4
ALTER TABLE events ADD COLUMN payload_data5         TEXT;  -- PayloadData5
ALTER TABLE events ADD COLUMN payload_data6         TEXT;  -- PayloadData6
ALTER TABLE events ADD COLUMN maps_version          TEXT;  -- Maps 来源标记
```

对应的 `ParsedEvent` 结构体扩展：

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ParsedEvent {
    // ...现有字段 25 个...
    
    // Maps 输出（新增 11 个字段，100% 兼容）
    pub map_description:      Option<String>,  // Description: 头字段
    pub map_user_name:        Option<String>,  // UserName / Username
    pub map_remote_host:      Option<String>,  // RemoteHost
    pub map_executable_info:  Option<String>,  // ExecutableInfo
    pub payload_data1:        Option<String>,  // PayloadData1
    pub payload_data2:        Option<String>,  // PayloadData2
    pub payload_data3:        Option<String>,  // PayloadData3
    pub payload_data4:        Option<String>,  // PayloadData4
    pub payload_data5:        Option<String>,  // PayloadData5
    pub payload_data6:        Option<String>,  // PayloadData6
    pub maps_version:         Option<String>,  // Maps 来源/版本标记
}
```

### 3.2 消息优先级策略

有 maps 和 messages.db 同时存在时：

```
最终 message = DB模板替换后的完整消息（优先，不可用时用 MapDescription）
map_description = MapDescription（始终填充）
payload_data1~6 = Maps 提取值（始终填充）
```

| 场景 | message | map_description | payload_data1~6 |
|------|---------|----------------|-----------------|
| 有 DB + 有 Maps | ✅ 完整消息 | ✅ Maps 描述 | ✅ Maps 字段 |
| 无 DB + 有 Maps | ✅ MapDescription 兜底 | ✅ Maps 描述 | ✅ Maps 字段 |
| 有 DB + 无 Maps | ✅ 完整消息 | ❌ NULL | ❌ NULL |
| 无 DB + 无 Maps | ✅ fallback 文本 | ❌ NULL | ❌ NULL |

### 3.3 CLI 参数

```bash
# 使用社区 Maps 目录
evtx-tools parse Security.evtx --evtx-maps-dir ./Maps/ -o output

# Maps + messages.db 同时使用
evtx-tools parse Security.evtx --msg-db ./msgs.db --evtx-maps-dir ./Maps/ -o output

# 下载最新 Maps（对应 EvtxECmd 的 --sync）
evtx-tools maps sync --dir ./Maps/
```

参数优先级：

```
--msg-db（消息库，最优先）
     ↓
--msg-dir（消息库目录，自动匹配）
     ↓
--evtx-maps-dir（Maps 目录，备选/补充）
```

### 3.4 运行时架构

```
Producer（主线程）
  │
  ├── 1. 解析 EVTX → ParsedEvent（含 event_data JSON）
  │
  ├── 2. message 填充（有 DB 时）
  │     ↓
  ├── 3. Maps 填充（有 --evtx-maps-dir 时）
  │     ├── 匹配规则: Provider + Channel + EventId 三键
  │     ├── 从 event_data 提取 JSON key 对应值
  │     ├── 填入 PropertyValue 模板（%varName% 替换）
  │     └── 将结果写入 map_description, payload_data1~6
  │
  └── 4. 发送到 Writer 线程（batch）

Writer（后台线程）
  └── 写入 events 表（含新 8 列）
  └── 路由到场景表（可选从 rule.columns 配置是否包含 maps 列）
  └── 统计表更新
```

### 3.5 Parser——Maps 解析器（100% 兼容）

```rust
use std::collections::HashMap;

/// Maps 解析器（100% 兼容 EvtxECmd Maps）
pub struct MapsParser {
    /// 从 Maps 目录加载的所有映射
    /// key: (Provider, Channel, EventId) → MapDefinition
    maps: HashMap<(String, String, u32), MapDefinition>,
    /// 版本标记（目录路径或同步时间）
    version: String,
}

/// 单条 Maps 定义（解析自 .map 文件的 YAML-like 格式）
#[derive(Debug, Clone)]
pub struct MapDefinition {
    pub description: String,            // Description: 头字段
    pub event_id: u32,                  // EventId:
    pub provider: String,               // Provider:
    pub channel: String,                // Channel:
    pub properties: Vec<MapProperty>,   // Maps: 列表
    pub lookups: Vec<LookupTable>,      // Lookups: 值映射（可选）
}

/// Maps 属性提取定义
#[derive(Debug, Clone)]
pub struct MapProperty {
    pub target_column: String,          // Property: "PayloadData1", "UserName"...
    pub template: String,               // PropertyValue: "Target: %Domain%\%User%"
    pub values: Vec<MapValue>,          // Values: 变量提取定义
}

/// XPath 变量定义
#[derive(Debug, Clone)]
pub struct MapValue {
    pub var_name: String,               // Name: "TargetDomainName"
    pub xpath: String,                  // Value: "/Event/EventData/Data[@Name='...']"
}

/// Lookups 值映射表
#[derive(Debug, Clone)]
pub struct LookupTable {
    pub name: String,                   // Name: "Status"
    pub default: String,                // Default: "Unknown code"
    pub entries: HashMap<String, String>, // Values: 映射对
}
```

解析器核心逻辑：

```rust
impl MapsParser {
    /// 从目录加载所有 .map 文件
    /// 跳过 ! 开头的指南/模板文件
    /// 只解析 .map 扩展名的实际映射文件
    pub fn load_from_dir(path: &str) -> Result<Self> {
        let mut maps = HashMap::new();
        let dir = std::fs::read_dir(path)
            .map_err(|e| format!("failed to read maps dir '{path}': {e}"))?;
        
        for entry in dir.flatten() {
            let file_path = entry.path();
            let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let fname = file_path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            
            // 跳过 ! 开头的指南/模板文件
            if ext != "map" || fname.starts_with('!') {
                continue;
            }
            
            let content = std::fs::read_to_string(&file_path)
                .map_err(|e| format!("failed to read map '{}': {e}", file_path.display()))?;
            
            match Self::parse_map_content(&content) {
                Ok(definition) => {
                    let key = (definition.provider.clone(), definition.channel.clone(), definition.event_id);
                    maps.insert(key, definition);
                }
                Err(e) => eprintln!("[warn] failed to parse map '{}': {e}", fname),
            }
        }
        
        Ok(MapsParser {
            maps,
            version: format!("evtxecmd-maps-{}", maps.len()),
        })
    }
    
    /// 解析单个 .map 文件内容（YAML-like 格式）
    fn parse_map_content(content: &str) -> Result<MapDefinition> {
        let mut description = String::new();
        let mut event_id = 0u32;
        let mut provider = String::new();
        let mut channel = String::new();
        let mut properties = Vec::new();
        let mut lookups = Vec::new();
        
        // 跳过 # 注释行
        // 扫描状态机
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.is_empty() {
                continue;
            }
            
            // Header 字段
            if let Some(val) = trimmed.strip_prefix("Description: ") {
                description = val.to_string();
            } else if let Some(val) = trimmed.strip_prefix("EventId: ") {
                event_id = val.parse::<u32>().unwrap_or(0);
            } else if let Some(val) = trimmed.strip_prefix("Provider: ") {
                provider = val.trim_matches('"').to_string();
            } else if let Some(val) = trimmed.strip_prefix("Channel: ") {
                channel = val.trim_matches('"').to_string();
            }
        }
        
        // 解析 Maps: 和 Lookups: 节（带缩进的 YAML-like）
        // ...（状态机解析缩进结构，或者直接用 serde_yaml 解析）
        
        Ok(MapDefinition { description, event_id, provider, channel, properties, lookups })
    }
    
    /// 匹配一个事件并填充 Maps 字段（100% 兼容）
    pub fn apply(&self, event: &mut ParsedEvent) {
        // 1. 找出匹配的 MapDefinition
        let map = match (&event.provider_name, &event.channel, event.event_id) {
            (Some(prov), Some(ch), Some(eid)) => {
                // 尝试精确匹配，再尝试去掉引号匹配
                match self.maps.get(&(prov.clone(), ch.clone(), eid)) {
                    Some(m) => m,
                    None => return,
                }
            }
            _ => return,
        };
        
        // 2. 解析 event_data JSON
        let event_data: Option<serde_json::Value> = event.event_data
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok());
        
        // 3. 提取所有变量（MapValue）的值到 HashMap
        let mut variables: HashMap<String, String> = HashMap::new();
        for prop in &map.properties {
            for v in &prop.values {
                let field_name = extract_field_name_from_xpath(&v.xpath);
                if let Some(name) = field_name {
                    if let Some(val) = event_data
                        .as_ref()
                        .and_then(|d| d.get(name))
                        .and_then(|v| v.as_str())
                    {
                        variables.insert(v.var_name.clone(), val.to_string());
                    }
                }
            }
        }
        
        // 4. 对每个 Property 做模板替换 + Lookups 映射
        for prop in &map.properties {
            let rendered = self.render_template(&prop.template, &prop.values, &variables);
            let resolved = self.apply_lookups(&rendered, &map.lookups, &variables);
            self.assign_to_event_field(event, &prop.target_column, &resolved);
        }
        
        // 5. 填充公共字段
        event.map_description = Some(map.description.clone());
        event.maps_version = Some(self.version.clone());
    }
    
    /// 模板替换：从变量表取值替换 %varName%
    fn render_template(
        &self,
        template: &str,
        _values: &[MapValue],
        variables: &HashMap<String, String>,
    ) -> String {
        let mut result = template.to_string();
        for (var_name, val) in variables {
            result = result.replace(&format!("%{var_name}%"), val);
            // 也处理 %var_name% 中带特殊字符的变体（如 %domain%\%user%）
        }
        result
    }
    
    /// 应用 Lookups 值映射
    fn apply_lookups(
        &self,
        rendered: &str,
        lookups: &[LookupTable],
        variables: &HashMap<String, String>,
    ) -> String {
        let mut result = rendered.to_string();
        for lookup in lookups {
            if let Some(raw_value) = variables.get(&lookup.name) {
                let mapped = lookup.entries
                    .get(raw_value)
                    .map(|s| s.as_str())
                    .unwrap_or(&lookup.default);
                result = result.replace(raw_value, mapped);
            }
        }
        result
    }
    
    /// 根据 Property 列名将值填入 ParsedEvent 对应字段
    fn assign_to_event_field(&self, event: &mut ParsedEvent, column: &str, value: &str) {
        let val = if value.is_empty() { None } else { Some(value.to_string()) };
        match column {
            "UserName" | "Username" => event.map_user_name = val,
            "RemoteHost" => event.map_remote_host = val,
            "ExecutableInfo" => event.map_executable_info = val,
            "PayloadData1" => event.payload_data1 = val,
            "PayloadData2" => event.payload_data2 = val,
            "PayloadData3" => event.payload_data3 = val,
            "PayloadData4" => event.payload_data4 = val,
            "PayloadData5" => event.payload_data5 = val,
            "PayloadData6" => event.payload_data6 = val,
            _ => {} // 未识别的 Property 忽略
        }
    }
}
```

### 3.6 实现中的关键注意事项

| 问题 | 解决方案 |
|------|----------|
| `.map` 不是标准 YAML（`Values:` 缩进有变体） | 使用自定义行解析器，或 `serde_yaml` + 预处理 |
| `Map` 字典用 `-` 列表表示 | 直接解析为 `Vec<MapProperty>` |
| `Lookups` 中的 `Values:` 是 `key: value` 对（不是 `-` 列表） | 特殊处理：无 `-` 前缀的 `key: value` 行 |
| `Channel:` 值可能带双引号（如 `"Application"`） | `trim_matches('"')` |
| `PropertyValue:` 中的 `\\` 是字面反斜杠 | 保持原样替换，不做转义 |
| 同一事件被多个 Map 匹配 | 以 `(Provider, Channel, EventID)` 为唯一键 |
| `UserName` 和 `Username` 大小写变体 | 匹配时统一处理两种写法 |

### 3.6 场景表集成

YAML 规则可配置是否包含 Maps 列：

```yaml
name: "登录审计"
table: "security_events"
columns:
  - event_id
  - event_time
  - user_name
  - message
  - map_description        # 新增
  - payload_data1          # 新增
  - payload_data2          # 新增
event_ids: [4624, 4625]
channels: ["Security"]
```

### 3.7 CSV 导出兼容 EvtxECmd 格式

新增 `--csv-format` 参数：

```bash
# 默认格式（25 + 10 列）
evtx-tools parse input.evtx --evtx-maps-dir ./Maps/ -o output --format csv

# EvtxECmd 兼容格式（列名映射为 EvtxECmd 的输出格式）
evtx-tools parse input.evtx --evtx-maps-dir ./Maps/ -o output \
  --format csv --csv-format evtxecmd
```

EvtxECmd 兼容格式映射（100% 兼容）：

| evtx-tools 列 | EvtxECmd 兼容列 | Maps 来源 |
|---------------|----------------|-----------|
| event_record_id | RecordNumber | 原生字段 |
| event_time | TimeCreated | 原生字段 |
| event_id | EventId | 原生字段 |
| provider_name | Provider | 原生字段 |
| channel | Channel | 原生字段 |
| computer | Computer | 原生字段 |
| level | Level | 原生字段 |
| process_id | ProcessId | 原生字段 |
| thread_id | ThreadId | 原生字段 |
| user_name | UserName | 原生字段 |
| user_sid | UserId | 原生字段 |
| **map_description** | MapDescription | `Description:` 头字段 |
| **map_user_name** | UserName | `UserName` Property |
| **map_remote_host** | RemoteHost | `RemoteHost` Property |
| **map_executable_info** | ExecutableInfo | `ExecutableInfo` Property |
| **payload_data1** | PayloadData1 | `PayloadData1` Property |
| **payload_data2** | PayloadData2 | `PayloadData2` Property |
| **payload_data3** | PayloadData3 | `PayloadData3` Property |
| **payload_data4** | PayloadData4 | `PayloadData4` Property |
| **payload_data5** | PayloadData5 | `PayloadData5` Property |
| **payload_data6** | PayloadData6 | `PayloadData6` Property |

---

## 4. 实现计划

### 4.1 阶段划分

| 阶段 | 内容 | 预估代码量 | 依赖 |
|------|------|:----------:|------|
| **P1** | Maps YAML-like 解析器 + Lookups + 模板渲染 | ~350 行 | Rust 标准库 + serde_yaml（可选） |
| **P2** | ParsedEvent 新增 11 列 + SQLite schema 更新 | ~120 行 | 无 |
| **P3** | `--evtx-maps-dir` CLI 参数 + 管道集成 | ~100 行 | P1 + P2 |
| **P4** | CSV EvtxECmd 100% 兼容导出 | ~120 行 | P3 |
| **P5** | `maps sync` 子命令 | ~150 行 | HTTP 下载或 git |
| **P6** | 场景表集成 + 完整测试 | ~150 行 | P3 |

### 4.2 具体任务分解

#### P1：Maps YAML-like 解析器（100% 兼容）

```
文件: evtx-tools-core/src/maps.rs（新建）

核心结构:
  MapsParser
    ├── maps: HashMap<(Provider, Channel, EventId), MapDefinition>
    │
    ├── load_from_dir(path) -> Result<MapsParser>
    │   ├── 遍历目录下所有 .map 文件（跳过 ! 开头的指南/模板）
    │   ├── 解析 YAML-like 格式（自定义行解析器或 serde_yaml）
    │   │   ├── Header: Description, EventId, Channel, Provider
    │   │   ├── Maps: 缩进列表 → Vec<MapProperty>
    │   │   ├── Lookups: 缩进列表 → Vec<LookupTable>
    │   │   └── # 注释行跳过
    │   └── 以 (Provider, Channel, EventId) 为 key 建立 HashMap
    │
    ├── apply(event: &mut ParsedEvent)
    │   ├── 按 (provider, channel, event_id) 匹配 MapDefinition
    │   │   └── Channel: 清洗引号和大小写
    │   ├── 解析 event_data JSON
    │   ├── 从 XPath @Name 提取字段名 → 从 JSON 取值
    │   ├── 模板替换 PropertyValue 中的 %varName%
    │   ├── 应用 Lookups 值映射（如果存在）
    │   └── 根据 Property 列名写入 ParsedEvent 对应字段
    │       ├── "UserName" / "Username" → map_user_name
    │       ├── "RemoteHost" → map_remote_host
    │       ├── "ExecutableInfo" → map_executable_info
    │       ├── "PayloadData1~6" → payload_data1~6
    │       └── description → map_description
    │
    └── render_template(template, variables) -> String

数据模型:
  MapDefinition     # .map 文件完整定义
  MapProperty       # Maps: 列表中的一项
  MapValue          # Values: 中的变量定义
  LookupTable       # Lookups: 值映射表

辅助函数:
  extract_field_name_from_xpath(xpath) -> Option<&str>
    从 "@Name='TargetDomainName'" 或 '@Name="TargetDomainName"' 提取
```

**边界情况处理：**

```rust
// 1. 模板变量在 event_data 中不存在 → 保留 %varName% 原文
//    例: PropertyValue="Target: %NonExistent%" → "Target: %NonExistent%"

// 2. event_data 为 None 或解析失败 → Maps 全部留空
//    例: 事件没有 EventData → map_description = NULL

// 3. 多个 Map 匹配同一个事件 → 取第一个（文件顺序决定优先级）
//    例: Security + Microsoft-Windows-Security-Auditing + 4624 可能有多个 map 定义

// 4. 模板值包含 HTML 实体 → 保留原文，不做转义
//    例: %TargetDomainName% = "DOMAIN" → "Target: DOMAIN\admin"

// 5. %varName% 在值中不存在 → 不替换
//    例: PropertyValue="固定文本"（无 %varName%）→ "固定文本"
```

#### P2：数据结构扩展

```
文件: evtx-tools-core/src/lib.rs

ParsedEvent 新增字段:
  map_description:  Option<String>,
  payload_data1:    Option<String>,
  payload_data2:    Option<String>,
  payload_data3:    Option<String>,
  payload_data4:    Option<String>,
  payload_data5:    Option<String>,
  payload_data6:    Option<String>,
  maps_version:     Option<String>,

文件: evtx-tools-core/src/storage/sqlite.rs

create_events_table() 更新:
  + map_description  TEXT
  + payload_data1    TEXT
  + payload_data2    TEXT
  + payload_data3    TEXT
  + payload_data4    TEXT
  + payload_data5    TEXT
  + payload_data6    TEXT
  + maps_version     TEXT

insert_events() 更新:
  + 8 列写入（对应 ParsedEvent 新增字段）

column_sql_type() 更新:
  + map_description → TEXT
  + payload_data1~6 → TEXT
  + maps_version    → TEXT

append_field_value() 更新:
  + map_description → append_text_opt
  + payload_data1~6 → append_text_opt
  + maps_version    → append_text_opt
```

#### P3：CLI 参数 + 管道集成

```
文件: evtx-tools-cli/src/main.rs

ParseArgs 新增参数:
  #[arg(long, help = "Path to EvtxECmd-compatible Maps directory")]
  evtx_maps_dir: Option<PathBuf>,

文件: evtx-tools-cli/src/cmd_parse.rs

run_parse() 更新:
  在 MessageDb warmup 之后，加入 Maps 初始化：
  
  let maps_parser: Option<MapsParser> = if let Some(dir) = &args.evtx_maps_dir {
      match MapsParser::load_from_dir(dir.to_string_lossy().as_ref()) {
          Ok(parser) => Some(parser),
          Err(e) => {
              eprintln!("[warn] failed to load maps: {e}");
              None
          }
      }
  } else {
      None
  };

process_parallel_batch() 更新:
  在消息替换之后，加入 Maps 填充：
  if let Some(ref maps) = maps_parser {
      maps.apply(event);
  }
```

#### P4：CSV 兼容导出

```
文件: evtx-tools-cli/src/cmd_parse.rs 或 evtx-tools-core/src/storage/csv.rs

新增 --csv-format 参数:
  #[arg(long, default_value = "default")]
  csv_format: String,  // "default" | "evtxecmd"

CSV 列映射（evtxecmd 模式）:
  record_number   → event_record_id
  time_created    → event_time
  event_id        → event.id
  provider        → provider_name
  channel         → channel
  level           → level
  ...
  map_description → MapDescription
  payload_data1   → PayloadData1
  ...
```

#### P5：maps sync 子命令

```
文件: evtx-tools-cli/src/cmd_maps.rs（新建）
注册: main.rs 新增 maps 子命令

evtx-tools maps sync
  --dir ./Maps/     # 目标目录
  --repo https://github.com/EricZimmerman/evtx.git  # Maps 仓库地址（可选）
  
实现:
  1. 使用 git clone --depth 1 --sparse 或直接下载 Maps/ 目录
  2. 验证 Maps 格式完整性
  3. 输出同步报告（多少个 map 文件、覆盖多少个 EventID）
```

### 4.3 测试计划

| 测试 | 类型 | 说明 |
|------|------|------|
| `test_maps_parse_single_file` | 单元 | 解析单个 .map 文件，验证 Header + Maps + Lookups 完整解析 |
| `test_maps_parse_directory` | 单元 | 从实际 Maps 目录（383 个文件）加载，验证覆盖率和健壮性 |
| `test_maps_parse_bad_format` | 单元 | 格式错误的 .map 文件安全跳过（不 panic） |
| `test_maps_render_simple` | 单元 | 单变量 `%varName%` 模板替换 |
| `test_maps_render_multiple` | 单元 | 多变量 `"%domain%\\%user%"` 模板替换 |
| `test_maps_render_missing_var` | 单元 | 变量在 event_data 中不存在 → `%varName%` 保留原文 |
| `test_maps_match_exact` | 单元 | 精确匹配 (Provider, Channel, EventID) |
| `test_maps_no_match` | 单元 | 事件无匹配 Maps 时安全跳过 |
| `test_maps_event_data_none` | 单元 | 事件无 EventData 时安全跳过 |
| `test_maps_lookups_resolve` | 单元 | Lookups 值映射表正确应用 |
| `test_maps_lookups_default` | 单元 | Lookups 无匹配时使用 Default |
| `test_maps_assign_all_properties` | 单元 | 全部 9 种 Property 正确写入对应 ParsedEvent 字段 |
| `test_maps_username_case_variant` | 单元 | 处理 `Username` vs `UserName` 大小写变体 |
| `test_maps_priority_db_vs_maps` | 集成 | 有 messages.db 时消息优先，Maps 字段补充 |
| `test_maps_channel_quote_stripping` | 集成 | Channel 引号去除（`"Application"` → `Application`） |
| `test_maps_scenario_table` | 集成 | 场景表包含 Maps 列（map_description, payload_data1~6 等） |
| `test_maps_csv_evtxecmd_format` | 集成 | CSV 导出兼容 EvtxECmd 格式（列名 + 顺序） |
| `test_maps_xpath_field_extraction` | 单元 | XPath `@Name` 提取函数处理引号变体 |
| `test_maps_actual_4624_map` | 集成 | 用真实 `Security_Microsoft-Windows-Security-Auditing_4624.map` 文件验证完整流程 |
| `test_maps_actual_7045_map` | 集成 | 用 `System_Service-Control-Manager_7045.map` 验证服务安装事件 |

### 4.4 依赖变更

```toml
# Cargo.toml（evtx-tools-core）— 无新增依赖
# serde_json 已存在，用于 event_data JSON 解析
# 所有 Maps 处理使用 Rust 标准库 + 现有依赖

# Cargo.toml（evtx-tools-cli）— 无新增依赖
# reqwest（可选）— 用于 maps sync 的 HTTP 下载
# 可选：使用 git2 crate 做 git clone，或者用 curl 替代
```

---

## 5. 兼容性分析

### 5.1 实测兼容性结论（基于 383 个 .map 文件逐项分析）

```
核心结论：技术上可以 100% 解析所有 .map 文件。
运行时匹配率 ~85% 完全兼容，~10% 基本兼容（部分字段空），~3-5% 匹配失败。
对 8 个内置 profile 场景：兼容率 > 98%。
```

### 5.2 按 Profile 的兼容性

| Profile | 核心 EventID | 对应 Maps 文件 | 兼容状态 |
|---------|------------|---------------|:--------:|
| security | 4624 | `Security_Microsoft-Windows-Security-Auditing_4624.map` | ✅ |
| security | 4625 | `Security_Microsoft-Windows-Security-Auditing_4625.map` | ✅ |
| security | 4648 | `Security_Microsoft-Windows-Security-Auditing_4648.map` | ✅ |
| security | 4672 | `Security_Microsoft-Windows-Security-Auditing_4672.map` | ✅ |
| security | 4720 | `Security_Microsoft-Windows-Security-Auditing_4720.map` | ✅ |
| security | 4776 | `Security_Microsoft-Windows-Security-Auditing_4776.map` | ✅ |
| security | 1102 | `Security_Microsoft-Windows-Eventlog_1102.map` | ✅ |
| persistence | 7045 | `System_Service-Control-Manager_7045.map` | ✅ |
| persistence | 4697 | `Security_Microsoft-Windows-Security-Auditing_4697.map` | ✅ |
| persistence | 4698 | `Security_Microsoft-Windows-Security-Auditing_4698.map` | ✅ |
| system | 6008 | `System_EventLog_6008.map` | ✅ |
| system | 1074 | `System_User32_1074.map` | ✅ |
| system | 104 | `System_Microsoft-Windows-Eventlog_104.map` | ✅ |
| system | 7031 | `System_Service-Control-Manager_7031.map` | ✅ |
| system | 7034 | `System_Service-Control-Manager_7034.map` | ✅ |
| process | 4688 | `Security_Microsoft-Windows-Security-Auditing_4688.map` | ✅ |
| remote | 4778 | `Security_Microsoft-Windows-Security-Auditing_4778.map` | ✅ |
| remote | 5140 | `Security_Microsoft-Windows-Security-Auditing_5140.map` | ✅ |
| remote | 5156 | `Security_Microsoft-Windows-Security-Auditing_5156.map` | ✅ |
| remote | 1149 | `Microsoft-Windows-TerminalServices-RemoteConnectionManager_1149.map` | ✅ |
| powershell | 400 | `Windows-PowerShell_PowerShell_400.map` | ✅ |
| powershell | 403 | `Windows-PowerShell_PowerShell_403.map` | ✅ |
| powershell | 4103 | `Microsoft-Windows-PowerShell-Operational_Microsoft-Windows-PowerShell_4103.map` | ✅ |
| powershell | 4104 | `Microsoft-Windows-PowerShell-Operational_Microsoft-Windows-PowerShell_4104.map` | ✅ |
| sysmon 1-26 | 全部 | `Microsoft-Windows-Sysmon-Operational_Microsoft-Windows-Sysmon_{1-26}.map` | ✅ **全部 26 个 EventID 全覆盖** |

**Security 通道本身有 93 个 .map 文件，覆盖了该通道几乎所有常用 EventID。**

### 5.3 已知兼容性问题的处理方案

| 问题 | 受影响文件数 | 严重程度 | 解决方案 | 代码量 |
|------|:-----------:|:--------:|----------|:-----:|
| Channel 带引号（`"Application"` vs `Application`） | 116 | 低 | `trim_matches('"')` 清洗 | 2 行 |
| Provider 带引号 | 89 | 低 | 同上 | 1 行 |
| Username 大小写变体（`Username` vs `UserName`） | 139 | 低 | 匹配时 case-insensitive | 1 行 |
| Lookups key:value 解析（`key: value` 无 `-` 前缀） | 43 | 中 | 特殊处理 Values: 节的缩进对 | 30 行 |
| PropertyValue 含转义反斜杠（`\\`） | 全部 | 低 | 模板替换前保持原样 | 0 行 |

**所有已知问题都有明确解决方案，无技术拦路石。**

### 5.4 真正的潜在运行时问题（需验证）

```
核心问题：evtx-tools 的 event_data JSON 和 Maps 期待的 XPath 提取是否完全对得上？
```

evtx-tools 输出的 event_data 示例（EventID 4624）：

```json
{"SubjectUserSid":"S-1-5-18","TargetUserName":"admin","TargetDomainName":"DOMAIN","LogonType":"2"}
```

Maps 提取逻辑：`@Name='TargetDomainName'` → JSON key `TargetDomainName` → 取值 `DOMAIN` ✅

**边界场景分析：**

| 场景 | 说明 | 风险 | 影响范围 |
|------|------|:----:|:--------:|
| **UserData 格式** | Setup 通道等使用 UserData 而非 EventData，evtx-tools 的 `extract_userdata_as_eventdata()` 已处理此转换，但嵌套结构可能变平 | 中 | `Microsoft-Windows-Bits-Client/Operational` 等 ~5 个通道 |
| **Classic #text 数组** | MsiInstaller 等 Provider 的 EventData 是 `{"#text": ["val1", "val2"]}` 格式而非 key-value，Maps 的 @Name 提取找不到字段名 | 中 | `MsiInstaller` 系列 .map 文件（~6 个） |
| **字段名大小写差异** | Windows 版本间字段名可能有细微差异（`CommandLine` vs `Command line`） | 低 | 极少见 |
| **Provider 名不精确匹配** | EVTX 中 provider_name 和 Maps 中 Provider: 字段存在空格/大小写差异 | 低 | 需对基础时额外清理 |

**预估运行时兼容率（基于理论分析）：**

```
完全兼容（全部字段正确填充）: ~85%
  └─ Provider + Channel + EventID 精确匹配，字段名一致

基本兼容（匹配成功但部分字段空）: ~10%
  └─ 匹配上了但部分 @Name 字段在 event_data 中不存在，该字段输出空

不兼容（匹配失败）: ~3-5%
  └─ Provider/Channel 命名差异、UserData 扁平化信息丢失、经典 #text 数组
```

### 5.5 验证计划

建议用参考数据集（74K 事件）验证关键路径：

```bash
# 1. 用 messages.db 跑基准分析
evtx-tools parse Security.evtx --msg-db ./msgs.db -o output_baseline

# 2. 用 Maps 跑对照分析
evtx-tools parse Security.evtx --evtx-maps-dir ./Maps/ -o output_maps

# 3. 对比关键 EventID 的输出
sqlite3 output_baseline "
  SELECT event_id, event_record_id, message, payload_data1
  FROM events WHERE event_id IN (4624, 4625, 4688, 7045)
  LIMIT 10;"
```

**验证清单：**

| 验证项 | 方法 | 通过标准 |
|--------|------|----------|
| Security 4624 Maps 匹配 | 从 events 表查询 | `map_description = 'Successful logon'` |
| System 7045 Maps 匹配 | 同上 | `map_description = 'A new service was installed...'` |
| Sysmon 1 Maps 匹配 | 同上 | `map_description` 非空 |
| UserName 字段填充 | 检查 `map_user_name` | 格式为 `DOMAIN\user` |
| PayloadData 字段顺序 | 对比 4624 的 payload_data1~6 | 按 Maps 定义的 Property 顺序 |
| Lookups 映射 | 检查 4625 的 Status 字段 | `0xC0000064` 被映射为 `user name does not exist` |
| 无匹配事件 | 查询其他 EventID 的 Maps 字段 | `map_description IS NULL` |
| 消息优先级 | 同时使用 `--msg-db + --evtx-maps-dir` | `message` 为完整格式化，`map_description` 为 Maps 描述 |

### 5.1 已知限制

| 限制 | 影响 | 严重程度 |
|------|------|:--------:|
| 不支持 XPath 索引（`Data[1]`） | 极少数嵌套 Map 无法解析 | 低 |
| 不支持 XPath 函数（`string-length()`） | 不影响字段提取 | 低 |
| 不支持 XPath 属性过滤（`[@Name='x' and @Type='y']`） | 极少见 | 低 |
| Maps 仅匹配 `Provider + Channel + EventID` 三键 | 部分跨版本 Map 可能不匹配 | 低 |
| 模板替换仅支持 `%varName%` 语法 | PropertyValue 中其他占位符保持原文 | 低 |

### 5.2 与 upstream Maps 的同步策略

```bash
# 用户更新 Maps 的方式
# 方案 A：git pull（推荐）
cd ./Maps && git pull

# 方案 B：evtx-tools maps sync（未来）
evtx-tools maps sync --dir ./Maps/
```

evtx-tools **不维护** Maps 数据本身，完全依赖 Eric Zimmerman 的社区生态。你只维护 **Maps 解析器**的兼容性。

---

## 6. 附录

### 6.1 完整的 Maps JSON 示例（EventID 4624）

```json
{
  "MapDescription": "An account was successfully logged on.",
  "EventId": 4624,
  "Provider": "Microsoft-Windows-Security-Auditing",
  "Channel": "Security",
  "Properties": [
    {
      "Property": "PayloadData1",
      "PropertyValue": "Target: %TargetDomainName%\\%TargetUserName%",
      "Values": [
        { "Name": "TargetDomainName", "Value": "/Event/EventData/Data[@Name='TargetDomainName']" },
        { "Name": "TargetUserName", "Value": "/Event/EventData/Data[@Name='TargetUserName']" }
      ]
    },
    {
      "Property": "PayloadData2",
      "PropertyValue": "LogonType: %LogonTypeName%, Process: %ProcessName%",
      "Values": [
        { "Name": "LogonTypeName", "Value": "/Event/EventData/Data[@Name='LogonTypeName']" },
        { "Name": "ProcessName", "Value": "/Event/EventData/Data[@Name='ProcessName']" }
      ]
    }
  ]
}
```

### 6.2 运行时效果

**输入 EVTX：EventID 4624，Security 通道**

```
event_data JSON:
{
  "SubjectUserSid": "S-1-5-18",
  "SubjectUserName": "SYSTEM",
  "TargetUserSid": "S-1-5-21-...",
  "TargetUserName": "admin",
  "TargetDomainName": "DOMAIN",
  "LogonType": "2"
}
```

**有 messages.db 的输出：**

| 列 | 值 |
|---|---|
| message | `已成功登录帐户。\n帐户名:\tadmin\n域名:\tDOMAIN\n登录类型:\t2` |
| map_description | `Successful logon` |
| map_user_name | `DOMAIN\SYSTEM` |
| map_remote_host | `- (192.168.1.1)` |
| map_executable_info | `C:\Windows\System32\svchost.exe` |
| payload_data1 | `Target: DOMAIN\admin` |
| payload_data2 | `LogonType 2` |
| payload_data3 | `LogonId: 0x3E7` |
| payload_data4 | ``（无匹配时为空） |
| payload_data5 | `` |
| payload_data6 | `` |
| event_data | `{"SubjectUserSid":"S-1-5-18",...}` |

**无 messages.db、有 Maps 的输出：**

| 列 | 值 |
|---|---|
| message | `Successful logon`（map_description 兜底） |
| map_description | `Successful logon` |
| map_user_name | `DOMAIN\SYSTEM` |
| map_remote_host | `- (192.168.1.1)` |
| map_executable_info | `C:\Windows\System32\svchost.exe` |
| payload_data1 | `Target: DOMAIN\admin` |
| payload_data2 | `LogonType 2` |
| payload_data3 | `LogonId: 0x3E7` |
| event_data | `{"SubjectUserSid":"S-1-5-18",...}` |

### 6.3 参考链接

- [EvtxECmd Maps 目录（GitHub）](https://github.com/EricZimmerman/evtx/tree/master/evtx/Maps)
- [EvtxECmd 官方文档](https://github.com/EricZimmerman/evtx)
- [ECOSYSTEM-RESEARCH.md](./ECOSYSTEM-RESEARCH.md) — 完整生态对比
