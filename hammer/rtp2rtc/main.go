package main

// #include <stdlib.h>
// #include <stdint.h>
// #include <bridge.h>
import "C"
import "fmt"
import "runtime/cgo"
import "unsafe"

// Since rtp2rtc is always built as a
// static library this is a no-op.
func main() {}

type PeerConnection struct {

}

//export hammer_rtp2rtc_init
func hammer_rtp2rtc_init() C.uintptr_t {
    fmt.Println("calling go from rust go brrr")
    return C.uintptr_t(cgo.NewHandle(PeerConnection{}))
}

//export hammer_rtp2rtc_signal_offer
func hammer_rtp2rtc_signal_offer(connection C.uintptr_t) *C.char {
    return C.CString("")
}

//export hammer_rtp2rtc_start
func hammer_rtp2rtc_start(connection C.uintptr_t, port C.uint16_t, callback C.hammer_rtp2rtc_input_callback) {
    C.HammerRTP2RTCInputCallbackBridge(callback)
}

//export hammer_rtp2rtc_stop
func hammer_rtp2rtc_stop(connection C.uintptr_t) {
    
}

//export hammer_free_cstring
func hammer_free_cstring(cstring *C.char) {
    C.free(unsafe.Pointer(cstring))
}
