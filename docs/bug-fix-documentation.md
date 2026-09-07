
# EVTX 解析工具 Bug 修复文档

## 文档概述
本文档记录了 EVTX 解析工具中所有已识别的 Bug、根本原因、修复方案和验证方法。

## 1. Bug 列表和状态

### 1.1 Bug 1: EventID 24 (Microsoft-Windows-Kernel-General) 模板匹配错误

#### **问题描述**
- **现象**: 显示错误的模板消息，应显示时区刷新消息
- **预期输出**: "The time zone information was refreshed with exit reason 0. Current time zone bias is -480."
- **实际输出**: "CreateAppContainerProfile failed with error 0 because it was unable to create registry key -480."
- **影响**: 高 - 影响系统事件解析准确性

#### **当前状态**: 🔄 修复中
- **负责人**: engineer
- **开始时间**: 2026-08-27T03:13:59.946Z
- **预计完成**: 1-2天

#### **根本原因**
1. **msg.db 模板错误**: 消息数据库中 EventID 24 的模板条目错误
2. **Provider 名称不匹配**: EVTX 中的 provider_name "Microsoft-Windows-Kernel-General" 与数据库中的不完全一致
3. **语言代码不匹配**: 查询使用 lang="zh-CN"，但模板可能只有 en-US

#### **修复方案**
1. **验证并修复 msg.db**
   ```sql
   -- 检查 EventID 24 的模板
   SELECT * FROM messages 
   WHERE event_id = 24 
   AND provider_name LIKE '%Kernel-General%';
   ```

2. **改进模板匹配逻辑 (resolver.rs)**
   ```rust
   // 添加 Provider 名称标准化函数
   fn normalize_provider_name(name: &str) -> String {
       let normalized = name.strip_prefix("Microsoft-Windows-").unwrap_or(name);
       normalized.to_string()
   }
   
   // 修改 warmup_lang() 添加语言回退
   pub fn warmup_lang(&mut self, pairs: &[(String, u32)], lang: &str) -> Result<()> {
       // 1. 先尝试精确匹配
       // 2. 如果失败，尝试标准化后的匹配
       // 3. 最后尝试语言回退 (zh-CN -> en-US)
   }
   ```

3. **使用 msgdb-dump 重新生成消息库**
   ```bash
   msgdb-dump dump --out-dir ./msg_dbs/ --lang 1033,2052
   ```

#### **验证方法**
```rust
fn test_eventid_24_template() {
    let expected = "The time zone information was refreshed with exit reason 0. Current time zone bias is -480.";
    let actual = get_message_for_event("Microsoft-Windows-Kernel-General", 24);
    assert_eq!(actual, expected, "EventID 24 template mismatch");
}
```

---

### 1.2 Bug 2: EventID 6013 (EventLog) 模板匹配错误

#### **问题描述**
- **现象**: 显示关机消息，应显示系统运行时间
- **预期输出**: "The system uptime is 354558 seconds."
- **实际输出**: "Windows has shut down or restarted. The system was last started on ."
- **影响**: 高 - 影响系统事件解析准确性

#### **当前状态**: 🔄 修复中
- **负责人**: engineer
- **开始时间**: 2026-08-27T03:13:59.946Z
- **预计完成**: 1-2天

#### **根本原因**
1. **msg.db 模板错误**: 消息数据库中 EventID 6013 的模板条目错误
2. **Provider 名称不匹配**: EVTX 中的 provider_name "EventLog" 与数据库中的不完全一致
3. **模板对应错误**: 模板显示的是关机消息，而非系统运行时间

#### **修复方案**
1. **验证 msg.db 内容**
   ```sql
   -- 检查 EventID 6013 的模板
   SELECT * FROM messages 
   WHERE event_id = 6013 
   AND provider_name LIKE '%EventLog%';
   ```

2. **修复模板匹配逻辑**
   - 在 resolver.rs 中添加 Provider 名称别名映射
   - 实现模糊匹配支持

3. **更新消息数据库**
   - 确保包含正确的 Provider 和 EventID 组合
   - 验证模板内容与 Windows 事件查看器一致

#### **验证方法**
```rust
fn test_eventid_6013_template() {
    let expected = "The system uptime is 354558 seconds.";
    let actual = get_message_for_event("EventLog", 6013);
    assert_eq!(actual, expected, "EventID 6013 template mismatch");
}
```

---

### 1.3 Bug 3: EventID 566 (Microsoft-Windows-Kernel-Power) 值映射缺失

#### **问题描述**
- **现象**: Reason 字段显示数值而非可读字符串
- **预期输出**: "Reason SessionUnlock"
- **实际输出**: "Reason 12"
- **影响**: 中 - 影响事件可读性

#### **当前状态**: 🔄 修复中
- **负责人**: engineer
- **开始时间**: 2026-08-27T03:13:59.946Z
- **预计完成**: 2-3天

#### **根本原因**
1. **VMAP/BMAP 数据未加载**: value_maps 表为空
2. **msgdb-dump 工具问题**: 未正确提取 VMAP/BMAP 数据
3. **DLL 资源缺失**: 目标 DLL 可能不包含 VMAP 块

#### **修复方案**
1. **验证 value_maps 表**
   ```sql
   -- 检查表是否存在
   SELECT name FROM sqlite_master WHERE type='table' AND name='value_maps';
   
   -- 检查内容
   SELECT COUNT(*) FROM value_maps;
   
   -- 查看 Kernel-Power 相关映射
   SELECT * FROM value_maps 
   WHERE provider_name LIKE '%Kernel-Power%';
   ```

2. **修复 msgdb-dump 工具**
   - 确保正确提取 VMAP/BMAP 数据
   - 验证 CRIM 资源解析逻辑

3. **实现临时解决方案**
   ```rust
   // 在 resolver.rs 中添加硬编码的值映射
   fn load_kernel_power_value_maps(&mut self) -> Result<()> {
       let reason_map = HashMap::from([
           ("12".to_string(), "SessionUnlock".to_string()),
           ("32".to_string(), "InputHid".to_string()),
           ("31".to_string(), "InputAccelerometer".to_string()),
       ]);
       self.value_maps.insert("Reason".to_string(), reason_map);
       Ok(())
   }
   ```

#### **验证方法**
```rust
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
```

---

### 1.4 Bug 4: Setup 通道值映射回归

#### **问题描述**
- **现象**: 222 个 Setup 事件显示数值而非可读字符串
- **预期输出**: "...changed to the Installed state."
- **实际输出**: "...changed to the 5112 state."
- **影响**: 高 - 导致所有 Setup 事件不匹配

#### **当前状态**: 🔄 修复中
- **负责人**: engineer
- **开始时间**: 2026-08-27T03:13:59.946Z
- **预计完成**: 2-3天

#### **根本原因**
1. **value_maps 表为空**: Package State 值映射缺失
2. **语言回退机制不完善**: t4 验证时使用 zh-CN 成功，t11 验证时失败
3. **回归问题**: 修复过程中引入的新问题

#### **修复方案**
1. **实现 Setup 特定值映射**
   ```rust
   // 在 resolver.rs 中添加 Setup 事件的值映射
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

2. **完善语言回退机制**
   - 确保当 zh-CN 不可用时自动回退到 en-US
   - 验证所有 EventID 的语言覆盖情况

#### **验证方法**
```rust
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

---

### 1.5 Bug 5: EventID 8233, 8230 (Microsoft-Windows-Security-SPP) 参数格式问题

#### **问题描述**
- **现象**: GUID 显示格式与 Windows 事件查看器不一致
- **预期输出**: 截断的 GUID 显示
- **实际输出**: 完整的 GUID 显示
- **影响**: 低 - 轻微格式差异

#### **当前状态**: 🔄 修复中
- **负责人**: engineer
- **开始时间**: 2026-08-27T03:13:59.946Z
- **预计完成**: 1天

#### **根本原因**
1. **参数格式化逻辑差异**: evtx-tools 与 Windows 事件查看器的格式化方式不同
2. **GUID 处理方式**: 可能需要特殊处理 GUID 格式

#### **修复方案**
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

#### **验证方法**
```rust
fn test_guid_formatting() {
    let guid = "{55c92734-d682-4d71-983e-d6ec3f16059f}";
    let formatted = format_for_display(guid);
    
    // 检查格式是否正确
    assert!(formatted.starts_with("{"));
    assert!(formatted.ends_with("}"));
    assert!(formatted.contains("-"));
}
```

---

## 2. 修复进度跟踪

### 2.1 当前修复状态
| Bug ID | 问题描述 | 状态 | 负责人 | 开始时间 | 预计完成 |
|--------|----------|------|--------|----------|----------|
| 1 | EventID 24 模板错误 | 🔄 修复中 | engineer | 2026-08-27T03:13:59.946Z | 1-2天 |
| 2 | EventID 6013 模板错误 | 🔄 修复中 | engineer | 2026-08-27T03:13:59.946Z | 1-2天 |
| 3 | EventID 566 值映射缺失 | 🔄 修复中 | engineer | 2026-08-27T03:13:59.946Z | 2-3天 |
| 4 | Setup 通道值映射回归 | 🔄 修复中 | engineer | 2026-08-27T03:13:59.946Z | 2-3天 |
| 5 | EventID 8233/8230 参数格式 | 🔄 修复中 | engineer | 2026-08-27T03:13:59.946Z | 1天 |

### 2.2 修复时间线
1. **阶段 1 (今天)**: 修复模板匹配错误 (EventID 24, 6013)
2. **阶段 2 (明天)**: 修复值映射问题 (EventID 566, Setup)
3. **阶段 3 (后天)**: 修复参数格式问题 (EventID 8233/8230)
4. **阶段 4 (3天后)**: 测试验证和最终报告

### 2.3 修复优先级
1. **高优先级**: EventID 24, 6013, Setup (影响系统事件解析)
2. **中优先级**: EventID 566 (影响事件可读性)
3. **低优先级**: EventID 8233/8230 (轻微格式差异)

---

## 3. 修复技术细节

### 3.1 模板匹配修复技术
#### **Provider 名称标准化**
```rust
// resolver.rs 中的标准化函数
fn normalize_provider_name(name: &str) -> String {
    // 移除 "Microsoft-Windows-" 前缀
    let normalized = name.strip_prefix("Microsoft-Windows-").unwrap_or(name);
    normalized.to_string()
}
```

#### **语言回退机制**
```rust
// 修改 warmup_lang() 添加语言回退
pub fn warmup_lang(&mut self, pairs: &[(String, u32)], lang: &str) -> Result<()> {
    // 1. 先尝试精确匹配
    // 2. 如果失败，尝试标准化后的匹配
    // 3. 最后尝试语言回退 (zh-CN -> en-US)
}
```

### 3.2 值映射修复技术
#### **硬编码值映射**
```rust
// resolver.rs 中的硬编码映射
fn load_kernel_power_value_maps(&mut self) -> Result<()> {
    let reason_map = HashMap::from([
        ("12".to_string(), "SessionUnlock".to_string()),
        ("32".to_string(), "InputHid".to_string()),
    ]);
    self.value_maps.insert("Reason".to_string(), reason_map);
    Ok(())
}
```

#### **Setup 值映射**
```rust
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

### 3.3 参数格式修复技术
#### **GUID 格式化**
```rust
// substitute.rs 中的 GUID 格式化
fn format_guid(guid: &str) -> String {
    if guid.len() > 36 {
        format!("{{{}}}", &guid[1..36])
    } else {
        guid.to_string()
    }
}
```

---

## 4. 验证计划

### 4.1 验证用例
1. **模板匹配验证**: 测试 EventID 24, 6013 的模板匹配
2. **值映射验证**: 测试 EventID 566, Setup 的值映射
3. **参数替换验证**: 测试 EventID 8233/8230 的参数替换
4. **回归验证**: 确保修复不引入新问题

### 4.2 验证方法
1. **单元测试**: 编写 Rust 单元测试
2. **集成测试**: 运行端到端解析测试
3. **对比测试**: 与 Windows 事件查看器输出对比
4. **性能测试**: 验证修复不影响性能

### 4.3 验证时间表
- **阶段 1**: 修复后立即进行单元测试
- **阶段 2**: 每个阶段完成后进行集成测试
- **阶段 3**: 所有修复完成后进行回归测试
- **阶段 4**: 最终验证和性能测试

---

## 5. 相关代码文件

### 5.1 核心代码
- `evtx-tools-core/src/resolver.rs` - 消息解析引擎
- `evtx-tools-core/src/substitute.rs` - 参数替换逻辑
- `evtx-tools-core/src/parser.rs` - EVTX 解析封装
- `evtx-tools-cli/src/cmd_parse.rs` - 解析管道实现

### 5.2 文档和报告
- `docs/validation-methods-guide.md` - 验证方法指南
- `docs/evtx-fix-final-report.md` - 最终修复报告
- `docs/fix-process-documentation.md` - 修复过程文档
- `docs/validation-report-documentation.md` - 验证报告文档

---

**文档版本**: v1.1
**更新时间**: 2026-08-27T03:13:59.946Z
**作者**: researcher
**团队**: evtx-bugfix-team
**状态**: 修复进行中
