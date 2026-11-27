use std::io;
use std::os::fd::RawFd;
use std::time::Duration;

pub struct Poller(RawFd);

fn check(result: libc::c_int) -> io::Result<libc::c_int> {
    if result < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(result)
    }
}

#[cfg(target_os = "macos")]
impl Poller {
    pub fn new() -> io::Result<Poller> {
        // SAFETY: kqueue takes no arguments and returns a new descriptor or -1.
        check(unsafe { libc::kqueue() }).map(Poller)
    }

    pub fn arm(&self, fd: RawFd, token: u64) -> io::Result<()> {
        let change = libc::kevent {
            ident: fd as libc::uintptr_t,
            filter: libc::EVFILT_READ,
            flags: libc::EV_ADD | libc::EV_ONESHOT,
            fflags: 0,
            data: 0,
            udata: token as *mut libc::c_void,
        };
        // SAFETY: one change is read from a live local and no events are written.
        check(unsafe {
            libc::kevent(
                self.0,
                &change,
                1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            )
        })
        .map(drop)
    }

    pub fn wait(&self, tokens: &mut Vec<u64>, timeout: Duration) -> io::Result<()> {
        let empty = libc::kevent {
            ident: 0,
            filter: 0,
            flags: 0,
            fflags: 0,
            data: 0,
            udata: std::ptr::null_mut(),
        };
        let mut events = [empty; 256];
        let wait = libc::timespec {
            tv_sec: timeout.as_secs() as libc::time_t,
            tv_nsec: timeout.subsec_nanos() as libc::c_long,
        };
        // SAFETY: the kernel writes at most events.len() entries into the local array.
        let found = unsafe {
            libc::kevent(
                self.0,
                std::ptr::null(),
                0,
                events.as_mut_ptr(),
                events.len() as libc::c_int,
                &wait,
            )
        };
        let found = match check(found) {
            Ok(found) => found as usize,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => 0,
            Err(error) => return Err(error),
        };
        tokens.extend(events[..found].iter().map(|event| event.udata as u64));
        Ok(())
    }
}

#[cfg(target_os = "linux")]
impl Poller {
    pub fn new() -> io::Result<Poller> {
        // SAFETY: epoll_create1 takes a flag and returns a new descriptor or -1.
        check(unsafe { libc::epoll_create1(libc::EPOLL_CLOEXEC) }).map(Poller)
    }

    pub fn arm(&self, fd: RawFd, token: u64) -> io::Result<()> {
        let mut event = libc::epoll_event {
            events: (libc::EPOLLIN | libc::EPOLLRDHUP | libc::EPOLLONESHOT) as u32,
            u64: token,
        };
        // SAFETY: the event is a live local that the kernel only reads.
        let modified =
            check(unsafe { libc::epoll_ctl(self.0, libc::EPOLL_CTL_MOD, fd, &mut event) });
        match modified {
            Err(error) if error.raw_os_error() == Some(libc::ENOENT) => {
                // SAFETY: as above, for a descriptor not yet registered.
                check(unsafe { libc::epoll_ctl(self.0, libc::EPOLL_CTL_ADD, fd, &mut event) })
                    .map(drop)
            }
            other => other.map(drop),
        }
    }

    pub fn wait(&self, tokens: &mut Vec<u64>, timeout: Duration) -> io::Result<()> {
        let mut events = [libc::epoll_event { events: 0, u64: 0 }; 256];
        // SAFETY: the kernel writes at most events.len() entries into the local array.
        let found = unsafe {
            libc::epoll_wait(
                self.0,
                events.as_mut_ptr(),
                events.len() as libc::c_int,
                timeout.as_millis().min(i32::MAX as u128) as libc::c_int,
            )
        };
        let found = match check(found) {
            Ok(found) => found as usize,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => 0,
            Err(error) => return Err(error),
        };
        tokens.extend(events[..found].iter().map(|event| event.u64));
        Ok(())
    }
}

impl Drop for Poller {
    fn drop(&mut self) {
        // SAFETY: the descriptor is owned by this poller and closed once.
        unsafe { libc::close(self.0) };
    }
}

pub fn writable(fd: RawFd, timeout: Duration) -> bool {
    ready(fd, libc::POLLOUT, timeout)
}

pub fn readable(fd: RawFd, timeout: Duration) -> bool {
    ready(fd, libc::POLLIN, timeout)
}

fn ready(fd: RawFd, events: libc::c_short, timeout: Duration) -> bool {
    let mut wanted = libc::pollfd {
        fd,
        events,
        revents: 0,
    };
    let millis = timeout.as_millis().clamp(1, i32::MAX as u128) as libc::c_int;
    // SAFETY: one pollfd is passed from a live local.
    unsafe { libc::poll(&mut wanted, 1, millis) > 0 }
}
