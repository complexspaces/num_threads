extern crate libc;

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
}

pub(crate) fn num_threads() -> Option<NonZeroUsize> {
    // https://developer.apple.com/documentation/kernel/1537751-task_threads
    let mut thread_list: thread_act_array_t = std::ptr::null_mut();
    let mut thread_count = 0;

    // Safety:
    //  - `mach_task_self` always returns a valid value,
    //  - `thread_list` is a pointer that will point to kernel allocated memory that needs to be
    //    deallocated if the call succeeds
    let task = unsafe { libc::mach_task_self() };
    let result = unsafe { task_threads(task, &mut thread_list, &mut thread_count) };

    if result == libc::KERN_SUCCESS {
        // Deallocate the mach port rights for the threads
        for thread in 0..thread_count {
            unsafe {
                mach_port_deallocate(task, *(thread_list.offset(thread as isize)) as u32);
            }
        }
        // Deallocate the thread list's memory, now that everything inside of it has been released.
        unsafe {
            vm_deallocate(
                task,
                thread_list as vm_address_t,
                std::mem::size_of::<mach_port_t>() * thread_count as usize,
            );
        }

        NonZeroUsize::new(thread_count as usize)
    } else {
        None
    }
}
