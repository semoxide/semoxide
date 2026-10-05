//! Kill a plugin together with everything it started.
//! Windows: Job Object with KILL_ON_JOB_CLOSE. Unix: own process group.

#[cfg(windows)]
pub struct ProcessTree {
    job: windows_sys::Win32::Foundation::HANDLE,
}

// The job handle is an owned kernel handle; usable from any thread.
#[cfg(windows)]
unsafe impl Send for ProcessTree {}
#[cfg(windows)]
unsafe impl Sync for ProcessTree {}

#[cfg(windows)]
impl ProcessTree {
    /// Prepare the command (nothing to do on Windows; the job is assigned after spawn).
    pub fn prepare(_cmd: &mut tokio::process::Command) {}

    /// Create a job and put the freshly spawned child in it.
    ///
    /// Race: the child runs before `AssignProcessToJobObject`, so a process it
    /// spawns in its first instructions could escape. Closing it needs
    /// `CREATE_SUSPENDED` + `ResumeThread` (std does not expose the thread
    /// handle) or `PROC_THREAD_ATTRIBUTE_JOB_LIST` (nightly `raw_attribute`).
    pub fn attach(child: &tokio::process::Child) -> std::io::Result<Self> {
        use windows_sys::Win32::System::JobObjects::*;
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return Err(std::io::Error::last_os_error());
            }
            let tree = Self { job };
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            ) == 0
            {
                return Err(std::io::Error::last_os_error());
            }
            let h = child.raw_handle().ok_or_else(|| std::io::Error::other("child already reaped"))?;
            if AssignProcessToJobObject(job, h as _) == 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(tree)
        }
    }

    pub fn kill(&self) {
        unsafe {
            windows_sys::Win32::System::JobObjects::TerminateJobObject(self.job, 1);
        }
    }
}

#[cfg(windows)]
impl Drop for ProcessTree {
    fn drop(&mut self) {
        // KILL_ON_JOB_CLOSE: closing the last handle kills every process in
        // the job, also when the host itself crashes.
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.job);
        }
    }
}

#[cfg(unix)]
pub struct ProcessTree {
    pgid: i32,
}

#[cfg(unix)]
impl ProcessTree {
    /// Start the child as leader of a new process group.
    pub fn prepare(cmd: &mut tokio::process::Command) {
        cmd.process_group(0);
    }

    pub fn attach(child: &tokio::process::Child) -> std::io::Result<Self> {
        let pid = child.id().ok_or_else(|| std::io::Error::other("child already reaped"))?;
        Ok(Self { pgid: pid as i32 })
    }

    /// SIGKILL the whole group. Escapes: a descendant that calls `setsid()`
    /// or `setpgid()` leaves the group.
    pub fn kill(&self) {
        unsafe {
            libc::killpg(self.pgid, libc::SIGKILL);
        }
    }
}

#[cfg(unix)]
impl Drop for ProcessTree {
    // Mirror KILL_ON_JOB_CLOSE. Unlike a job, this does NOT fire if the host
    // process itself dies (would need PR_SET_PDEATHSIG / a subreaper).
    fn drop(&mut self) {
        self.kill();
    }
}

/// Is `pid` still running? (Tests: verify grandchildren are gone.)
pub fn is_alive(pid: u32) -> bool {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
        use windows_sys::Win32::System::Threading::*;
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return false;
        }
        let mut code = 0u32;
        let ok = GetExitCodeProcess(h, &mut code);
        CloseHandle(h);
        ok != 0 && code == STILL_ACTIVE as u32
    }
    #[cfg(unix)]
    {
        // A killed orphan may linger as a zombie when PID 1 does not reap
        // (e.g. `docker run` without --init): treat zombies as dead.
        match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
            Ok(s) => s
                .rsplit_once(')')
                .and_then(|(_, rest)| rest.split_whitespace().next())
                .is_some_and(|state| state != "Z" && state != "X"),
            Err(_) => unsafe { libc::kill(pid as i32, 0) == 0 },
        }
    }
}
