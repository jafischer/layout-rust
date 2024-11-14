use std::ops::Deref;

use cocoa_foundation::base::{id, nil};
use cocoa_foundation::foundation::NSAutoreleasePool;

// Got this from https://github.com/tauri-apps/tao although I've seen it in several github repos.
#[derive(Debug, PartialEq)]
pub struct IdRef(id);

impl IdRef {
    pub fn new(inner: id) -> IdRef {
        IdRef(inner)
    }

    #[allow(dead_code)]
    pub fn retain(inner: id) -> IdRef {
        if inner != nil {
            let _: id = unsafe { msg_send![inner, retain] };
        }
        IdRef(inner)
    }

    #[allow(unused)]
    pub fn non_nil(self) -> Option<IdRef> {
        if self.0 == nil {
            None
        } else {
            Some(self)
        }
    }
}

impl Drop for IdRef {
    fn drop(&mut self) {
        if self.0 != nil {
            unsafe {
                let pool = NSAutoreleasePool::new(nil);
                let () = msg_send![self.0, release];
                pool.drain();
            };
        }
    }
}

impl Deref for IdRef {
    type Target = id;
    #[allow(clippy::needless_lifetimes)]
    fn deref<'a>(&'a self) -> &'a id {
        &self.0
    }
}

impl Clone for IdRef {
    fn clone(&self) -> IdRef {
        IdRef::retain(self.0)
    }
}
