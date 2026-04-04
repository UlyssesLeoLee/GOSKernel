//! Edge: init
use crate::pluginGroup::K_IDT::node::IDT;

/// 激活 K_IDT 插件：加载中断描述符表
pub fn init() {
    IDT.load();
    crate::serial_println!("[K_IDT]    online  Exceptions routed");
}
