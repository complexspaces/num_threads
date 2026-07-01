extern crate libc;

use std::convert::TryInto;
use std::num::NonZeroUsize;

use self::libc::{
    kern_return_t, mach_port_t, natural_t, task_threads, thread_act_array_t, vm_address_t,
    vm_deallocate,
};

#[allow(non_camel_case_types)]
// https://developer.apple.com/documentation/kernel/mach_port_name_t
type mach_port_name_t = natural_t;

extern "C" {
    // https://developer.apple.com/documentation/kernel/1578777-mach_port_deallocate
    fn mach_port_deallocate(task: mach_port_t, name: mach_port_name_t) -> kern_return_t;
    // `libc::mach_task_self` (deprecated) wraps this and its SDK header states that it
    // is concurrency safe (mach/mach_init.h):
    // `extern __swift_nonisolated_unsafe mach_port_t mach_task_self_;`
    static mut mach_task_self_: mach_port_t;
}

pub(crate) fn num_threads() -> Option<NonZeroUsize> {
    // https://developer.apple.com/documentation/kernel/1537751-task_threads
    let mut thread_list: thread_act_array_t = std::ptr::null_mut();
    let mut thread_count = 0;

    // Safety:
    //  - `mach_task_self` is always valid to access,
    //  - `thread_list` is a pointer that will point to kernel allocated memory that needs to be
    //    deallocated if the call succeeds
    let result = unsafe { task_threads(mach_task_self_, &mut thread_list, &mut thread_count) };

    if result == libc::KERN_SUCCESS {
        // It is impossible for the early return below to be taken and cause a memory leak:
        // - `u32` -> `usize` is infallible on any Apple target because the minimum pointer size
        //   anywhere is 32-bit,
        // - It is not possible for a process to have 0 threads and be alive, so this code running
        //   proves at least one thread exists
        let thread_count = thread_count.try_into().ok().and_then(NonZeroUsize::new)?;

        // Deallocate the mach port rights for the threads
        for thread in 0..thread_count.get() {
            // Safety:
            // - `mach_task_self` is always valid to access,
            // - `thread_list` is valid to read and `thread` is always within the array's bounds
            unsafe { mach_port_deallocate(mach_task_self_, *(thread_list.add(thread))) };
        }
        // Deallocate the thread list's memory, now that everything inside of it has been released.
        // Safety:
        // `mach_task_self` is always valid to access,
        // `thread_list` was originally allocated as kernel memory and has not been modified.
        // `size` is the same number of elements returned by the original call and the same number
        // deallocated above.
        unsafe {
            vm_deallocate(
                mach_task_self_,
                // XXX: Use `expose_provenance` when MSRV is high enough.
                thread_list as vm_address_t,
                thread_count.get() * std::mem::size_of::<mach_port_t>(),
            );
        }

        Some(thread_count)
    } else {
        None
    }
}
