//! FFI glue to work with Pion.

use std::os::raw::c_char;

/// A pointer to a Pion WebRTC connection.
/// **This pointer is NOT thread safe**.
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct PeerConnection {
    _ptr: usize,
}

unsafe impl Send for PeerConnection {}
unsafe impl Sync for PeerConnection {}

extern "C" {
    /// Initializes a Pion RTP -> WebRTC connection.
    pub fn hammer_rtp2rtc_init() -> PeerConnection;

    /// Signals a remote offer to the RTP -> WebRTC connection.
    /// Returns an answer that should be forwarded to the remote peer.
    /// The returned CString should be freed using [`hammer_free_cstring`].
    pub fn hammer_rtp2rtc_signal_offer(connection: PeerConnection) -> *mut c_char;

    /// Synchronously starts the RTP -> WebRTC connection. RTP packets
    /// will be read from the given port and forwarded to the remote peer.
    /// Key and mouse inputs from the remote peer will be given to the
    /// provided callback.
    pub fn hammer_rtp2rtc_start(connection: PeerConnection, port: u16, input_callback: extern "C" fn());

    /// Asynchronously requests the RTP -> WebRTC connection to stop.
    pub fn hammer_rtp2rtc_stop(connection: PeerConnection);

    /// Frees a CString allocated by Go code.
    pub fn hammer_free_cstring(cstring: *mut c_char);
}
