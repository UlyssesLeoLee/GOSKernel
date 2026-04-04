//! Edge: init
use crate::pluginGroup::K_GDT::node::GDT;
use x86_64::instructions::segmentation::{Segment, CS};
use x86_64::instructions::tables::load_tss;

/// 激活 K_GDT 插件：加载 GDT 和 TSS。
pub fn init() {
    GDT.0.load();
    unsafe {
        CS::set_reg(GDT.1.code_selector);
        load_tss(GDT.1.tss_selector);
    }
    crate::serial_println!("[K_GDT]    online  TSS & Code Segment loaded");
}
