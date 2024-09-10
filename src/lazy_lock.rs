use core::cell::UnsafeCell;
use core::ops::Deref;
use core::sync::atomic::AtomicBool;
use core::sync::atomic::Ordering;

pub struct LazyLock<T> {
    value: UnsafeCell<Option<T>>,
    initialized: AtomicBool,
    initializer: fn() -> T,
}

impl<T> LazyLock<T> {
    pub const fn new(initializer: fn() -> T) -> Self {
        LazyLock {
            value: UnsafeCell::new(None),
            initialized: AtomicBool::new(false),
            initializer,
        }
    }
}

impl <T> Deref for LazyLock<T>{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        if !self.initialized.load(Ordering::Acquire) {
            let value = (self.initializer)();
            unsafe {
                *self.value.get() = Some(value);
            }
            self.initialized.store(true, Ordering::Release);
        }
        unsafe { &(*self.value.get()).as_ref().unwrap() }
    }
}


unsafe impl<T: Send + Sync> Sync for LazyLock<T> {}