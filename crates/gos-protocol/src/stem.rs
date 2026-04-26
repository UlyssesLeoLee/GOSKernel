use crate::{NodeCell, CellDeclaration, CellResult, NodeState, Signal, VectorAddress};

/// 细胞特质基因 (CellBehavior)
///
/// 特化功能的极简纯粹逻辑接口，抛弃了所有 NodeState 管理和 NGR 繁文缛节。
pub trait CellBehavior: core::marker::Send {
    /// 宣告此组件的功能与依赖
    fn declare(&self) -> CellDeclaration;
    
    /// 触发硬件层面的生命开始 (Spawn 第一次到达时调用)
    ///
    /// # Safety
    ///
    /// Caller must guarantee the cell has been registered with a valid
    /// runtime page before invocation; this method may touch hardware
    /// state (port writes, MSR writes) that is unsafe outside that
    /// context.
    unsafe fn init(&mut self) {}
    
    /// 接收特定的外界信号
    #[allow(unused_variables)]
    fn on_signal(&mut self, signal: Signal) -> CellResult {
        CellResult::Done
    }

    /// 执行组件独立的内部轮询循环 (如果在 NGR 调度器中需要轮询)
    fn on_activate(&mut self) -> CellResult {
        CellResult::Done
    }
    
    /// 当休眠时需要的析构处理
    fn on_suspend(&mut self) {}
}

/// 超级干细胞容器 (StemCell)
/// 
/// 真正的造物主结构。内部封装了系统的 `NodeState` 与 `VectorAddress`，
/// 它是通用的 NodeCell，把路由的责任从特化的功能开发中剥离出来。
pub struct StemCell<B: CellBehavior> {
    pub state: NodeState,
    pub vec: VectorAddress,
    pub behavior: B,
}

impl<B: CellBehavior> StemCell<B> {
    /// 用于装配特定基因的构造方法
    pub const fn new(vec: VectorAddress, behavior: B) -> Self {
        Self {
            state: NodeState::Unregistered,
            vec,
            behavior,
        }
    }
    
    /// 方便开发者强制手动进入就绪态（独立系统引导时使用）
    pub fn force_ready(&mut self) {
        self.state = NodeState::Ready;
    }
}

/// 干细胞天然遵守系统的 NGR 底层生命周期交互法则，并自动反射调用内置的特殊 Behavior。
impl<B: CellBehavior> NodeCell for StemCell<B> {
    fn declare(&self) -> CellDeclaration {
        let mut d = self.behavior.declare();
        d.vec = self.vec; // 强制覆盖保证法则安全
        d
    }

    unsafe fn init(&mut self) {
        self.behavior.init();
        self.state = NodeState::Ready;
    }

    fn on_activate(&mut self) -> CellResult {
        if self.state != NodeState::Ready && self.state != NodeState::Running {
            return CellResult::Done;
        }
        self.state = NodeState::Running;
        let res = self.behavior.on_activate();
        if matches!(res, CellResult::Done) {
            self.state = NodeState::Ready;
        }
        res
    }

    fn on_signal(&mut self, signal: Signal) -> CellResult {
        match signal {
            // NGR 系统标准处理机制：Spawn 信号第一次收到时强制 init
            Signal::Spawn { .. } => {
                if self.state == NodeState::Unregistered {
                    unsafe { self.init(); }
                }
                // 转发给特化行为
                self.behavior.on_signal(signal)
            }
            Signal::Terminate => {
                self.behavior.on_suspend();
                self.state = NodeState::Terminated;
                CellResult::Done
            }
            _ => {
                if self.state == NodeState::Unregistered {
                    // 没有点燃的干细胞拒绝处理除了 Spawn 以外的任何信号
                    return CellResult::Fault("Unregistered stem cell received operational signal");
                }
                self.behavior.on_signal(signal)
            }
        }
    }

    fn on_suspend(&mut self) {
        self.behavior.on_suspend();
        self.state = NodeState::Suspended;
    }

    fn state(&self) -> NodeState {
        self.state
    }

    fn vec(&self) -> VectorAddress {
        self.vec
    }
}
