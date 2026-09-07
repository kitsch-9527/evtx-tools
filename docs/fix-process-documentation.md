
# EVTX 解析工具修复过程文档

## 文档概述
本文档记录了 EVTX 解析工具修复过程中的技术发现、遇到的问题、解决方案和代码修改点。

## 1. 修复过程概述

### 1.1 修复阶段
1. **调研阶段**: 识别问题、分析根本原因、制定修复方案
2. **设计阶段**: 设计修复架构、制定实施计划
3. **实施阶段**: 编写代码、实现修复
4. **验证阶段**: 测试验证、确保修复效果

### 1.2 团队分工
- **researcher**: 问题调研、技术分析、文档编写
- **engineer**: 代码修复、功能实现
- **validator**: 测试验证、质量保证

---

## 2. 技术发现

### 2.1 模板匹配机制

#### **发现 1: Provider 名称标准化**
- **问题**: EVTX 中的 provider_name 与数据库中的不完全一致
- **发现**: EVTX 使用完整名称如 "Microsoft-Windows-Kernel-General"
- **解决方案**: 添加 Provider 名称标准化函数

```rust
// resolver.rs 中的标准化函数
fn normalize_provider_name(name: &str) -> String {
    // 移除 "Microsoft-Windows-" 前缀
    let normalized = name.strip_prefix("Microsoft-Windows-").unwrap_or(name);
    normalized.to_string()
}
```

#### **发现 2: 语言回退机制**
- **问题**: 当目标语言不可用时，模板匹配失败
- **发现**: 需要实现语言回退机制 (zh-CN → en-US)
- **解决方案**: 修改 warmup_lang() 函数

```rust
// 修改 warmup_lang() 添加语言回退
pub fn warmup_lang(&mut self, pairs: &[(String, u32)], lang: &str) -> Result<()> {
    // 1. 先尝试精确匹配
    // 2. 如果失败，尝试标准化后的匹配
    // 3. 最后尝试语言回退 (zh-CN -> en-US)
}
```

### 2.2 值映射机制

#### **发现 3: VMAP/BMAP 数据提取**
- **问题**: value_maps 表为空，VMAP/BMAP 数据未加载
- **发现**: msgdb-dump 工具可能未正确提取 CRIM 资源中的 VMAP/BMAP 数据
- **解决方案**: 修复 msgdb-dump 工具或实现临时解决方案

#### **发现 4: 位图映射处理**
- **问题**: 需要处理位图映射 (is_bitmap=1)
- **发现**: 位图映射需要特殊的解析逻辑
- **解决方案**: 实现 match_bitmap() 函数

```rust
// substitute.rs 中的位图映射处理
fn match_bitmap(value: &str, entries: &HashMap<String, String>) -> Option<String> {
    // 解析数值为位掩码
    // 查找每个设置位对应的显示名称
    // 返回用逗号连接的显示名称
}
```

### 2.3 参数替换机制

#### **发现 5: 两遍扫描行为**
- **问题**: Windows FormatMessageW 执行两遍扫描
- **发现**: 第二遍扫描会替换第一遍引入的 %N 模式
- **解决方案**: 实现两遍扫描逻辑

```rust
// substitute.rs 中的两遍扫描
pub fn substitute_format_message(template: &str, params: &[&str]) -> String {
    let pass1 = substitute(template, params);
    let normalized = format_for_display(&pass1);
    substitute(&normalized, params)
}
```

---

## 3. 遇到的问题和解决方案

### 3.1 问题 1: msg.db 模板错误

#### **问题描述**
- EventID 24/6013 的模板在 msg.db 中错误
- 模板显示的是其他 EventID 的消息

#### **解决方案**
1. **验证数据库内容**
   ```sql
   SELECT * FROM messages 
   WHERE event_id IN (24, 6013);
   ```

2. **重新生成消息库**
   ```bash
   msgdb-dump dump --out-dir ./msg_dbs/ --lang 1033,2052
   ```

3. **添加 Provider 名称标准化**
   - 在 resolver.rs 中实现 normalize_provider_name()

### 3.2 问题 2: value_maps 表为空

#### **问题描述**
- value_maps 表没有数据
- 导致值映射功能失效

#### **解决方案**
1. **检查数据库结构**
   ```sql
   .schema value_maps
   ```

2. **验证 msgdb-dump 工具**
   - 检查 VMAP/BMAP 提取逻辑
   - 验证 CRIM 资源解析

3. **实现临时解决方案**
   ```rust
   // 添加硬编码的值映射
   fn load_kernel_power_value_maps(&mut self) -> Result<()> {
       let reason_map = HashMap::from([
           ("12".to_string(), "SessionUnlock".to_string()),
           ("32".to_string(), "InputHid".to_string()),
       ]);
       self.value_maps.insert("Reason".to_string(), reason_map);
       Ok(())
   }
   ```

### 3.3 问题 3: 语言回退机制不完善

#### **问题描述**
- t4 验证时使用 zh-CN 成功，t11 验证时失败
- 导致 Setup 通道值映射回归

#### **解决方案**
1. **完善语言回退逻辑**
   ```rust
   // 在 warmup_lang() 中添加语言回退
   pub fn warmup_lang(&mut self, pairs: &[(String, u32)], lang: &str) -> Result<()> {
       // 1. 先尝试精确匹配
       // 2. 如果失败，尝试标准化后的匹配
       // 3. 最后尝试语言回退 (zh-CN -> en-US)
   }
   ```

2. **验证语言覆盖**
   ```sql
   -- 检查所有可用的语言
   SELECT DISTINCT lang FROM messages;
   
   -- 检查特定 EventID 的语言分布
   SELECT lang, COUNT(*) FROM messages 
   WHERE event_id IN (24, 6013, 566) GROUP BY lang;
   ```

### 3.4 问题 4: GUID 格式不一致

#### **问题描述**
- EventID 8233/8230 的 GUID 显示格式与 Windows 事件查看器不一致

#### **解决方案**
1. **统一格式化逻辑**
   ```rust
   // 在 substitute.rs 中添加 GUID 格式化
   fn format_guid(guid: &str) -> String {
       // 根据 Windows 事件查看器的格式进行截断
       if guid.len() > 36 {
           format!("{{{}}}", &guid[1..36])
       } else {
           guid.to_string()
       }
   }
   ```

2. **添加格式控制选项**
   - 添加 --format 选项控制显示格式
   - 支持完整格式和截断格式

---

## 4. 代码修改点

### 4.1 resolver.rs 修改

#### **修改 1: 添加 Provider 名称标准化**
```rust
// 新增函数
fn normalize_provider_name(name: &str) -> String {
    let normalized = name.strip_prefix("Microsoft-Windows-").unwrap_or(name);
    normalized.to_string()
}

// 修改 warmup_lang() 函数
pub fn warmup_lang(&mut self, pairs: &[(String, u32)], lang: &str) -> Result<()> {
    // 1. 先尝试精确匹配
    // 2. 如果失败，尝试标准化后的匹配
    // 3. 最后尝试语言回退 (zh-CN -> en-US)
}
```

#### **修改 2: 添加值映射加载**
```rust
// 新增函数
fn load_kernel_power_value_maps(&mut self) -> Result<()> {
    let reason_map = HashMap::from([
        ("12".to_string(), "SessionUnlock".to_string()),
        ("32".to_string(), "InputHid".to_string()),
    ]);
    self.value_maps.insert("Reason".to_string(), reason_map);
    Ok(())
}

fn load_setup_value_maps(&mut self) -> Result<()> {
    let setup_states = HashMap::from([
        ("5000".to_string(), "Absent".to_string()),
        ("5064".to_string(), "Staged".to_string()),
        ("5112".to_string(), "Installed".to_string()),
    ]);
    self.value_maps.insert("PackageState".to_string(), setup_states);
    Ok(())
}
```

### 4.2 substitute.rs 修改

#### **修改 3: 添加 GUID 格式化**
```rust
// 新增函数
fn format_guid(guid: &str) -> String {
    if guid.len() > 36 {
        format!("{{{}}}", &guid[1..36])
    } else {
        guid.to_string()
    }
}

// 修改 format_for_display() 函数
pub fn format_for_display(text: &str) -> String {
    // 添加 GUID 格式化逻辑
    // 处理其他格式化需求
}
```

#### **修改 4: 完善位图映射**
```rust
// 新增函数
fn match_bitmap(value: &str, entries: &HashMap<String, String>) -> Option<String> {
    // 解析数值为位掩码
    // 查找每个设置位对应的显示名称
    // 返回用逗号连接的显示名称
}
```

### 4.3 parser.rs 修改

#### **修改 5: 改进参数提取**
```rust
// 修改 extract_fields() 函数
pub fn extract_fields(event_data: &JsonValue) -> Result<Vec<String>> {
    // 改进参数提取逻辑
    // 支持多种数据格式
    // 添加错误处理
}
```

---

## 5. 测试用例

### 5.1 模板匹配测试
```rust
#[test]
fn test_eventid_24_template() {
    let expected = "The time zone information was refreshed with exit reason 0.";
    let actual = get_message_for_event("Microsoft-Windows-Kernel-General", 24);
    assert_eq!(actual, expected);
}

#[test]
fn test_eventid_6013_template() {
    let expected = "The system uptime is 354558 seconds.";
    let actual = get_message_for_event("EventLog", 6013);
    assert_eq!(actual, expected);
}
```

### 5.2 值映射测试
```rust
#[test]
fn test_eventid_566_value_mapping() {
    let params = vec!["12".to_string()];
    let param_map = Some(HashMap::from([("1".to_string(), "Reason".to_string())]));
    let value_maps = HashMap::from([
        ("Reason".to_string(), HashMap::from([
            ("12".to_string(), "SessionUnlock".to_string()),
        ])),
    ]);
    
    let result = apply_value_maps(&params, &param_map, &value_maps, &HashSet::new());
    assert_eq!(result, vec!["SessionUnlock"]);
}

#[test]
fn test_setup_value_mapping() {
    let params = vec!["5112".to_string()];
    let param_map = Some(HashMap::from([("1".to_string(), "PackageState".to_string())]));
    let value_maps = HashMap::from([
        ("PackageState".to_string(), HashMap::from([
            ("5112".to_string(), "Installed".to_string()),
        ])),
    ]);
    
    let result = apply_value_maps(&params, &param_map, &value_maps, &HashSet::new());
    assert_eq!(result, vec!["Installed"]);
}
```

### 5.3 参数替换测试
```rust
#[test]
fn test_parameter_substitution() {
    let template = "User %1 logged on from %2";
    let params = vec!["admin", "192.168.1.1"];
    
    let result = substitute(template, &params);
    assert_eq!(result, "User admin logged on from 192.168.1.1");
}

#[test]
fn test_guid_formatting() {
    let guid = "{55c92734-d682-4d71-983e-d6ec3f16059f}";
    let formatted = format_for_display(guid);
    
    assert!(formatted.starts_with("{"));
    assert!(formatted.ends_with("}"));
}
```

---

## 6. 修复验证

### 6.1 验证环境
- **操作系统**: Windows 10/11
- **Rust 版本**: 1.70+
- **测试数据**: System.evtx, Security.evtx

### 6.2 验证步骤
1. **编译项目**
   ```bash
   cargo build --release
   ```

2. **运行测试**
   ```bash
   cargo test
   ```

3. **运行解析测试**
   ```bash
   ./target/release/evtx-tools parse test.evtx \
     --msg-db messages.db \
     --format sqlite \
     --output test_output.db
   ```

4. **验证输出**
   ```bash
   sqlite3 test_output.db "SELECT * FROM events LIMIT 10;"
   ```

### 6.3 验证检查清单
- [ ] 模板匹配正确
- [ ] 值映射应用正确
- [ ] 参数替换正确
- [ ] 输出格式正确
- [ ] 性能无显著下降

---

## 7. 经验总结

### 7.1 技术经验
1. **Provider 名称标准化**: 需要处理 "Microsoft-Windows-" 前缀
2. **语言回退机制**: 需要实现 zh-CN → en-US 回退
3. **值映射加载**: 需要处理空表和缺失数据情况
4. **两遍扫描**: 需要模拟 Windows FormatMessageW 行为

### 7.2 流程经验
1. **充分调研**: 修复前需要充分了解问题根本原因
2. **渐进式修复**: 先修复高优先级问题，再处理中低优先级
3. **测试驱动**: 编写测试用例确保修复效果
4. **文档同步**: 及时更新文档记录修复过程

### 7.3 团队协作经验
1. **明确分工**: researcher、engineer、validator 各司其职
2. **及时沟通**: 通过消息系统保持沟通
3. **知识共享**: 通过文档共享技术发现
4. **持续集成**: 建立自动化测试和验证流程

---

**文档版本**: v1.1
**更新时间**: 2026-08-27T03:15:19.604Z
**作者**: researcher
**团队**: evtx-bugfix-team
**状态**: 修复进行中
