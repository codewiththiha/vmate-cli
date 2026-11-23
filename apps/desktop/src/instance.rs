//! Linux per-user ownership and existing-window activation.

use std::{
    io,
    os::linux::net::SocketAddrExt,
    os::unix::net::{SocketAddr, UnixListener, UnixStream},
};

pub(super) struct Instance(UnixListener);

impl Instance {
    pub(super) fn acquire() -> io::Result<Option<Self>> {
        let name = format!("vmate-desktop-{}", rustix::process::getuid().as_raw());
        let address = SocketAddr::from_abstract_name(name)?;
        match UnixListener::bind_addr(&address) {
            Ok(listener) => {
                listener.set_nonblocking(true)?;
                Ok(Some(Self(listener)))
            }
            Err(error) if error.kind() == io::ErrorKind::AddrInUse => {
                UnixStream::connect_addr(&address)?;
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    pub(super) fn bind(self, app: slint::Weak<crate::AppWindow>) -> slint::Timer {
        let timer = slint::Timer::default();
        timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(100),
            move || {
                for _ in 0..16 {
                    match self.0.accept() {
                        Ok((stream, _)) => {
                            let own_user = rustix::net::sockopt::socket_peercred(&stream)
                                .is_ok_and(|peer| peer.uid == rustix::process::getuid());
                            if own_user {
                                crate::tray::reopen(&app, false);
                            }
                        }
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                        Err(_) => break,
                    }
                }
            },
        );
        timer
    }
}
