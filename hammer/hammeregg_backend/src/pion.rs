//! FFI glue to work with Pion.

use std::fmt::{Debug, Formatter};
use std::os::raw::{c_char, c_int, c_void};

macro_rules! define_handle_structs {
    ($($(#[$meta:ident $($meta_arg:tt)*])* pub struct $name:ident);*$(;)?) => {
        $(
            $(#[$meta $($meta_arg)*])*
            #[repr(transparent)]
            #[derive(Copy, Clone)]
            pub struct $name {
                ptr: usize,
            }

            impl $name {
                #[inline]
                pub fn is_null(&self) -> bool {
                    self.ptr == 0
                }
            }

            impl Debug for $name {
                fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                    f.write_str(stringify!($name))
                }
            }

            unsafe impl Send for $name {}
            unsafe impl Sync for $name {}
        )*
    }
}

define_handle_structs! {
    /// A pointer to a Pion WebRTC connection.
    pub struct PeerConnection;

    /// A pointer to a parsed Pion `SessionDescription`.
    pub struct SessionDescription;
}

extern "C" {
    /// Initializes a Pion RTP -> WebRTC connection.
    ///
    /// **If initialization fails, this will return a
    /// null [`PeerConnection`].**
    pub fn hammer_rtp2rtc_init() -> PeerConnection;

    /// Creates a [`SessionDescription`] that can be used
    /// to signal an offer to an RTC -> WebRTC connection.
    ///
    /// **If the offer is invalid, this will return a
    /// null [`SessionDescription`].**
    pub fn hammer_rtp2rtc_build_offer(offer: *const c_char, offer_len: c_int) -> SessionDescription;

    /// Signals a remote offer to the RTP -> WebRTC connection.
    /// Returns an answer that should be forwarded to the remote peer.
    ///
    /// **If signalling fails, this will return a null pointer.**
    ///
    /// The returned CString should be freed using [`hammer_free_cstring`].
    pub fn hammer_rtp2rtc_signal_offer(connection: PeerConnection, desc: SessionDescription) -> *mut c_char;

    /// Synchronously starts the RTP -> WebRTC connection. RTP packets
    /// will be read from local ports and forwarded to the remote peer.
    ///
    /// The ports this method binds to are passed to `ports_callback`.
    ///
    /// Key and mouse inputs from the remote peer are passed to
    /// `input_callback`.
    pub fn hammer_rtp2rtc_start(
        connection: PeerConnection,
        ports_callback: extern "C" fn(video: u16, audio: u16, user_data: *mut c_void),
        ports_callback_user_data: *mut c_void,
        input_callback: extern "C" fn(),
    );

    /// Asynchronously requests the RTP -> WebRTC connection to stop.
    /// If this is called more than once, any subsequent calls will have
    /// no effect.
    pub fn hammer_rtp2rtc_stop(connection: PeerConnection);

    /// Deletes the RTP -> WebRTC connection.
    /// After calling this, the PeerConnection pointer becomes invalid
    /// and should not be used.
    pub fn hammer_rtp2rtc_free(connection: PeerConnection);

    /// Frees a CString allocated by Go code.
    pub fn hammer_free_cstring(cstring: *mut c_char);
}

/// Uses questionable casting to turn a Rust closure into a callback
/// function and user data pair that can be called from C. Returns
/// a tuple containing the original closure (so it doesn't get
/// prematurely dropped), the c callback, and the user_data to pass
/// to a C function.
macro_rules! make_c_closure {
    ($($closure:tt)*) => {
        {
            let mut closure = $($closure)*;
            crate::pion::__make_c_closure_rest!(closure callback data $($closure)*);
            (closure, callback, data)
        }
    };
}

#[doc(hidden)]
macro_rules! __make_c_closure_rest {
    ($closure:ident $callback:ident $data:ident $(move)? |$($arg_name:ident: $arg_type:ty),*$(,)?| $code:block) => {
        let $data = &mut (&mut $closure as &mut dyn FnMut($($arg_type),*))
                        as *mut &mut dyn FnMut($($arg_type),*)
                        as *mut ::std::os::raw::c_void;
        extern "C" fn $callback($($arg_name: $arg_type),*, user_data: *mut ::std::os::raw::c_void) {
            let closure: &mut &mut dyn FnMut($($arg_type),*) = unsafe { std::mem::transmute(user_data) };
            closure($($arg_name),*);
        }
    };
}

pub(crate) use {__make_c_closure_rest, make_c_closure};
